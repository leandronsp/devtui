use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tui_term::widget::PseudoTerminal;

use super::chrome::ChromePreview;
use super::preview;

const DRAFT_PATH: &str = "draft.md";
const CONTENT_TMP: &str = "/tmp/devtui-content";

#[derive(Clone, Copy, PartialEq)]
enum PreviewMode {
    Text,
    Html,
}

#[derive(Clone, Copy, PartialEq)]
enum SplitLayout {
    Vertical,   // editor left, preview right (50/50)
    Horizontal, // editor top, preview bottom (2/3 + 1/3)
    EditorOnly, // no preview
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
    chrome: Option<&ChromePreview>,
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
        "autocmd CursorHold,CursorHoldI,TextChanged,BufWritePost * call writefile(getline(1,'$'),'{}')",
        CONTENT_TMP
    );
    let initial_write = format!("call writefile(getline(1,'$'),'{}')", CONTENT_TMP);

    let mut cmd = CommandBuilder::new("vim");
    cmd.args([
        "-u", "NONE",
        "-N",
        "--cmd", "set shortmess=aFIoOstTWcCS",
        "-c", "set noswapfile noruler noshowmode noshowcmd laststatus=0 updatetime=150 tabstop=2 shiftwidth=2 expandtab title titlestring=%{line('w0')}:%{mode()}",
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

    let picker = ratatui_image::picker::Picker::halfblocks();

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
        &picker,
    )?;

    // Read final content from swap buffer or fall back to last file read
    let final_content = content_swap
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
        .or_else(|| std::fs::read_to_string(CONTENT_TMP).ok())
        .unwrap_or_default();

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

fn parse_title(parser: &Arc<RwLock<vt100::Parser>>) -> (usize, &'static str) {
    if let Ok(p) = parser.read() {
        let title = p.screen().title().to_string();
        if let Some((line_str, mode_char)) = title.trim().split_once(':') {
            let source_line = line_str.parse::<usize>().unwrap_or(1).saturating_sub(1);
            let mode = match mode_char {
                "i" => "INSERT",
                "v" | "V" | "\x16" => "VISUAL",
                "R" => "REPLACE",
                "c" => "COMMAND",
                _ => "NORMAL",
            };
            return (source_line, mode);
        }
    }
    (0, "NORMAL")
}

/// Extract the last line from the vim PTY screen (where vim shows messages).
/// Returns non-empty string only when vim is showing a message (e.g. "written", "E37", etc).
fn extract_vim_message(parser: &Arc<RwLock<vt100::Parser>>) -> String {
    if let Ok(p) = parser.read() {
        let screen = p.screen();
        let rows = screen.size().0;
        if rows == 0 {
            return String::new();
        }
        // Vim messages appear on the last row of the screen
        let last_row = rows - 1;
        let line = screen.contents_between(last_row, 0, last_row, screen.size().1);
        let trimmed = line.trim();
        // Filter out empty lines and tilde lines (vim's empty buffer indicator)
        if trimmed.is_empty() || trimmed == "~" {
            return String::new();
        }
        // Only show if it looks like a vim message (contains common patterns)
        if trimmed.starts_with('"')        // "file" written
            || trimmed.starts_with("E")     // E37: No write since last change
            || trimmed.contains("written")
            || trimmed.contains("change")
            || trimmed.contains("line")
            || trimmed.starts_with("--")    // -- INSERT --
            || trimmed.starts_with(':')     // command line
        {
            return trimmed.to_string();
        }
    }
    String::new()
}

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
        SplitLayout::Horizontal => (width.saturating_sub(2), rows / 2),
        SplitLayout::EditorOnly => (width.saturating_sub(2), rows),
    }
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
    chrome: Option<&ChromePreview>,
    picker: &ratatui_image::picker::Picker,
) -> io::Result<()> {
    let mut last_content = String::new();
    let mut cached_lines: Vec<Line<'static>> = Vec::new();
    let mut preview_mode = PreviewMode::Text;
    let mut cached_image_protocol: Option<ratatui_image::protocol::StatefulProtocol> = None;
    let mut split_layout = SplitLayout::Vertical;
    let mut preview_scroll: u16 = 0;
    let mut pending_html: Option<String> = None;
    let chrome_available = chrome.is_some();

    loop {
        if vim_exited.load(Ordering::Relaxed) {
            break;
        }

        // Pick up new content from swap buffer (non-blocking take)
        if let Ok(mut slot) = content_swap.try_lock() {
            if let Some(new_content) = slot.take() {
                if new_content != last_content {
                    last_content = new_content;

                    if split_layout != SplitLayout::EditorOnly {
                        let (lines, _) = preview::render_with_offsets(&last_content);
                        cached_lines = lines;
                    }

                    if preview_mode == PreviewMode::Html {
                        if let Some(cfg) = html_config {
                            pending_html = Some(render_html_preview(&last_content, cfg));
                        }
                    }
                }
            }
        }

        // Chrome screenshot on main thread
        if preview_mode == PreviewMode::Html {
            if let Some(html) = pending_html.take() {
                if let Some(ch) = chrome {
                    if let Some(bytes) = ch.screenshot(&html) {
                        if let Ok(img) = image::load_from_memory(&bytes) {
                            cached_image_protocol = Some(picker.new_resize_protocol(img));
                        }
                    }
                }
            }
        }

        let (source_line, mode) = parse_title(parser);
        let (mode_label, mode_st) = mode_style(mode);

        // Extract vim message from last line of PTY screen
        let vim_message = extract_vim_message(parser);

        let max_scroll = cached_lines.len().saturating_sub(1) as u16;
        let clamped_scroll = preview_scroll.min(max_scroll);

        let layout_label = match split_layout {
            SplitLayout::Vertical => "|",
            SplitLayout::Horizontal => "-",
            SplitLayout::EditorOnly => "[]",
        };

        let preview_mode_label = match preview_mode {
            PreviewMode::Text => "TEXT",
            PreviewMode::Html => "HTML",
        };

        terminal.draw(|frame| {
            let area = frame.area();
            let status_split = Layout::vertical([
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

            let main_area = status_split[0];
            let status_area = status_split[1];

            match split_layout {
                SplitLayout::Vertical => {
                    let panes = Layout::horizontal([
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                    ])
                    .split(main_area);
                    render_editor(frame, parser, mode_label, mode_st, &vim_message, panes[0]);
                    render_preview(
                        frame, &cached_lines, clamped_scroll, preview_mode,
                        preview_mode_label, &mut cached_image_protocol, panes[1],
                    );
                }
                SplitLayout::Horizontal => {
                    let panes = Layout::vertical([
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                    ])
                    .split(main_area);
                    render_editor(frame, parser, mode_label, mode_st, &vim_message, panes[0]);
                    render_preview(
                        frame, &cached_lines, clamped_scroll, preview_mode,
                        preview_mode_label, &mut cached_image_protocol, panes[1],
                    );
                }
                SplitLayout::EditorOnly => {
                    render_editor(frame, parser, mode_label, mode_st, &vim_message, main_area);
                }
            }

            // Status bar
            let chrome_hint = if chrome_available { " ^P:html" } else { "" };
            let browser_hint = if html_config.is_some() { " ^O:browser" } else { "" };
            let scroll_hint = if split_layout != SplitLayout::EditorOnly {
                " ^T:sync ^J/^K:scroll"
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

        if event::poll(Duration::from_millis(30))? {
            match event::read()? {
                Event::Key(key) => {
                    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

                    // Ctrl+G: cycle layout
                    if key.code == KeyCode::Char('g') && ctrl {
                        split_layout = match split_layout {
                            SplitLayout::Vertical => SplitLayout::Horizontal,
                            SplitLayout::Horizontal => SplitLayout::EditorOnly,
                            SplitLayout::EditorOnly => SplitLayout::Vertical,
                        };
                        resize_pty(split_layout, terminal, pty_master, parser)?;
                        // Re-render preview content when coming back from EditorOnly
                        if split_layout != SplitLayout::EditorOnly && cached_lines.is_empty() {
                            let (lines, _) = preview::render_with_offsets(&last_content);
                            cached_lines = lines;
                        }
                        continue;
                    }

                    // Ctrl+T: sync preview to vim cursor (proportional positioning)
                    if key.code == KeyCode::Char('t') && ctrl && split_layout != SplitLayout::EditorOnly {
                        // Count total source lines from content
                        let total_source_lines = last_content.lines().count().max(1);
                        let total_rendered_lines = cached_lines.len().max(1);

                        // source_line is the first visible line in vim (from titlestring w0)
                        // Calculate what proportion of the document we're viewing
                        let ratio = source_line as f64 / total_source_lines as f64;
                        let target = (ratio * total_rendered_lines as f64) as u16;

                        let term_size = terminal.size()?;
                        let visible_height = match split_layout {
                            SplitLayout::Vertical => term_size.height.saturating_sub(4),
                            SplitLayout::Horizontal => term_size.height / 2,
                            SplitLayout::EditorOnly => 0,
                        };

                        preview_scroll = target.saturating_sub(visible_height / 3);
                        continue;
                    }

                    // Ctrl+P: toggle preview mode (text/html)
                    if key.code == KeyCode::Char('p') && ctrl && chrome_available {
                        preview_mode = match preview_mode {
                            PreviewMode::Text => {
                                if let Some(cfg) = html_config {
                                    pending_html = Some(render_html_preview(&last_content, cfg));
                                }
                                PreviewMode::Html
                            }
                            PreviewMode::Html => {
                                cached_image_protocol = None;
                                PreviewMode::Text
                            }
                        };
                        continue;
                    }

                    // Ctrl+O: open in browser
                    if key.code == KeyCode::Char('o') && ctrl {
                        if let Some(cfg) = html_config {
                            let html = render_html_preview(&last_content, cfg);
                            let tmp_html = std::env::temp_dir().join("devtui-preview.html");
                            let _ = std::fs::write(&tmp_html, &html);
                            let _ = std::process::Command::new("open")
                                .arg(&tmp_html)
                                .spawn();
                        }
                        continue;
                    }

                    // Ctrl+J: scroll preview down
                    if key.code == KeyCode::Char('j') && ctrl && split_layout != SplitLayout::EditorOnly {
                        preview_scroll = preview_scroll.saturating_add(3).min(max_scroll);
                        continue;
                    }

                    // Ctrl+K: scroll preview up
                    if key.code == KeyCode::Char('k') && ctrl && split_layout != SplitLayout::EditorOnly {
                        preview_scroll = preview_scroll.saturating_sub(3);
                        continue;
                    }

                    // Pass everything else to vim
                    if let Some(bytes) = key_to_bytes(key.code, key.modifiers) {
                        if pty_writer.write_all(&bytes).is_err() {
                            break;
                        }
                        let _ = pty_writer.flush();
                    }
                }
                Event::Resize(width, height) => {
                    let (cols, rows) = vim_size(split_layout, width, height);
                    let _ = pty_master.resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
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

fn render_editor(
    frame: &mut ratatui::Frame,
    parser: &Arc<RwLock<vt100::Parser>>,
    mode_label: &str,
    mode_st: Style,
    vim_message: &str,
    area: ratatui::layout::Rect,
) {
    let mut title_spans = vec![
        Span::styled(mode_label, mode_st),
        Span::raw(" EDITOR"),
    ];

    if !vim_message.is_empty() {
        title_spans.push(Span::raw(" "));
        title_spans.push(Span::styled(
            vim_message,
            Style::default().fg(Color::Yellow),
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

fn render_preview(
    frame: &mut ratatui::Frame,
    cached_lines: &[Line<'static>],
    scroll: u16,
    preview_mode: PreviewMode,
    mode_label: &str,
    cached_image_protocol: &mut Option<ratatui_image::protocol::StatefulProtocol>,
    area: ratatui::layout::Rect,
) {
    let preview_title = format!(" PREVIEW [{mode_label}] ");
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
            if let Some(ref mut protocol) = cached_image_protocol {
                let image_widget = ratatui_image::StatefulImage::default();
                let inner = preview_block.inner(area);
                frame.render_widget(preview_block, area);
                frame.render_stateful_widget(image_widget, inner, protocol);
            } else {
                let loading = Paragraph::new(Line::from(Span::styled(
                    " Rendering HTML preview...",
                    Style::default().fg(Color::DarkGray),
                )))
                .block(preview_block);
                frame.render_widget(loading, area);
            }
        }
    }
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
        .unwrap_or_else(|| "Preview".to_string());

    let html = build::render_preview_html(content, &title, &config.blog_config, &config.article_tpl);
    let minified_css = minify::minify_css(&config.css);
    minify::inline_css(&html, &minified_css)
}

pub fn key_to_bytes(code: KeyCode, modifiers: KeyModifiers) -> Option<Vec<u8>> {
    if modifiers.contains(KeyModifiers::CONTROL) {
        return match code {
            KeyCode::Char(c) => {
                let byte = (c as u8).wrapping_sub(b'a').wrapping_add(1);
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
