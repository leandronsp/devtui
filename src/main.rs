mod buffer;
mod preview;
mod ui;
mod vim;

use std::io;
use std::path::PathBuf;

use crossterm::event::{self, Event};

use buffer::Buffer;
use vim::{Action, Vim};

const DRAFT_PATH: &str = "draft.md";

fn run(terminal: &mut ratatui::DefaultTerminal, file_path: PathBuf) -> io::Result<()> {
    let mut buf = Buffer::open(file_path);
    let mut vim = Vim::new();

    loop {
        let clear_status = !vim.status_msg.is_empty() && vim.mode != vim::Mode::Command;

        terminal.draw(|frame| ui::draw(frame, &mut buf, &vim))?;

        if clear_status {
            vim.status_msg.clear();
        }

        if let Event::Key(key) = event::read()? {
            if let Action::Quit = vim.handle_key(key.code, key.modifiers, &mut buf) {
                break;
            }
        }
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let file_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DRAFT_PATH));

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, file_path);
    ratatui::restore();
    result
}
