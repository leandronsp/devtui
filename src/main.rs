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
const PREVIEW_TMP: &str = "/tmp/devtui-preview.md";

fn main() -> io::Result<()> {
    let file_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DRAFT_PATH));

    if !file_path.exists() {
        std::fs::write(&file_path, "")?;
    }

    // Copy file to preview tmp so preview shows initial content
    let _ = std::fs::copy(&file_path, PREVIEW_TMP);

    let mut terminal = ratatui::init();
    let term_size = terminal.size()?;

    let vim_cols = (term_size.width / 2).saturating_sub(2); // half width minus borders
    let vim_rows = term_size.height.saturating_sub(3);     // total - top border - bottom border - status bar

    // Spawn vim in a PTY
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

    let vim_setup = format!(
        "set noswapfile noruler laststatus=0 | \
         autocmd CursorMoved,CursorMovedI,TextChanged,TextChangedI,BufReadPost * silent! write! {}",
        PREVIEW_TMP
    );

    let mut cmd = CommandBuilder::new("vim");
    cmd.args([
        "-u", "NONE",
        "-N",
        "-c", &vim_setup,
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
    let pty_has_data = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));

    let vim_exited = Arc::new(AtomicBool::new(false));

    // Reader thread: PTY output -> vt100 parser
    let parser_w = Arc::clone(&parser);
    let has_data_w = Arc::clone(&pty_has_data);
    let running_r = Arc::clone(&running);
    let exited_w = Arc::clone(&vim_exited);
    let reader_handle = thread::spawn(move || {
        read_pty(pty_reader, parser_w, has_data_w, running_r);
        exited_w.store(true, Ordering::Relaxed);
    });

    // Preview: start with the actual file content
    let preview_content = Arc::new(RwLock::new(
        std::fs::read_to_string(&file_path).unwrap_or_default(),
    ));
    let preview_changed = Arc::new(AtomicBool::new(false));
    let preview_w = Arc::clone(&preview_content);
    let preview_changed_w = Arc::clone(&preview_changed);
    let running_p = Arc::clone(&running);
    let preview_handle = thread::spawn(move || {
        let mut last = String::new();
        while running_p.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(100));
            if let Ok(content) = std::fs::read_to_string(PREVIEW_TMP) {
                if content != last {
                    last.clone_from(&content);
                    if let Ok(mut w) = preview_w.write() {
                        *w = content;
                    }
                    preview_changed_w.store(true, Ordering::Relaxed);
                }
            }
        }
    });

    let file_display = file_path.display().to_string();

    // Main loop
    let result = run_loop(&mut terminal, &parser, &mut pty_writer, &pty_pair.master, &preview_content, &pty_has_data, &vim_exited, &preview_changed, &file_display);

    // Cleanup
    running.store(false, Ordering::Relaxed);
    ratatui::restore();
    drop(pty_pair.master);
    let _ = child.kill();
    let _ = reader_handle.join();
    let _ = preview_handle.join();
    let _ = std::fs::remove_file(PREVIEW_TMP);

    result
}

fn read_pty(
    mut reader: Box<dyn Read + Send>,
    parser: Arc<RwLock<vt100::Parser>>,
    has_data: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
) {
    let mut buf = [0u8; 4096];
    while running.load(Ordering::Relaxed) {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if let Ok(mut p) = parser.write() {
                    p.process(&buf[..n]);
                    has_data.store(true, Ordering::Relaxed);
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
        // Check last row of vim for mode indicators
        let last_row = extract_row(screen, rows.saturating_sub(1));
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

fn extract_row(screen: &vt100::Screen, row: u16) -> String {
    let cols = screen.size().1;
    let mut s = String::new();
    for col in 0..cols {
        let cell = screen.cell(row, col);
        if let Some(cell) = cell {
            s.push_str(&cell.contents());
        }
    }
    s
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
    pty_has_data: &Arc<AtomicBool>,
    vim_exited: &Arc<AtomicBool>,
    preview_changed: &Arc<AtomicBool>,
    file_display: &str,
) -> io::Result<()> {
    let mut needs_draw = true;

    loop {
        if vim_exited.load(Ordering::Relaxed) {
            break;
        }

        if needs_draw {
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
                    let lines = preview::render(&content);
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
            needs_draw = false;
        }

        // Check for PTY or preview updates
        if pty_has_data.swap(false, Ordering::Relaxed) {
            needs_draw = true;
        }
        if preview_changed.swap(false, Ordering::Relaxed) {
            needs_draw = true;
        }

        // Poll for keyboard/resize events
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    if let Some(bytes) = key_to_bytes(key.code, key.modifiers) {
                        if pty_writer.write_all(&bytes).is_err() {
                            break;
                        }
                        let _ = pty_writer.flush();
                    }
                    needs_draw = true;
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
                    needs_draw = true;
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
