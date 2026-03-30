use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
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

pub enum EditorResult {
    Quit,
}

pub struct HtmlPreviewConfig {
    pub css: String,
    pub article_tpl: String,
    pub blog_config: crate::engine::config::BlogConfig,
}

/// Run the vim+preview editor for a single file.
/// chrome is an optional pre-initialized Chrome instance (singleton from CMS).
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

    let preview_content = Arc::new(RwLock::new(initial_content));
    let preview_w = Arc::clone(&preview_content);
    let running_p = Arc::clone(&running);
    let content_handle = thread::spawn(move || {
        let mut last_content = String::new();
        while running_p.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(100));
            if let Ok(content) = std::fs::read_to_string(CONTENT_TMP) {
                if content != last_content {
                    last_content.clone_from(&content);
                    if let Ok(mut w) = preview_w.write() {
                        *w = content;
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
        &preview_content,
        html_config,
        chrome,
        &picker,
    )?;

    let final_content = preview_content
        .read()
        .map(|c| c.clone())
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

fn mode_style(mode: &str) -> (&str, Style) {
    match mode {
        "INSERT" => (" INSERT ", Style::default().fg(Color::Black).bg(Color::Green)),
        "VISUAL" => (" VISUAL ", Style::default().fg(Color::Black).bg(Color::Magenta)),
        "COMMAND" => (" COMMAND ", Style::default().fg(Color::Black).bg(Color::Yellow)),
        "REPLACE" => (" REPLACE ", Style::default().fg(Color::Black).bg(Color::Red)),
        _ => (" NORMAL ", Style::default().fg(Color::Black).bg(Color::Blue)),
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
    preview_content: &Arc<RwLock<String>>,
    html_config: Option<&HtmlPreviewConfig>,
    chrome: Option<&ChromePreview>,
    picker: &ratatui_image::picker::Picker,
) -> io::Result<()> {
    let mut last_content = String::new();
    let mut cached_lines: Vec<Line<'static>> = Vec::new();
    let mut cached_offsets: Vec<u16> = Vec::new();
    let mut preview_mode = PreviewMode::Text;
    let mut cached_image_protocol: Option<ratatui_image::protocol::StatefulProtocol> = None;

    // Preview scroll state
    let mut scroll_offset: i32 = 0; // manual scroll offset (added to auto scroll)
    let mut last_vim_scroll: u16 = 0; // track vim scroll to detect movement

    // Pending HTML for Chrome screenshot (rendered on main thread since Chrome isn't Send)
    let mut pending_html: Option<String> = None;

    let chrome_available = chrome.is_some();

    loop {
        if vim_exited.load(Ordering::Relaxed) {
            break;
        }

        // Re-render preview when content changed
        if let Ok(content) = preview_content.read() {
            if *content != last_content {
                last_content.clone_from(&content);

                let (lines, offsets) = preview::render_with_offsets(&last_content);
                cached_lines = lines;
                cached_offsets = offsets;

                // Queue HTML render if in HTML mode
                if preview_mode == PreviewMode::Html {
                    if let Some(cfg) = html_config {
                        pending_html = Some(render_html_preview(&last_content, cfg));
                    }
                }
            }
        }

        // Take Chrome screenshot on main thread (ChromePreview is not Send)
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

        // Auto scroll from vim
        let auto_scroll = if source_line < cached_offsets.len() {
            cached_offsets[source_line]
        } else {
            cached_offsets.last().copied().unwrap_or(0)
        };

        // Reset manual offset when vim scrolls (user moved in vim)
        if auto_scroll != last_vim_scroll {
            scroll_offset = 0;
            last_vim_scroll = auto_scroll;
        }

        // Final scroll = auto + manual offset, clamped to valid range
        let max_scroll = cached_lines.len().saturating_sub(1) as i32;
        let final_scroll = (auto_scroll as i32 + scroll_offset).clamp(0, max_scroll) as u16;

        let preview_mode_label = match preview_mode {
            PreviewMode::Text => "[TEXT]",
            PreviewMode::Html => "[HTML]",
        };

        let scroll_hint = if scroll_offset != 0 {
            format!(" scroll:{:+}", scroll_offset)
        } else {
            String::new()
        };

        terminal.draw(|frame| {
            let rows = Layout::vertical([
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(frame.area());

            let panes = Layout::horizontal([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(rows[0]);

            // Left: vim via PTY
            let editor_title = Line::from(vec![
                Span::styled(mode_label, mode_st),
                Span::raw(" EDITOR"),
            ]);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(editor_title);

            if let Ok(p) = parser.read() {
                let pseudo_term = PseudoTerminal::new(p.screen()).block(block);
                frame.render_widget(pseudo_term, panes[0]);
            }

            // Right: preview pane
            let preview_title = format!(" PREVIEW {}{} ", preview_mode_label, scroll_hint);
            let preview_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(preview_title);

            match preview_mode {
                PreviewMode::Text => {
                    let preview_widget = Paragraph::new(cached_lines.clone())
                        .block(preview_block)
                        .wrap(Wrap { trim: false })
                        .scroll((final_scroll, 0));
                    frame.render_widget(preview_widget, panes[1]);
                }
                PreviewMode::Html => {
                    if let Some(ref mut protocol) = cached_image_protocol {
                        let image_widget = ratatui_image::StatefulImage::default();
                        let inner = preview_block.inner(panes[1]);
                        frame.render_widget(preview_block, panes[1]);
                        frame.render_stateful_widget(image_widget, inner, protocol);
                    } else {
                        let loading = Paragraph::new(Line::from(Span::styled(
                            " Rendering HTML preview...",
                            Style::default().fg(Color::DarkGray),
                        )))
                        .block(preview_block);
                        frame.render_widget(loading, panes[1]);
                    }
                }
            }

            // Status bar
            let chrome_hint = if chrome_available { " ^P:html" } else { "" };
            let browser_hint = if html_config.is_some() { " ^B:browser" } else { "" };
            let status = Line::from(Span::styled(
                format!(" {} | DevTUI{}{} ^J/^K:scroll-preview", file_display, chrome_hint, browser_hint),
                Style::default().fg(Color::DarkGray),
            ));
            frame.render_widget(Paragraph::new(status), rows[1]);
        })?;

        if event::poll(Duration::from_millis(30))? {
            match event::read()? {
                Event::Key(key) => {
                    // Ctrl+P: toggle preview mode
                    if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) && chrome_available {
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

                    // Ctrl+B: open in browser
                    if key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL) {
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
                    if key.code == KeyCode::Char('j') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        scroll_offset += 3;
                        continue;
                    }

                    // Ctrl+K: scroll preview up
                    if key.code == KeyCode::Char('k') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        scroll_offset -= 3;
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
                    let new_cols = (width / 2).saturating_sub(2);
                    let new_rows = height.saturating_sub(3);
                    let _ = pty_master.resize(PtySize {
                        rows: new_rows,
                        cols: new_cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                    if let Ok(mut p) = parser.write() {
                        p.set_size(new_rows, new_cols);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn render_html_preview(content: &str, config: &HtmlPreviewConfig) -> String {
    render_html_from_parts(content, &config.css, &config.article_tpl, &config.blog_config)
}

fn render_html_from_parts(
    content: &str,
    css: &str,
    article_tpl: &str,
    blog_config: &crate::engine::config::BlogConfig,
) -> String {
    use crate::engine::{build, minify};

    let title = crate::engine::config::frontmatter("title", content)
        .unwrap_or_else(|| "Preview".to_string());

    let html = build::render_preview_html(content, &title, blog_config, article_tpl);
    let minified_css = minify::minify_css(css);
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
