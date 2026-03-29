mod preview;

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

const DRAFT_PATH: &str = "draft.md";
const CONTENT_TMP: &str = "/tmp/devtui-content";
const POS_TMP: &str = "/tmp/devtui-pos";

fn main() -> io::Result<()> {
    let file_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DRAFT_PATH));

    if !file_path.exists() {
        std::fs::write(&file_path, "")?;
    }

    // Canonicalize to absolute path so vim finds the file regardless of cwd
    let file_path = std::fs::canonicalize(&file_path)?;

    let initial_content = std::fs::read_to_string(&file_path).unwrap_or_default();

    let mut terminal = ratatui::init();
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
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    let file_str = file_path.to_str().unwrap_or(DRAFT_PATH);

    let content_autocmd = format!(
        "autocmd TextChanged,TextChangedI,BufWritePost * call writefile(getline(1,'$'),'{}')",
        CONTENT_TMP
    );
    let pos_autocmd = format!(
        "autocmd CursorHold,CursorHoldI,TextChanged,TextChangedI,BufWritePost * call writefile([line('w0')],'{}')",
        POS_TMP
    );
    let initial_write = format!("call writefile(getline(1,'$'),'{}')", CONTENT_TMP);

    let mut cmd = CommandBuilder::new("vim");
    cmd.args([
        "-u", "NONE",
        "-N",
        "-c", "set noswapfile noruler laststatus=0 updatetime=100 tabstop=2 shiftwidth=2 expandtab",
        "-c", &content_autocmd,
        "-c", &pos_autocmd,
        "-c", &initial_write,
        file_str,
    ]);

    let mut child = pty_pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    drop(pty_pair.slave);

    let mut pty_writer = pty_pair
        .master
        .take_writer()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    let pty_reader = pty_pair
        .master
        .try_clone_reader()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

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
    let viewport_line = Arc::new(std::sync::atomic::AtomicU16::new(0));
    let preview_w = Arc::clone(&preview_content);
    let viewport_w = Arc::clone(&viewport_line);
    let running_p = Arc::clone(&running);
    let preview_handle = thread::spawn(move || {
        let mut last_content = String::new();
        let mut last_pos = String::new();
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
            if let Ok(pos) = std::fs::read_to_string(POS_TMP) {
                if pos != last_pos {
                    last_pos.clone_from(&pos);
                    if let Ok(n) = pos.trim().parse::<u16>() {
                        viewport_w.store(n.saturating_sub(1), std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
        }
    });

    let file_display = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| file_path.display().to_string());

    let result = run_loop(
        &mut terminal,
        &parser,
        &mut pty_writer,
        &pty_pair.master,
        &preview_content,
        &viewport_line,
        &vim_exited,
        &file_display,
    );

    running.store(false, Ordering::Relaxed);
    ratatui::restore();
    drop(pty_pair.master);
    let _ = child.kill();
    let _ = reader_handle.join();
    let _ = preview_handle.join();
    let _ = std::fs::remove_file(CONTENT_TMP);
    let _ = std::fs::remove_file(POS_TMP);

    result
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

fn detect_mode(parser: &Arc<RwLock<vt100::Parser>>) -> &'static str {
    if let Ok(p) = parser.read() {
        let screen = p.screen();
        let rows = screen.size().0;
        let cols = screen.size().1;
        let mut last_row = String::new();
        for col in 0..cols {
            if let Some(cell) = screen.cell(rows.saturating_sub(1), col) {
                last_row.push_str(&cell.contents());
            }
        }
        let trimmed = last_row.trim();

        if trimmed.starts_with(':') || trimmed.starts_with('/') || trimmed.starts_with('?') {
            return "COMMAND";
        }
        if trimmed.contains("-- INSERT --") {
            return "INSERT";
        }
        if trimmed.contains("-- VISUAL") {
            return "VISUAL";
        }
        if trimmed.contains("-- REPLACE --") {
            return "REPLACE";
        }
    }
    "NORMAL"
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

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    parser: &Arc<RwLock<vt100::Parser>>,
    pty_writer: &mut Box<dyn Write + Send>,
    pty_master: &Box<dyn portable_pty::MasterPty + Send>,
    preview_content: &Arc<RwLock<String>>,
    viewport_line: &Arc<std::sync::atomic::AtomicU16>,
    vim_exited: &Arc<AtomicBool>,
    file_display: &str,
) -> io::Result<()> {
    loop {
        if vim_exited.load(Ordering::Relaxed) {
            break;
        }

        let mode = detect_mode(parser);
        let (mode_label, mode_st) = mode_style(mode);

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

            // Right: markdown preview
            let preview_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" PREVIEW ");

            if let Ok(content) = preview_content.read() {
                let source_line = viewport_line.load(std::sync::atomic::Ordering::Relaxed) as usize;
                // Render only from the visible source line forward
                let visible: String = content
                    .lines()
                    .skip(source_line)
                    .collect::<Vec<_>>()
                    .join("\n");
                let (lines, _) = preview::render_with_offsets(&visible);
                let preview = Paragraph::new(lines)
                    .block(preview_block)
                    .wrap(Wrap { trim: false });
                frame.render_widget(preview, panes[1]);
            }

            // Status bar
            let status = Line::from(Span::styled(
                format!(" {} | DevTUI", file_display),
                Style::default().fg(Color::DarkGray),
            ));
            frame.render_widget(Paragraph::new(status), rows[1]);
        })?;

        // Poll for keyboard/resize events
        if event::poll(Duration::from_millis(30))? {
            match event::read()? {
                Event::Key(key) => {
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

fn key_to_bytes(code: KeyCode, modifiers: KeyModifiers) -> Option<Vec<u8>> {
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
