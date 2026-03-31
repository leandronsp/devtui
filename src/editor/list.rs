use std::io;
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

/// Action returned from the list view to the dispatcher.
pub enum ListAction {
    Edit(i64),
    New,
    Quit,
}

pub struct ListView {
    articles: Vec<Article>,
    table_state: TableState,
    search_query: String,
    search_active: bool,
    flash: Option<(&'static str, Instant)>,
    show_help: bool,
    confirm_delete: Option<i64>,
}

impl ListView {
    pub fn new(articles: Vec<Article>) -> Self {
        let mut table_state = TableState::default();
        if !articles.is_empty() {
            table_state.select(Some(0));
        }
        Self {
            articles,
            table_state,
            search_query: String::new(),
            search_active: false,
            flash: None,
            show_help: false,
            confirm_delete: None,
        }
    }

    pub fn run(
        &mut self,
        terminal: &mut ratatui::DefaultTerminal,
        conn: &Connection,
    ) -> io::Result<ListAction> {
        loop {
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
        })?;
        Ok(())
    }

    fn render_header(&self, frame: &mut ratatui::Frame, area: Rect) {
        let mut spans = vec![
            Span::styled(" DevTUI CMS ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(format!("  {} articles", self.articles.len())),
        ];

        if let Some((msg, _)) = &self.flash {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                *msg,
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
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
            Constraint::Length(4),
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
                " j/k:nav  Enter:edit  n:new  p:publish  i:pin  d:delete  /:search  ?:help  q:quit",
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

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        conn: &Connection,
    ) -> io::Result<Option<ListAction>> {
        // Delete confirmation mode
        if let Some(id) = self.confirm_delete {
            match code {
                KeyCode::Char('y') => {
                    self.confirm_delete = None;
                    if self.selected_article().is_some()
                        && db::delete_article(conn, id).is_ok()
                    {
                        self.flash = Some(("Deleted", Instant::now()));
                        self.refresh(conn);
                    }
                }
                _ => {
                    self.confirm_delete = None;
                }
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
            KeyCode::Char('q') => return Ok(Some(ListAction::Quit)),
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
            KeyCode::Char('/') => {
                self.search_active = true;
                self.search_query.clear();
            }
            KeyCode::Char('?') => {
                self.show_help = true;
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
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
                self.flash = Some((msg, Instant::now()));
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
                self.flash = Some((msg, Instant::now()));
                self.refresh(conn);
            }
        }
    }

    fn refresh(&mut self, conn: &Connection) {
        let search = if self.search_query.is_empty() {
            None
        } else {
            Some(self.search_query.as_str())
        };
        let selected_idx = self.table_state.selected().unwrap_or(0);
        self.articles = db::list_articles(conn, search).unwrap_or_default();
        if self.articles.is_empty() {
            self.table_state.select(None);
        } else {
            let idx = selected_idx.min(self.articles.len().saturating_sub(1));
            self.table_state.select(Some(idx));
        }
    }

    fn refresh_search(&mut self, conn: &Connection) {
        let search = if self.search_query.is_empty() {
            None
        } else {
            Some(self.search_query.as_str())
        };
        self.articles = db::list_articles(conn, search).unwrap_or_default();
        if self.articles.is_empty() {
            self.table_state.select(None);
        } else {
            self.table_state.select(Some(0));
        }
    }
}
