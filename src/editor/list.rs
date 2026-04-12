use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, TableState},
};
use rusqlite::Connection;
use std::time::Duration;

use super::db::{self, Article, Status};
use super::ops;

/// Action returned from the list view to the dispatcher.
pub enum ListAction {
    Edit(i64),
    New,
    Quit,
}

pub struct ListView {
    blog_dir: PathBuf,
    articles: Vec<Article>,
    table_state: TableState,
    search_query: String,
    search_active: bool,
    flash: Option<(String, Instant)>,
    show_help: bool,
    confirm_delete: Option<i64>,
    building: bool,
    build_result: Arc<Mutex<Option<Result<String, String>>>>,
    last_built: Option<Instant>,
    last_error: Option<String>,
    show_error: bool,
    serving: bool,
    serve_after_build: bool,
    serve_stop: Arc<AtomicBool>,
    serve_handle: Option<JoinHandle<()>>,
    deploying: bool,
    deploy_result: Arc<Mutex<Option<Result<ops::DeployPreview, String>>>>,
    deploy_preview: Option<ops::DeployPreview>,
    pushing: bool,
    push_result: Arc<Mutex<Option<Result<String, String>>>>,
    deploy_status: Option<String>,
    show_theme_picker: bool,
    themes: Vec<String>,
    theme_index: usize,
    current_theme: String,
}

impl ListView {
    pub fn new(blog_dir: PathBuf, articles: Vec<Article>, config: &crate::engine::config::BlogConfig) -> Self {
        let mut table_state = TableState::default();
        if !articles.is_empty() {
            table_state.select(Some(0));
        }
        let current_theme = config.theme.clone().unwrap_or_else(|| "paper".to_string());
        let themes = ops::available_themes();
        let theme_index = themes.iter().position(|t| t == &current_theme).unwrap_or(0);
        Self {
            blog_dir,
            articles,
            table_state,
            search_query: String::new(),
            search_active: false,
            flash: None,
            show_help: false,
            confirm_delete: None,
            building: false,
            build_result: Arc::new(Mutex::new(None)),
            last_built: None,
            last_error: None,
            show_error: false,
            serving: false,
            serve_after_build: false,
            serve_stop: Arc::new(AtomicBool::new(false)),
            serve_handle: None,
            deploying: false,
            deploy_result: Arc::new(Mutex::new(None)),
            deploy_preview: None,
            pushing: false,
            push_result: Arc::new(Mutex::new(None)),
            deploy_status: None,
            show_theme_picker: false,
            themes,
            theme_index,
            current_theme,
        }
    }

    pub fn run(
        &mut self,
        terminal: &mut ratatui::DefaultTerminal,
        conn: &Connection,
    ) -> io::Result<ListAction> {
        loop {
            self.poll_build_result();
            self.poll_deploy_result();
            self.poll_push_result();
            self.draw(terminal)?;

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if let Some(action) = self.handle_key(key.code, key.modifiers, conn)? {
                        return Ok(action);
                    }
                }
            }
        }
    }

    fn poll_build_result(&mut self) {
        let result = if let Ok(mut guard) = self.build_result.lock() {
            guard.take()
        } else {
            return;
        };
        if let Some(result) = result {
            self.building = false;
            match result {
                Ok(msg) => {
                    self.flash = Some((msg, Instant::now()));
                    self.last_built = Some(Instant::now());
                    if self.serve_after_build {
                        self.serve_after_build = false;
                        self.spawn_serve();
                    }
                }
                Err(err) => {
                    self.serve_after_build = false;
                    let first_line = err.lines().next().unwrap_or(&err).to_string();
                    self.flash = Some((format!("Build failed: {first_line}"), Instant::now()));
                    self.last_error = Some(err);
                }
            }
        }
    }

    fn start_serve(&mut self) {
        if self.serving {
            return;
        }
        // Build first, then spawn server after build completes
        self.serve_after_build = true;
        self.start_build();
    }

    fn spawn_serve(&mut self) {
        self.serving = true;
        self.serve_stop.store(false, Ordering::Relaxed);

        let dist_dir = ops::dist_dir_for_blog(&self.blog_dir);
        let stop = Arc::clone(&self.serve_stop);

        self.serve_handle = Some(std::thread::spawn(move || {
            let _ = ops::run_serve(&dist_dir, stop);
        }));

        self.flash = Some((
            format!("Serving at http://localhost:{}", ops::SERVE_PORT),
            Instant::now(),
        ));
    }

    fn stop_serve(&mut self) {
        if !self.serving {
            return;
        }
        self.serve_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.serve_handle.take() {
            let _ = handle.join();
        }
        self.serving = false;
        self.flash = Some(("Server stopped".into(), Instant::now()));
    }

    fn start_deploy(&mut self) {
        if self.deploying || self.building {
            return;
        }
        self.deploying = true;
        self.flash = Some(("Deploying...".into(), Instant::now()));

        let blog_dir = self.blog_dir.clone();
        let result_slot = Arc::clone(&self.deploy_result);

        std::thread::spawn(move || {
            let result = ops::deploy_build_and_sync(&blog_dir);
            if let Ok(mut guard) = result_slot.lock() {
                *guard = Some(result);
            }
        });
    }

    fn poll_deploy_result(&mut self) {
        let result = if let Ok(mut guard) = self.deploy_result.lock() {
            guard.take()
        } else {
            return;
        };
        if let Some(result) = result {
            self.deploying = false;
            self.last_built = Some(Instant::now());
            match result {
                Ok(preview) => {
                    if preview.diff.is_empty() {
                        self.flash = Some(("Nothing to deploy".into(), Instant::now()));
                    } else {
                        self.deploy_preview = Some(preview);
                        self.flash = None;
                    }
                }
                Err(err) => {
                    let first_line = err.lines().next().unwrap_or(&err).to_string();
                    self.flash = Some((format!("Deploy failed: {first_line}"), Instant::now()));
                    self.last_error = Some(err);
                }
            }
        }
    }

    fn start_push(&mut self) {
        if self.pushing {
            return;
        }
        let Some(preview) = self.deploy_preview.take() else {
            return;
        };
        self.pushing = true;
        self.flash = Some(("Pushing...".into(), Instant::now()));

        let deploy_dir = preview.deploy_dir;
        let blog_dir = self.blog_dir.clone();
        let result_slot = Arc::clone(&self.push_result);

        std::thread::spawn(move || {
            let result = ops::repo_commit_push(&deploy_dir);
            if let Ok(push_result) = &result {
                // Fetch CF status after successful push
                let project = blog_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().replace('.', "-"))
                    .unwrap_or_default();
                let status = ops::cf_deployment_status(&project)
                    .unwrap_or_else(|e| format!("Could not fetch status: {e}"));
                let msg = format!("{push_result}\n{status}");
                if let Ok(mut guard) = result_slot.lock() {
                    *guard = Some(Ok(msg));
                }
            } else if let Ok(mut guard) = result_slot.lock() {
                *guard = Some(result);
            }
        });
    }

    fn poll_push_result(&mut self) {
        let result = if let Ok(mut guard) = self.push_result.lock() {
            guard.take()
        } else {
            return;
        };
        if let Some(result) = result {
            self.pushing = false;
            match result {
                Ok(msg) => {
                    self.deploy_status = Some(msg);
                }
                Err(err) => {
                    let first_line = err.lines().next().unwrap_or(&err).to_string();
                    self.flash = Some((format!("Push failed: {first_line}"), Instant::now()));
                    self.last_error = Some(err);
                }
            }
        }
    }

    fn start_build(&mut self) {
        if self.building {
            return;
        }
        self.building = true;

        let blog_dir = self.blog_dir.clone();
        let dist_dir = ops::dist_dir_for_blog(&self.blog_dir);
        let result_slot = Arc::clone(&self.build_result);

        std::thread::spawn(move || {
            let start = Instant::now();
            let result = ops::run_build(&blog_dir, &dist_dir);
            let msg = match result {
                Ok(report) => Ok(format!(
                    "Built {} articles in {:.1}s",
                    report.built,
                    start.elapsed().as_secs_f64()
                )),
                Err(e) => Err(e),
            };
            if let Ok(mut guard) = result_slot.lock() {
                *guard = Some(msg);
            }
        });
    }

    fn draw(&mut self, terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
        // Expire flash after 2s
        if let Some((_, instant)) = &self.flash {
            if instant.elapsed() > Duration::from_secs(2) {
                self.flash = None;
            }
        }

        terminal.draw(|frame| {
            let rows = Layout::vertical([
                Constraint::Length(1), // header
                Constraint::Min(1),   // table
                Constraint::Length(1), // status bar
            ])
            .split(frame.area());

            // Header with flash message
            self.render_header(frame, rows[0]);

            // Article table
            self.render_table(frame, rows[1]);

            // Status bar (search or hotkey hints)
            self.render_status_bar(frame, rows[2]);

            // Help overlay
            if self.show_help {
                self.render_help_overlay(frame, frame.area());
            }

            // Delete confirmation
            if let Some(id) = self.confirm_delete {
                self.render_delete_confirm(frame, frame.area(), id);
            }

            // Deploy preview (diff + confirm)
            if self.deploy_preview.is_some() {
                self.render_deploy_preview(frame, frame.area());
            }

            // Theme picker
            if self.show_theme_picker {
                self.render_theme_picker(frame, frame.area());
            }

            // Deploy status
            if self.deploy_status.is_some() {
                self.render_deploy_status(frame, frame.area());
            }

            // Error overlay
            if self.show_error {
                self.render_error_overlay(frame, frame.area());
            }
        })?;
        Ok(())
    }

    fn render_header(&self, frame: &mut ratatui::Frame, area: Rect) {
        let blog_name = self
            .blog_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let sep = Span::styled(" · ", Style::default().fg(Color::DarkGray));

        let mut spans = vec![
            Span::styled(" DevTUI CMS ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(format!("  {blog_name}")),
            sep.clone(),
            Span::styled(
                format!("{} articles", self.articles.len()),
                Style::default().fg(Color::DarkGray),
            ),
            sep.clone(),
        ];

        if self.deploying {
            spans.push(Span::styled(
                "deploying...",
                Style::default().fg(Color::Yellow),
            ));
        } else if self.pushing {
            spans.push(Span::styled(
                "pushing...",
                Style::default().fg(Color::Yellow),
            ));
        } else if self.building {
            spans.push(Span::styled(
                "building...",
                Style::default().fg(Color::Yellow),
            ));
        } else if self.serving {
            spans.push(Span::styled(
                format!("serving :{}", ops::SERVE_PORT),
                Style::default().fg(Color::Green),
            ));
        } else {
            spans.push(Span::styled(
                "server stopped",
                Style::default().fg(Color::DarkGray),
            ));
        }

        if let Some(built_at) = &self.last_built {
            spans.push(sep);
            spans.push(Span::styled(
                ops::format_built_ago(built_at.elapsed()),
                Style::default().fg(Color::DarkGray),
            ));
        }

        if let Some((msg, _)) = &self.flash {
            let color = if msg.starts_with("Build failed") {
                Color::Red
            } else {
                Color::Green
            };
            spans.push(Span::styled(
                format!("  {msg}"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
        }

        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn render_table(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let header = Row::new(vec![
            " STATUS", "PIN", "LANG", "DATE", "TITLE",
        ])
        .style(Style::default().fg(Color::DarkGray));

        let rows: Vec<Row> = self
            .articles
            .iter()
            .map(|article| {
                let status = match article.status {
                    Status::Published => Span::styled(" PUB", Style::default().fg(Color::Green)),
                    Status::Draft => Span::styled(" DRF", Style::default().fg(Color::Yellow)),
                };
                let pin = if article.pinned {
                    Span::styled(" *", Style::default().fg(Color::Magenta))
                } else {
                    Span::raw("  ")
                };
                let lang = Span::raw(format!(" {}", article.language));
                let date = Span::raw(format!(
                    " {}",
                    article
                        .published_at
                        .as_deref()
                        .unwrap_or(&article.created_at)
                        .get(..10)
                        .unwrap_or("          ")
                ));
                let title_text = if article.title.len() > 60 {
                    format!(" {}...", &article.title[..57])
                } else {
                    format!(" {}", article.title)
                };
                let title = Span::raw(title_text);
                Row::new(vec![
                    Line::from(status),
                    Line::from(pin),
                    Line::from(lang),
                    Line::from(date),
                    Line::from(title),
                ])
            })
            .collect();

        let widths = [
            Constraint::Length(7),
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Min(20),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Articles "),
            )
            .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .highlight_symbol("> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_status_bar(&self, frame: &mut ratatui::Frame, area: Rect) {
        if self.search_active {
            let search = Line::from(vec![
                Span::styled(" / ", Style::default().fg(Color::Yellow)),
                Span::raw(&self.search_query),
                Span::styled("_", Style::default().fg(Color::Yellow)),
            ]);
            frame.render_widget(Paragraph::new(search), area);
        } else {
            let hints = Line::from(Span::styled(
                " j/k:nav  Enter:edit  n:new  p:pub  i:pin  d:del  b:build  s:serve  o:open  D:deploy  t:theme  e:errors  ?:help  q:quit",
                Style::default().fg(Color::DarkGray),
            ));
            frame.render_widget(Paragraph::new(hints), area);
        }
    }

    fn render_help_overlay(&self, frame: &mut ratatui::Frame, area: Rect) {
        let help_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                " Keyboard Shortcuts ",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(" j / Down     Move down"),
            Line::from(" k / Up       Move up"),
            Line::from(" Enter        Edit article"),
            Line::from(" n            New article"),
            Line::from(" p            Toggle publish/draft"),
            Line::from(" i            Toggle pin"),
            Line::from(" d            Delete (with confirm)"),
            Line::from(" b            Build blog"),
            Line::from(" s            Start/stop server"),
            Line::from(" o            Open in browser"),
            Line::from(" D            Deploy (build + rsync)"),
            Line::from(" t            Change theme"),
            Line::from(" e            Show last error"),
            Line::from(" /            Search by title"),
            Line::from(" Esc          Clear search / close"),
            Line::from(" q            Quit"),
            Line::from(""),
            Line::from(Span::styled(
                " Press any key to close ",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let width = 40.min(area.width);
        let height = (help_text.len() as u16 + 2).min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        let help = Paragraph::new(help_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Help "),
        );
        frame.render_widget(ratatui::widgets::Clear, popup_area);
        frame.render_widget(help, popup_area);
    }

    fn render_delete_confirm(&self, frame: &mut ratatui::Frame, area: Rect, _id: i64) {
        let title = self
            .selected_article()
            .map(|a| a.title.as_str())
            .unwrap_or("this article");
        let text = vec![
            Line::from(""),
            Line::from(format!(" Delete \"{}\"?", title)),
            Line::from(""),
            Line::from(vec![
                Span::styled(" y ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("confirm  "),
                Span::styled(" n ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("cancel"),
            ]),
        ];

        let width = 50.min(area.width);
        let height = 6.min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        let confirm = Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title(" Confirm Delete "),
        );
        frame.render_widget(ratatui::widgets::Clear, popup_area);
        frame.render_widget(confirm, popup_area);
    }

    fn render_error_overlay(&self, frame: &mut ratatui::Frame, area: Rect) {
        let error_text = self
            .last_error
            .as_deref()
            .unwrap_or("No errors");

        let lines: Vec<Line> = error_text
            .lines()
            .map(|l| Line::from(format!(" {l}")))
            .collect();

        let width = (area.width - 4).min(80);
        let height = (lines.len() as u16 + 2).min(area.height - 2);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        let error = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title(" Error (Esc to close) "),
        );
        frame.render_widget(ratatui::widgets::Clear, popup_area);
        frame.render_widget(error, popup_area);
    }

    fn render_deploy_preview(&self, frame: &mut ratatui::Frame, area: Rect) {
        let Some(preview) = &self.deploy_preview else {
            return;
        };

        let mut lines = vec![
            Line::from(Span::styled(
                format!(" Deploy to {}", preview.deploy_dir.display()),
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
        ];

        for diff_line in preview.diff.lines().take(20) {
            lines.push(Line::from(format!(" {diff_line}")));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" y ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("commit + push  "),
            Span::styled(" Esc ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("cancel"),
        ]));

        let width = (area.width - 4).min(80);
        let height = (lines.len() as u16 + 2).min(area.height - 2);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        let deploy = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(" Deploy Preview "),
        );
        frame.render_widget(ratatui::widgets::Clear, popup_area);
        frame.render_widget(deploy, popup_area);
    }

    fn render_theme_picker(&self, frame: &mut ratatui::Frame, area: Rect) {
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                " Select Theme ",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        for (idx, theme) in self.themes.iter().enumerate() {
            let marker = if idx == self.theme_index { "> " } else { "  " };
            let current = if *theme == self.current_theme { " (current)" } else { "" };
            let style = if idx == self.theme_index {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            lines.push(Line::from(Span::styled(
                format!(" {marker}{theme}{current}"),
                style,
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " j/k:select  Enter:apply  Esc:cancel ",
            Style::default().fg(Color::DarkGray),
        )));

        let width = 40.min(area.width);
        let height = (lines.len() as u16 + 2).min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        let picker = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Theme "),
        );
        frame.render_widget(ratatui::widgets::Clear, popup_area);
        frame.render_widget(picker, popup_area);
    }

    fn render_deploy_status(&self, frame: &mut ratatui::Frame, area: Rect) {
        let Some(status) = &self.deploy_status else {
            return;
        };

        let mut lines = vec![
            Line::from(Span::styled(
                " Deploy Successful ",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        // Status contains push result + wrangler output separated by newline
        for line in status.lines() {
            // Parse wrangler table row if present
            if line.contains('│') {
                let fields: Vec<&str> = line.split('│').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                if fields.len() >= 5 {
                    lines.push(Line::from(format!(" Environment: {}", fields[1])));
                    lines.push(Line::from(format!(" Branch:      {}", fields[2])));
                    lines.push(Line::from(format!(" Commit:      {}", fields[3])));
                    lines.push(Line::from(format!(" URL:         {}", fields[4])));
                    if fields.len() > 5 {
                        lines.push(Line::from(format!(" Deployed:    {}", fields[5])));
                    }
                    continue;
                }
            }
            if !line.is_empty() {
                lines.push(Line::from(format!(" {line}")));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Press any key to close ",
            Style::default().fg(Color::DarkGray),
        )));

        let width = (area.width - 4).min(70);
        let height = (lines.len() as u16 + 2).min(area.height - 2);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        let status_widget = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title(" Deploy Status "),
        );
        frame.render_widget(ratatui::widgets::Clear, popup_area);
        frame.render_widget(status_widget, popup_area);
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        conn: &Connection,
    ) -> io::Result<Option<ListAction>> {
        // Deploy status modal
        if self.deploy_status.is_some() {
            self.deploy_status = None;
            return Ok(None);
        }

        // Error overlay mode
        if self.show_error {
            if matches!(code, KeyCode::Esc) {
                self.show_error = false;
            }
            return Ok(None);
        }

        // Delete confirmation mode
        if let Some(id) = self.confirm_delete {
            match code {
                KeyCode::Char('y') => {
                    self.confirm_delete = None;
                    if self.selected_article().is_some()
                        && db::delete_article(conn, id).is_ok()
                    {
                        self.flash = Some(("Deleted".into(), Instant::now()));
                        self.refresh(conn);
                    }
                }
                _ => {
                    self.confirm_delete = None;
                }
            }
            return Ok(None);
        }

        // Deploy preview mode (diff shown, awaiting confirm)
        if self.deploy_preview.is_some() {
            match code {
                KeyCode::Char('y') => {
                    self.start_push();
                }
                KeyCode::Esc => {
                    self.deploy_preview = None;
                }
                _ => {}
            }
            return Ok(None);
        }

        // Theme picker mode
        if self.show_theme_picker {
            match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if self.theme_index + 1 < self.themes.len() {
                        self.theme_index += 1;
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if self.theme_index > 0 {
                        self.theme_index -= 1;
                    }
                }
                KeyCode::Enter => {
                    if let Some(theme) = self.themes.get(self.theme_index) {
                        let theme = theme.clone();
                        if theme != self.current_theme {
                            if let Err(e) = ops::set_theme(&self.blog_dir, &theme) {
                                self.flash = Some((format!("Theme error: {e}"), Instant::now()));
                            } else {
                                self.current_theme = theme.clone();
                                self.flash = Some((format!("Theme set to {theme}"), Instant::now()));
                                if self.serving {
                                    self.stop_serve();
                                    self.start_serve();
                                }
                            }
                        }
                    }
                    self.show_theme_picker = false;
                }
                KeyCode::Esc => {
                    self.show_theme_picker = false;
                }
                _ => {}
            }
            return Ok(None);
        }

        // Help overlay mode
        if self.show_help {
            self.show_help = false;
            return Ok(None);
        }

        // Search mode
        if self.search_active {
            match code {
                KeyCode::Esc => {
                    self.search_active = false;
                    self.search_query.clear();
                    self.refresh(conn);
                }
                KeyCode::Enter => {
                    self.search_active = false;
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.refresh_search(conn);
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.refresh_search(conn);
                }
                _ => {}
            }
            return Ok(None);
        }

        // Normal mode
        match code {
            KeyCode::Char('q') => {
                self.stop_serve();
                return Ok(Some(ListAction::Quit));
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('G') => {
                if !self.articles.is_empty() {
                    self.table_state.select(Some(self.articles.len() - 1));
                }
            }
            KeyCode::Char('g') => {
                self.table_state.select(Some(0));
            }
            KeyCode::Enter => {
                if let Some(article) = self.selected_article() {
                    return Ok(Some(ListAction::Edit(article.id)));
                }
            }
            KeyCode::Char('n') => return Ok(Some(ListAction::New)),
            KeyCode::Char('p') => self.toggle_publish(conn),
            KeyCode::Char('i') => self.toggle_pin(conn),
            KeyCode::Char('d') => {
                if let Some(article) = self.selected_article() {
                    self.confirm_delete = Some(article.id);
                }
            }
            KeyCode::Char('b') => self.start_build(),
            KeyCode::Char('s') => {
                if self.serving {
                    self.stop_serve();
                } else {
                    self.start_serve();
                }
            }
            KeyCode::Char('o') => {
                if !self.serving {
                    self.start_serve();
                }
                let url = format!("http://localhost:{}", ops::SERVE_PORT);
                let _ = std::process::Command::new("open").arg(&url).spawn();
            }
            KeyCode::Char('D') => {
                if !self.deploying {
                    self.start_deploy();
                }
            }
            KeyCode::Char('t') => {
                if !self.themes.is_empty() {
                    self.theme_index = self.themes.iter().position(|t| t == &self.current_theme).unwrap_or(0);
                    self.show_theme_picker = true;
                }
            }
            KeyCode::Char('e') => {
                if self.last_error.is_some() {
                    self.show_error = true;
                }
            }
            KeyCode::Char('/') => {
                self.search_active = true;
                self.search_query.clear();
            }
            KeyCode::Char('?') => {
                self.show_help = true;
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.stop_serve();
                return Ok(Some(ListAction::Quit));
            }
            _ => {}
        }
        Ok(None)
    }

    fn move_selection(&mut self, delta: i32) {
        if self.articles.is_empty() {
            return;
        }
        let current = self.table_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, self.articles.len() as i32 - 1) as usize;
        self.table_state.select(Some(next));
    }

    fn selected_article(&self) -> Option<&Article> {
        self.table_state
            .selected()
            .and_then(|i| self.articles.get(i))
    }

    fn toggle_publish(&mut self, conn: &Connection) {
        if let Some(article) = self.selected_article() {
            let id = article.id;
            let result = match article.status {
                Status::Draft => {
                    db::publish(conn, id).map(|_| "Published")
                }
                Status::Published => {
                    db::unpublish(conn, id).map(|_| "Unpublished")
                }
            };
            if let Ok(msg) = result {
                self.flash = Some((msg.into(), Instant::now()));
                self.refresh(conn);
            }
        }
    }

    fn toggle_pin(&mut self, conn: &Connection) {
        if let Some(article) = self.selected_article() {
            let id = article.id;
            let was_pinned = article.pinned;
            let result = if was_pinned {
                db::unpin(conn, id).map(|_| "Unpinned")
            } else {
                db::pin(conn, id).map(|_| "Pinned")
            };
            if let Ok(msg) = result {
                self.flash = Some((msg.into(), Instant::now()));
                self.refresh(conn);
            }
        }
    }

    pub fn refresh(&mut self, conn: &Connection) {
        self.refresh_with_selection(conn, true);
    }

    fn refresh_search(&mut self, conn: &Connection) {
        self.refresh_with_selection(conn, false);
    }

    fn refresh_with_selection(&mut self, conn: &Connection, keep_selection: bool) {
        let search = if self.search_query.is_empty() {
            None
        } else {
            Some(self.search_query.as_str())
        };
        let prev_idx = self.table_state.selected().unwrap_or(0);
        self.articles = db::list_articles(conn, search).unwrap_or_default();
        if self.articles.is_empty() {
            self.table_state.select(None);
        } else if keep_selection {
            let idx = prev_idx.min(self.articles.len().saturating_sub(1));
            self.table_state.select(Some(idx));
        } else {
            self.table_state.select(Some(0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> crate::engine::config::BlogConfig {
        crate::engine::config::BlogConfig {
            title: "Test".into(),
            subtitle: None,
            url: "https://test.com".into(),
            author: "A".into(),
            lang: "en".into(),
            articles_path: None,
            theme: Some("paper".into()),
            analytics_id: None,
            license: None,
            license_url: None,
            og_image: None,
            deploy_dir: None,
            tags: None,
            links: None,
            guides: None,
        }
    }

    fn make_list_view() -> ListView {
        ListView::new(PathBuf::from("blogs/test"), vec![], &test_config())
    }

    #[test]
    fn poll_build_result_clears_building_flag_on_success() {
        let mut view = make_list_view();
        view.building = true;
        *view.build_result.lock().unwrap() = Some(Ok("Built 1 articles in 0.1s".into()));
        view.poll_build_result();
        assert!(!view.building);
    }

    #[test]
    fn poll_build_result_sets_flash_on_success() {
        let mut view = make_list_view();
        view.building = true;
        *view.build_result.lock().unwrap() = Some(Ok("Built 3 articles in 0.5s".into()));
        view.poll_build_result();
        let (msg, _) = view.flash.as_ref().unwrap();
        assert_eq!(msg, "Built 3 articles in 0.5s");
    }

    #[test]
    fn poll_build_result_stores_error_on_failure() {
        let mut view = make_list_view();
        view.building = true;
        *view.build_result.lock().unwrap() =
            Some(Err("engine exploded\ndetails here".into()));
        view.poll_build_result();
        assert!(!view.building);
        assert_eq!(
            view.last_error.as_deref(),
            Some("engine exploded\ndetails here")
        );
    }

    #[test]
    fn poll_build_result_shows_first_error_line_in_flash() {
        let mut view = make_list_view();
        view.building = true;
        *view.build_result.lock().unwrap() =
            Some(Err("first line\nsecond line\nthird".into()));
        view.poll_build_result();
        let (msg, _) = view.flash.as_ref().unwrap();
        assert_eq!(msg, "Build failed: first line");
    }

    #[test]
    fn start_build_guards_against_double_build() {
        let mut view = make_list_view();
        view.building = true;
        let slot_before = Arc::strong_count(&view.build_result);
        view.start_build();
        // No new thread spawned, Arc count unchanged
        assert_eq!(Arc::strong_count(&view.build_result), slot_before);
    }

    #[test]
    fn poll_build_result_is_noop_when_no_result() {
        let mut view = make_list_view();
        view.building = true;
        view.poll_build_result();
        // building flag unchanged, no flash set
        assert!(view.building);
        assert!(view.flash.is_none());
    }

    #[test]
    fn poll_build_result_sets_last_built_on_success() {
        let mut view = make_list_view();
        view.building = true;
        *view.build_result.lock().unwrap() = Some(Ok("Built 1 articles in 0.1s".into()));
        view.poll_build_result();
        assert!(view.last_built.is_some());
    }

    #[test]
    fn stop_serve_clears_serving_flag() {
        let mut view = make_list_view();
        view.serving = true;
        view.stop_serve();
        assert!(!view.serving);
    }

    #[test]
    fn poll_build_result_spawns_serve_when_serve_after_build() {
        let mut view = make_list_view();
        view.building = true;
        view.serve_after_build = true;
        *view.build_result.lock().unwrap() = Some(Ok("Built 1 articles in 0.1s".into()));
        view.poll_build_result();
        assert!(view.serving);
        assert!(!view.serve_after_build);
        // Clean up the spawned server
        view.stop_serve();
    }

    #[test]
    fn poll_build_result_clears_serve_after_build_on_error() {
        let mut view = make_list_view();
        view.building = true;
        view.serve_after_build = true;
        *view.build_result.lock().unwrap() = Some(Err("build failed".into()));
        view.poll_build_result();
        assert!(!view.serving);
        assert!(!view.serve_after_build);
    }

    #[test]
    fn stop_serve_is_noop_when_not_serving() {
        let mut view = make_list_view();
        view.stop_serve();
        assert!(!view.serving);
    }
}
