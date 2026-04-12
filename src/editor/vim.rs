use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tui_term::widget::PseudoTerminal;

use super::chrome::{ChromeHandle, ChromeResult};
use super::kitty::KittyImage;
use super::preview;


const DRAFT_PATH: &str = "draft.md";
const CONTENT_TMP: &str = "/tmp/devtui-content";

#[derive(Clone, Copy, Debug, PartialEq)]
enum PreviewMode {
    Text,
    Html,
}

#[derive(Clone, Copy, PartialEq)]
enum SplitLayout {
    Vertical,   // editor left, preview right (50/50)
    EditorOnly, // full editor, no preview
}

pub enum EditorResult {
    Quit,
}

pub struct HtmlPreviewConfig {
    pub css: String,
    pub article_tpl: String,
    pub blog_config: crate::engine::config::BlogConfig,
}

pub fn run(
    terminal: &mut ratatui::DefaultTerminal,
    file_path: PathBuf,
    html_config: Option<&HtmlPreviewConfig>,
    chrome: Option<&ChromeHandle>,
    picker: &ratatui_image::picker::Picker,
) -> io::Result<(EditorResult, String)> {
    if !file_path.exists() {
        std::fs::write(&file_path, "")?;
    }

    let file_path = std::fs::canonicalize(&file_path)?;
    let initial_content = std::fs::read_to_string(&file_path).unwrap_or_default();

    let term_size = terminal.size()?;
    let vim_cols = (term_size.width / 2).saturating_sub(2);
    let vim_rows = term_size.height.saturating_sub(3);

    let pty_system = NativePtySystem::default();
    let pty_pair = pty_system
        .openpty(PtySize {
            rows: vim_rows,
            cols: vim_cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| io::Error::other(e.to_string()))?;

    let file_str = file_path.to_str().unwrap_or(DRAFT_PATH);

    let content_autocmd = format!(
        "autocmd TextChanged,TextChangedI,BufWritePost * call writefile(getline(1,'$'),'{}')",
        CONTENT_TMP
    );
    let initial_write = format!("call writefile(getline(1,'$'),'{}')", CONTENT_TMP);

    let mut cmd = CommandBuilder::new("vim");
    cmd.args([
        "-u", "NONE",
        "-N",
        "--cmd", "set shortmess=aFIoOstTWcCS",
        "-c", "syntax on | set filetype=markdown",
        "-c", "set noswapfile noruler noshowmode noshowcmd laststatus=0 updatetime=150 tabstop=2 shiftwidth=2 expandtab title titlestring=%{line('w0')}:%{line('.')}:%{line('$')}:%{mode()}:%{&modified}",
        "-c", "nnoremap u :silent! undo<CR>",
        "-c", "nnoremap <C-r> :silent! redo<CR>",
        "-c", "autocmd BufWritePost * call timer_start(1,{->execute('redraw!')})",
        "-c", &content_autocmd,
        "-c", &initial_write,
        file_str,
    ]);

    let mut child = pty_pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| io::Error::other(e.to_string()))?;

    drop(pty_pair.slave);

    let mut pty_writer = pty_pair
        .master
        .take_writer()
        .map_err(|e| io::Error::other(e.to_string()))?;

    let pty_reader = pty_pair
        .master
        .try_clone_reader()
        .map_err(|e| io::Error::other(e.to_string()))?;

    let parser = Arc::new(RwLock::new(vt100::Parser::new(vim_rows, vim_cols, 0)));
    let running = Arc::new(AtomicBool::new(true));
    let vim_exited = Arc::new(AtomicBool::new(false));

    let parser_w = Arc::clone(&parser);
    let running_r = Arc::clone(&running);
    let exited_w = Arc::clone(&vim_exited);
    let reader_handle = thread::spawn(move || {
        read_pty(pty_reader, parser_w, running_r);
        exited_w.store(true, Ordering::Relaxed);
    });

    // Content swap buffer: thread writes new content here, main loop takes it.
    // Using Mutex<Option<String>> instead of RwLock<String> to avoid read/write contention (blink source).
    let content_swap = Arc::new(Mutex::new(Some(initial_content)));
    let content_swap_w = Arc::clone(&content_swap);
    let running_p = Arc::clone(&running);
    let content_handle = thread::spawn(move || {
        let mut last_content = String::new();
        while running_p.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(100));
            if let Ok(content) = std::fs::read_to_string(CONTENT_TMP) {
                if content != last_content {
                    last_content.clone_from(&content);
                    if let Ok(mut slot) = content_swap_w.lock() {
                        *slot = Some(content);
                    }
                }
            }
        }
    });

    let file_display = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| file_path.display().to_string());

    run_loop(
        terminal,
        &parser,
        &mut pty_writer,
        &pty_pair.master,
        &vim_exited,
        &file_display,
        &content_swap,
        html_config,
        chrome,
        picker,
    )?;

    // Read final content from the actual file vim was editing.
    // This respects :q! (file unchanged) vs :wq (file saved).
    let final_content = std::fs::read_to_string(&file_path).unwrap_or_default();

    running.store(false, Ordering::Relaxed);
    drop(pty_pair.master);
    let _ = child.kill();
    let _ = reader_handle.join();
    let _ = content_handle.join();
    let _ = std::fs::remove_file(CONTENT_TMP);

    Ok((EditorResult::Quit, final_content))
}

fn read_pty(
    mut reader: Box<dyn Read + Send>,
    parser: Arc<RwLock<vt100::Parser>>,
    running: Arc<AtomicBool>,
) {
    let mut buf = [0u8; 4096];
    while running.load(Ordering::Relaxed) {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if let Ok(mut p) = parser.write() {
                    p.process(&buf[..n]);
                }
            }
            Err(_) => break,
        }
    }
}

/// Parsed vim state from titlestring: w0:cursor:total:mode
struct VimState {
    cursor_line: usize,
    total_lines: usize,
    mode: &'static str,
    modified: bool,
}

fn parse_title(parser: &Arc<RwLock<vt100::Parser>>) -> VimState {
    if let Ok(p) = parser.read() {
        let title = p.screen().title().to_string();
        let parts: Vec<&str> = title.trim().split(':').collect();
        if parts.len() >= 5 {
            let cursor_line = parts[1].parse::<usize>().unwrap_or(1);
            let total_lines = parts[2].parse::<usize>().unwrap_or(1);
            let mode = match parts[3] {
                "i" => "INSERT",
                "v" | "V" | "\x16" => "VISUAL",
                "R" => "REPLACE",
                "c" => "COMMAND",
                _ => "NORMAL",
            };
            let modified = parts[4] == "1";
            return VimState { cursor_line, total_lines, mode, modified };
        }
    }
    VimState { cursor_line: 1, total_lines: 1, mode: "NORMAL", modified: false }
}

/// Extract the last line from the vim PTY screen (where vim shows messages).
/// Returns non-empty string only when vim is showing a message (e.g. "written", "E37", etc).
fn mode_style(mode: &str) -> (&str, Style) {
    match mode {
        "INSERT" => (" INSERT ", Style::default().fg(Color::Black).bg(Color::Green)),
        "VISUAL" => (" VISUAL ", Style::default().fg(Color::Black).bg(Color::Magenta)),
        "COMMAND" => (" COMMAND ", Style::default().fg(Color::Black).bg(Color::Yellow)),
        "REPLACE" => (" REPLACE ", Style::default().fg(Color::Black).bg(Color::Red)),
        _ => (" NORMAL ", Style::default().fg(Color::Black).bg(Color::Blue)),
    }
}

/// Calculate vim PTY dimensions based on layout and terminal size.
fn vim_size(layout: SplitLayout, width: u16, height: u16) -> (u16, u16) {
    let rows = height.saturating_sub(3); // status bar + borders
    match layout {
        SplitLayout::Vertical => ((width / 2).saturating_sub(2), rows),
        SplitLayout::EditorOnly => (width.saturating_sub(2), rows),
    }
}

/// All mutable state owned by the run loop. Extracted to allow splitting the loop body
/// into focused methods without passing a dozen locals through each call.
struct RunLoopState {
    last_content: String,
    cached_lines: Vec<Line<'static>>,
    cached_offsets: Vec<u16>,
    preview_mode: PreviewMode,
    kitty_image: Option<KittyImage>,
    split_layout: SplitLayout,
    preview_scroll: u16,
    chrome_available: bool,
    html_rendering: bool,
    chrome_error: Option<String>,
    /// True when content changed since last HTML render was dispatched.
    html_stale: bool,
    /// Set when content changes; cleared after 100ms debounce fires.
    content_changed_at: Option<std::time::Instant>,
    preview_stale: bool,
    flash: Option<(&'static str, std::time::Instant)>,
    /// Tracks modified state across frames to detect :w saves.
    was_modified: bool,
    /// Blog author from blog.toml; rendered in preview header when post has no
    /// `author:` frontmatter field.
    blog_author: Option<String>,
}

impl RunLoopState {
    fn new(chrome_available: bool, blog_author: Option<String>) -> Self {
        Self {
            last_content: String::new(),
            cached_lines: Vec::new(),
            cached_offsets: Vec::new(),
            preview_mode: PreviewMode::Text,
            kitty_image: None,
            split_layout: SplitLayout::Vertical,
            preview_scroll: 0,
            chrome_available,
            html_rendering: false,
            chrome_error: None,
            html_stale: true,
            content_changed_at: None,
            preview_stale: false,
            flash: None,
            was_modified: false,
            blog_author,
        }
    }

    /// Pick up new content from the swap buffer (non-blocking). Marks preview and HTML
    /// as stale when content changes so the debounce timer starts.
    fn poll_content_swap(&mut self, content_swap: &Arc<Mutex<Option<String>>>) {
        if let Ok(mut slot) = content_swap.try_lock() {
            if let Some(new_content) = slot.take() {
                if new_content != self.last_content {
                    self.last_content = new_content;
                    self.content_changed_at = Some(std::time::Instant::now());
                    self.preview_stale = true;
                    self.html_stale = true;
                }
            }
        }
    }

    /// Debounced preview re-render: waits 100ms after the last content change before
    /// rebuilding the ratatui line cache, avoiding re-renders on every keystroke.
    fn poll_preview_render(&mut self) -> bool {
        if !self.preview_stale {
            return false;
        }
        let Some(changed_at) = self.content_changed_at else { return false };
        if changed_at.elapsed() < std::time::Duration::from_millis(100) {
            return false;
        }

        self.preview_stale = false;
        self.content_changed_at = None;

        if self.split_layout != SplitLayout::EditorOnly {
            let (lines, offsets) = preview::render_with_offsets(&self.last_content, self.blog_author.as_deref());
            self.cached_offsets = offsets;
            self.cached_lines = lines;
        }
        true
    }

    /// Pick up a pending Chrome screenshot result (non-blocking). On success, stores the
    /// new KittyImage and positions the scroll to follow the current cursor. On error,
    /// stores the error message for display in the preview pane.
    fn poll_chrome_result(
        &mut self,
        vim_state: &VimState,
        terminal: &ratatui::DefaultTerminal,
        chrome: Option<&ChromeHandle>,
        picker: &ratatui_image::picker::Picker,
    ) {
        if self.preview_mode != PreviewMode::Html {
            return;
        }
        let Some(ch) = chrome else { return };
        let Some(result) = ch.try_recv() else { return };

        match result {
            ChromeResult::Image(png_bytes) => {
                match image::load_from_memory(&png_bytes) {
                    Ok(img) => {
                        let font_h = picker.font_size().1 as u32;
                        let w = img.width();
                        let h = img.height();
                        drop(img);
                        let image_rows = (h / font_h.max(1)) as u16;
                        log::debug!("Chrome image: {}x{} px, font_h={}, image_rows={}", w, h, font_h, image_rows);
                        drop(self.kitty_image.take());
                        match KittyImage::transmit(&png_bytes, w, h) {
                            Ok(mut ki) => {
                                ki.set_max_rows(image_rows);
                                // Follow cursor to new image position
                                let visible = terminal.size()
                                    .map(|s| s.height.saturating_sub(4))
                                    .unwrap_or(30);
                                let ratio = vim_state.cursor_line as f64
                                    / vim_state.total_lines.max(1) as f64;
                                let target = (ratio * ki.max_rows() as f64) as u16;
                                ki.scroll_row = target.saturating_sub(visible / 2);
                                ki.scroll_row = ki.scroll_row.min(ki.max_rows().saturating_sub(visible));
                                self.kitty_image = Some(ki);
                                self.html_rendering = false;
                                self.chrome_error = None;
                            }
                            Err(e) => {
                                log::error!("Kitty transmit failed: {}", e);
                                self.chrome_error = Some(format!("Kitty transmit failed: {e}"));
                                self.html_rendering = false;
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to decode Chrome PNG: {}", e);
                    }
                }
            }
            ChromeResult::Error(msg) => {
                self.chrome_error = Some(msg);
                self.html_rendering = false;
            }
        }
    }

    /// Detect a `:w` save (modified flag transitions true -> false). Shows "saved" flash,
    /// reopens the preview pane if it was hidden, and triggers a preview refresh.
    #[allow(clippy::borrowed_box, clippy::too_many_arguments)]
    fn handle_save_detected(
        &mut self,
        vim_state: &VimState,
        terminal: &mut ratatui::DefaultTerminal,
        pty_master: &Box<dyn portable_pty::MasterPty + Send>,
        parser: &Arc<RwLock<vt100::Parser>>,
        html_config: Option<&HtmlPreviewConfig>,
        chrome: Option<&ChromeHandle>,
        picker: &ratatui_image::picker::Picker,
    ) -> io::Result<()> {
        if self.was_modified && !vim_state.modified {
            self.flash = Some(("saved", std::time::Instant::now()));

            // Open preview if hidden
            if self.split_layout == SplitLayout::EditorOnly {
                self.split_layout = SplitLayout::Vertical;
                resize_pty(self.split_layout, terminal, pty_master, parser)?;
            }

            // Refresh text preview
            let (lines, offsets) = preview::render_with_offsets(&self.last_content, self.blog_author.as_deref());
            self.cached_offsets = offsets;
            self.cached_lines = lines;

            // Refresh HTML preview
            if self.preview_mode == PreviewMode::Html {
                if let (Some(cfg), Some(ch)) = (html_config, chrome) {
                    let html = render_html_preview(&self.last_content, cfg);
                    let vw = viewport_width(terminal, picker);
                    ch.send_html(html, vw);
                    self.html_rendering = true;
                    self.html_stale = false;
                }
            }

            // Follow cursor in text preview (HTML follows when new image arrives)
            if self.preview_mode != PreviewMode::Html {
                let size = terminal.size()?;
                let pw = (size.width / 2).saturating_sub(2) as usize;
                let vr = size.height.saturating_sub(4) as usize;
                self.preview_scroll = follow_editor_cursor(
                    vim_state.cursor_line, vim_state.total_lines,
                    &self.cached_lines, &self.cached_offsets, pw, vr,
                );
            }
        }
        self.was_modified = vim_state.modified;
        Ok(())
    }

    /// Auto-dismiss the "saved" flash after 2 seconds.
    fn manage_flash(&mut self) {
        if let Some((_, at)) = &self.flash {
            if at.elapsed() > std::time::Duration::from_secs(2) {
                self.flash = None;
            }
        }
    }

    /// Return the text shown in the editor title bar: flash message, modified marker, or empty.
    fn title_message(&self, vim_state: &VimState) -> &'static str {
        if let Some((msg, _)) = &self.flash {
            msg
        } else if vim_state.modified {
            "[+]"
        } else {
            ""
        }
    }

    /// Compute scroll info needed for rendering: pane width, visible rows, max scroll,
    /// and clamped scroll. Also auto-follows the cursor when content just updated.
    fn calculate_scroll(
        &mut self,
        terminal: &ratatui::DefaultTerminal,
        vim_state: &VimState,
        content_updated: bool,
    ) -> io::Result<ScrollInfo> {
        let pane_width = match self.split_layout {
            SplitLayout::Vertical => (terminal.size()?.width / 2).saturating_sub(2),
            SplitLayout::EditorOnly => 1,
        } as usize;
        let visual_lines: usize = self.cached_lines.iter().map(|line| {
            let w = line.width();
            if w == 0 { 1 } else { w.div_ceil(pane_width) }
        }).sum();
        let visible_rows = terminal.size()?.height.saturating_sub(4) as usize;
        let max_scroll = visual_lines.saturating_sub(visible_rows) as u16;

        // Auto-follow cursor in text preview when content changes
        if content_updated && self.preview_mode == PreviewMode::Text && self.split_layout == SplitLayout::Vertical {
            self.preview_scroll = follow_editor_cursor(
                vim_state.cursor_line, vim_state.total_lines,
                &self.cached_lines, &self.cached_offsets, pane_width, visible_rows,
            );
        }

        Ok(ScrollInfo { pane_width, visible_rows, max_scroll, clamped_scroll: self.preview_scroll.min(max_scroll) })
    }

    /// Draw one frame: editor pane, optional preview pane, and the status bar.
    fn draw_frame(
        &self,
        terminal: &mut ratatui::DefaultTerminal,
        parser: &Arc<RwLock<vt100::Parser>>,
        vim_state: &VimState,
        scroll: &ScrollInfo,
        file_display: &str,
        html_config: Option<&HtmlPreviewConfig>,
    ) -> io::Result<Rect> {
        let (mode_label, mode_st) = mode_style(vim_state.mode);
        let title_message = self.title_message(vim_state);
        let layout_label = match self.split_layout {
            SplitLayout::Vertical => "|",
            SplitLayout::EditorOnly => "[]",
        };
        let preview_mode_label = match self.preview_mode {
            PreviewMode::Text => "TEXT",
            PreviewMode::Html => "HTML",
        };

        let mut preview_area = Rect::default();
        terminal.draw(|frame| {
            let area = frame.area();
            log::debug!("frame.area: {:?}", area);
            let status_split = Layout::vertical([
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

            let main_area = status_split[0];
            let status_area = status_split[1];

            let post_title = super::preview::frontmatter_field(&self.last_content, "title");
            let is_draft = super::preview::frontmatter_field(&self.last_content, "status")
                .map(|s| s.eq_ignore_ascii_case("draft"))
                .unwrap_or(false);

            match self.split_layout {
                SplitLayout::Vertical => {
                    let panes = Layout::horizontal([
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                    ])
                    .split(main_area);
                    render_editor(frame, parser, mode_label, mode_st, title_message, post_title.as_deref(), is_draft, panes[0]);
                    render_preview(
                        frame, &self.cached_lines, scroll.clamped_scroll, self.preview_mode,
                        preview_mode_label, &self.kitty_image,
                        self.html_rendering, &self.chrome_error, panes[1],
                    );
                    preview_area = panes[1];
                }
                SplitLayout::EditorOnly => {
                    render_editor(frame, parser, mode_label, mode_st, title_message, post_title.as_deref(), is_draft, main_area);
                }
            }

            // Status bar
            let chrome_hint = if !self.chrome_available {
                ""
            } else if self.preview_mode == PreviewMode::Html {
                " ^P:text"
            } else {
                " ^P:html"
            };
            let browser_hint = if html_config.is_some() { " ^O:browser" } else { "" };
            let scroll_hint = if self.split_layout != SplitLayout::EditorOnly {
                " ^T:follow-cursor ^J/^K:scroll"
            } else {
                ""
            };
            let status = Line::from(Span::styled(
                format!(
                    " {} | DevTUI [{layout_label}] ^G:layout{chrome_hint}{browser_hint}{scroll_hint}",
                    file_display,
                ),
                Style::default().fg(Color::DarkGray),
            ));
            frame.render_widget(Paragraph::new(status), status_area);
        })?;

        Ok(preview_area)
    }

    /// Handle a single key event. Returns `true` if the loop should break (vim exited via write failure).
    #[allow(clippy::borrowed_box, clippy::too_many_arguments)]
    fn dispatch_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        terminal: &mut ratatui::DefaultTerminal,
        pty_writer: &mut Box<dyn Write + Send>,
        pty_master: &Box<dyn portable_pty::MasterPty + Send>,
        parser: &Arc<RwLock<vt100::Parser>>,
        vim_state: &VimState,
        scroll: &ScrollInfo,
        html_config: Option<&HtmlPreviewConfig>,
        chrome: Option<&ChromeHandle>,
        picker: &ratatui_image::picker::Picker,
    ) -> io::Result<bool> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        // Ctrl+G: toggle layout
        if key.code == KeyCode::Char('g') && ctrl {
            self.split_layout = match self.split_layout {
                SplitLayout::Vertical => SplitLayout::EditorOnly,
                SplitLayout::EditorOnly => SplitLayout::Vertical,
            };
            resize_pty(self.split_layout, terminal, pty_master, parser)?;
            // Re-render preview content when coming back from EditorOnly
            if self.split_layout != SplitLayout::EditorOnly && self.cached_lines.is_empty() {
                let (lines, offsets) = preview::render_with_offsets(&self.last_content, self.blog_author.as_deref());
                self.cached_offsets = offsets;
                self.cached_lines = lines;
            }
            return Ok(false);
        }

        // Ctrl+T: follow editor cursor in preview
        if key.code == KeyCode::Char('t') && ctrl && self.split_layout != SplitLayout::EditorOnly {
            if self.preview_mode == PreviewMode::Html {
                if let Some(ref mut ki) = self.kitty_image {
                    let visible = terminal.size()?.height.saturating_sub(4);
                    let ratio = vim_state.cursor_line as f64 / vim_state.total_lines.max(1) as f64;
                    let target_row = (ratio * ki.max_rows() as f64) as u16;
                    ki.scroll_row = target_row.saturating_sub(visible / 2);
                    ki.scroll_row = ki.scroll_row.min(ki.max_rows().saturating_sub(visible));
                }
            } else {
                self.preview_scroll = follow_editor_cursor(
                    vim_state.cursor_line, vim_state.total_lines,
                    &self.cached_lines, &self.cached_offsets, scroll.pane_width, scroll.visible_rows,
                );
            }
            return Ok(false);
        }

        // Ctrl+P: toggle preview mode (text/html)
        if key.code == KeyCode::Char('p') && ctrl && self.chrome_available {
            log::info!("Ctrl+P: toggling preview mode (current={:?})", self.preview_mode);
            self.preview_mode = match self.preview_mode {
                PreviewMode::Text => {
                    if self.html_stale {
                        if let (Some(cfg), Some(ch)) = (html_config, chrome) {
                            let html = render_html_preview(&self.last_content, cfg);
                            let vw = viewport_width(terminal, picker);
                            ch.send_html(html, vw);
                            self.html_rendering = true;
                            self.html_stale = false;
                        }
                    }
                    PreviewMode::Html
                }
                PreviewMode::Html => PreviewMode::Text,
            };
            return Ok(false);
        }

        // Ctrl+O: open in browser
        if key.code == KeyCode::Char('o') && ctrl {
            if let Some(cfg) = html_config {
                let html = render_html_preview(&self.last_content, cfg);
                let tmp_html = std::env::temp_dir().join("devtui-preview.html");
                let _ = std::fs::write(&tmp_html, &html);
                let _ = std::process::Command::new("open").arg(&tmp_html).spawn();
            }
            return Ok(false);
        }

        // Ctrl+J: scroll preview down
        if key.code == KeyCode::Char('j') && ctrl && self.split_layout != SplitLayout::EditorOnly {
            if self.preview_mode == PreviewMode::Html {
                if let Some(ref mut ki) = self.kitty_image {
                    let visible = terminal.size()?.height.saturating_sub(4);
                    ki.scroll_down(5, visible);
                }
            } else {
                self.preview_scroll = self.preview_scroll.saturating_add(3).min(scroll.max_scroll);
            }
            return Ok(false);
        }

        // Ctrl+K: scroll preview up
        if key.code == KeyCode::Char('k') && ctrl && self.split_layout != SplitLayout::EditorOnly {
            if self.preview_mode == PreviewMode::Html {
                if let Some(ref mut ki) = self.kitty_image {
                    ki.scroll_up(5);
                }
            } else {
                self.preview_scroll = self.preview_scroll.saturating_sub(3);
            }
            return Ok(false);
        }

        // Pass everything else to vim
        if let Some(bytes) = key_to_bytes(key.code, key.modifiers) {
            if pty_writer.write_all(&bytes).is_err() {
                return Ok(true); // signal break
            }
            let _ = pty_writer.flush();
        }
        Ok(false)
    }
}

/// Scroll info computed once per frame and shared across draw and key handling.
struct ScrollInfo {
    pane_width: usize,
    visible_rows: usize,
    max_scroll: u16,
    clamped_scroll: u16,
}

#[allow(clippy::too_many_arguments, clippy::borrowed_box)]
fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    parser: &Arc<RwLock<vt100::Parser>>,
    pty_writer: &mut Box<dyn Write + Send>,
    pty_master: &Box<dyn portable_pty::MasterPty + Send>,
    vim_exited: &Arc<AtomicBool>,
    file_display: &str,
    content_swap: &Arc<Mutex<Option<String>>>,
    html_config: Option<&HtmlPreviewConfig>,
    chrome: Option<&ChromeHandle>,
    picker: &ratatui_image::picker::Picker,
) -> io::Result<()> {
    let blog_author = html_config.map(|c| c.blog_config.author.clone());
    let mut state = RunLoopState::new(chrome.is_some(), blog_author);

    loop {
        if vim_exited.load(Ordering::Relaxed) {
            break;
        }

        state.poll_content_swap(content_swap);
        let content_updated = state.poll_preview_render();
        let vim_state = parse_title(parser);
        state.poll_chrome_result(&vim_state, terminal, chrome, picker);
        state.handle_save_detected(&vim_state, terminal, pty_master, parser, html_config, chrome, picker)?;
        state.manage_flash();
        let scroll = state.calculate_scroll(terminal, &vim_state, content_updated)?;
        state.draw_frame(terminal, parser, &vim_state, &scroll, file_display, html_config)?;

        if event::poll(Duration::from_millis(30))? {
            match event::read()? {
                Event::Key(key) => {
                    if state.dispatch_key(key, terminal, pty_writer, pty_master, parser, &vim_state, &scroll, html_config, chrome, picker)? {
                        break;
                    }
                }
                Event::Resize(width, height) => {
                    let (cols, rows) = vim_size(state.split_layout, width, height);
                    let _ = pty_master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
                    if let Ok(mut p) = parser.write() {
                        p.set_size(rows, cols);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_editor(
    frame: &mut ratatui::Frame,
    parser: &Arc<RwLock<vt100::Parser>>,
    mode_label: &str,
    mode_st: Style,
    title_message: &str,
    post_title: Option<&str>,
    is_draft: bool,
    area: ratatui::layout::Rect,
) {
    let mut title_spans = vec![
        Span::styled(mode_label, mode_st),
        Span::raw(" EDITOR"),
    ];

    if is_draft {
        title_spans.push(Span::raw(" "));
        title_spans.push(Span::styled(
            "DRAFT",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    if !title_message.is_empty() {
        title_spans.push(Span::raw(" "));
        title_spans.push(Span::styled(
            title_message.to_string(),
            Style::default().fg(Color::Yellow),
        ));
    }

    if let Some(title) = post_title {
        let max_len = area.width.saturating_sub(30) as usize;
        let display = if title.len() > max_len && max_len > 3 {
            format!("{}...", &title[..max_len - 3])
        } else {
            title.to_string()
        };
        title_spans.push(Span::raw(" "));
        title_spans.push(Span::styled(
            display,
            Style::default().fg(Color::DarkGray),
        ));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Line::from(title_spans));

    if let Ok(p) = parser.read() {
        let pseudo_term = PseudoTerminal::new(p.screen()).block(block);
        frame.render_widget(pseudo_term, area);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_preview(
    frame: &mut ratatui::Frame,
    cached_lines: &[Line<'static>],
    scroll: u16,
    preview_mode: PreviewMode,
    mode_label: &str,
    kitty_image: &Option<KittyImage>,
    html_rendering: bool,
    chrome_error: &Option<String>,
    area: ratatui::layout::Rect,
) {
    let rendering_indicator = if html_rendering { " ..." } else { "" };
    let preview_title = format!(" PREVIEW [{mode_label}]{rendering_indicator} ");
    let preview_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(preview_title);

    match preview_mode {
        PreviewMode::Text => {
            let preview_widget = Paragraph::new(cached_lines.to_vec())
                .block(preview_block)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0));
            frame.render_widget(preview_widget, area);
        }
        PreviewMode::Html => {
            if let Some(ref err) = chrome_error {
                let error_msg = Paragraph::new(Line::from(Span::styled(
                    format!(" Chrome error: {err}"),
                    Style::default().fg(Color::Red),
                )))
                .block(preview_block)
                .wrap(Wrap { trim: false });
                frame.render_widget(error_msg, area);
            } else if let Some(ref ki) = kitty_image {
                let inner = preview_block.inner(area);
                frame.render_widget(preview_block, area);
                ki.render_placeholders(inner, frame.buffer_mut());
            } else {
                let msg = if html_rendering {
                    " Rendering..."
                } else {
                    " Press ^P to render HTML preview"
                };
                let loading = Paragraph::new(Line::from(Span::styled(
                    msg,
                    Style::default().fg(Color::DarkGray),
                )))
                .block(preview_block);
                frame.render_widget(loading, area);
            }
        }
    }
}



#[allow(clippy::borrowed_box)]
/// Calculate Chrome viewport width in CSS pixels based on current terminal size.
fn viewport_width(terminal: &ratatui::DefaultTerminal, picker: &ratatui_image::picker::Picker) -> u32 {
    let term_width = terminal.size().map(|s| s.width).unwrap_or(120);
    let font_w = picker.font_size().0 as u32;
    let pane_cols = term_width as u32 / 2;
    let physical = pane_cols * font_w;
    let scale: f64 = if font_w > 12 { 2.0 } else { 1.0 };
    (physical as f64 / scale) as u32
}

/// Calculate text preview scroll position to follow the cursor line.
fn follow_editor_cursor(
    cursor_line: usize,
    total_lines: usize,
    cached_lines: &[Line<'static>],
    cached_offsets: &[u16],
    pane_width: usize,
    visible_rows: usize,
) -> u16 {
    let ratio = cursor_line as f64 / total_lines.max(1) as f64;
    let visual_lines: usize = cached_lines.iter().map(|l| {
        let w = l.width();
        if w == 0 { 1 } else { w.div_ceil(pane_width) }
    }).sum();
    let target = if !cached_offsets.is_empty() {
        let idx = cursor_line.min(cached_offsets.len().saturating_sub(1));
        let rendered = cached_offsets[idx] as usize;
        cached_lines[..rendered.min(cached_lines.len())]
            .iter()
            .map(|l| {
                let w = l.width();
                if w == 0 { 1u16 } else { w.div_ceil(pane_width) as u16 }
            })
            .sum::<u16>()
    } else {
        (ratio * visual_lines as f64) as u16
    };
    let max_scroll = visual_lines.saturating_sub(visible_rows) as u16;
    target.saturating_sub(visible_rows as u16 / 2).min(max_scroll)
}

#[allow(clippy::borrowed_box)]
fn resize_pty(
    layout: SplitLayout,
    terminal: &mut ratatui::DefaultTerminal,
    pty_master: &Box<dyn portable_pty::MasterPty + Send>,
    parser: &Arc<RwLock<vt100::Parser>>,
) -> io::Result<()> {
    let term_size = terminal.size()?;
    let (cols, rows) = vim_size(layout, term_size.width, term_size.height);
    let _ = pty_master.resize(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    });
    if let Ok(mut p) = parser.write() {
        p.set_size(rows, cols);
    }
    Ok(())
}

fn render_html_preview(content: &str, config: &HtmlPreviewConfig) -> String {
    use crate::engine::{build, minify};

    let title = crate::engine::config::frontmatter("title", content)
        .unwrap_or_else(|| "Untitled".to_string());

    let html = build::render_preview_html(content, &title, &config.blog_config, &config.article_tpl);
    let minified_css = minify::minify_css(&config.css);
    minify::inline_css(&html, &minified_css)
}

pub fn key_to_bytes(code: KeyCode, modifiers: KeyModifiers) -> Option<Vec<u8>> {
    if modifiers.contains(KeyModifiers::CONTROL) {
        return match code {
            KeyCode::Char(c) => {
                let byte = (c as u8) & 0x1f;
                Some(vec![byte])
            }
            _ => None,
        };
    }

    match code {
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            Some(s.as_bytes().to_vec())
        }
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Backspace => Some(vec![127]),
        KeyCode::Tab => Some(vec![9]),
        KeyCode::BackTab => Some(vec![27, 91, 90]),
        KeyCode::Esc => Some(vec![27]),
        KeyCode::Up => Some(vec![27, 91, 65]),
        KeyCode::Down => Some(vec![27, 91, 66]),
        KeyCode::Right => Some(vec![27, 91, 67]),
        KeyCode::Left => Some(vec![27, 91, 68]),
        KeyCode::Home => Some(vec![27, 91, 72]),
        KeyCode::End => Some(vec![27, 91, 70]),
        KeyCode::PageUp => Some(vec![27, 91, 53, 126]),
        KeyCode::PageDown => Some(vec![27, 91, 54, 126]),
        KeyCode::Delete => Some(vec![27, 91, 51, 126]),
        KeyCode::Insert => Some(vec![27, 91, 50, 126]),
        KeyCode::F(n) => {
            let seq = match n {
                1 => vec![27, 79, 80],
                2 => vec![27, 79, 81],
                3 => vec![27, 79, 82],
                4 => vec![27, 79, 83],
                5 => vec![27, 91, 49, 53, 126],
                6 => vec![27, 91, 49, 55, 126],
                7 => vec![27, 91, 49, 56, 126],
                8 => vec![27, 91, 49, 57, 126],
                9 => vec![27, 91, 50, 48, 126],
                10 => vec![27, 91, 50, 49, 126],
                11 => vec![27, 91, 50, 51, 126],
                12 => vec![27, 91, 50, 52, 126],
                _ => return None,
            };
            Some(seq)
        }
        _ => None,
    }
}
