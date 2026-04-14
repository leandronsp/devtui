//! Agent pane: embedded Claude Code CLI via PTY.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

/// PTY state for the embedded Claude Code agent.
pub struct AgentState {
    pub parser: Arc<RwLock<vt100::Parser>>,
    pub writer: Box<dyn Write + Send>,
    pub master: Box<dyn portable_pty::MasterPty + Send>,
    running: Arc<AtomicBool>,
}

impl AgentState {
    /// Spawn `claude` in a PTY with the given dimensions.
    pub fn spawn(rows: u16, cols: u16) -> io::Result<Self> {
        let pty_system = NativePtySystem::default();
        let pty_pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| io::Error::other(e.to_string()))?;

        let mut cmd = CommandBuilder::new("claude");
        cmd.args([
            "--allow-dangerously-skip-permissions",
            "--append-system-prompt",
            "The user is writing a blog article in DevTUI. \
             The live article content is at /tmp/devtui-content (updated only when the user saves with :w). \
             Start by running /cms watch devtui to activate the writing companion. \
             The cms skill is at ~/.claude/skills/cms/SKILL.md. \
             The user's vault is at ~/vault (searchable with qmd).",
        ]);

        let _child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| io::Error::other(e.to_string()))?;

        drop(pty_pair.slave);

        let writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| io::Error::other(e.to_string()))?;

        let reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| io::Error::other(e.to_string()))?;

        let parser = Arc::new(RwLock::new(vt100::Parser::new(rows, cols, 10_000)));
        let running = Arc::new(AtomicBool::new(true));

        let parser_w = Arc::clone(&parser);
        let running_r = Arc::clone(&running);
        thread::spawn(move || {
            read_pty(reader, parser_w, running_r);
        });

        Ok(Self {
            parser,
            writer,
            master: pty_pair.master,
            running,
        })
    }

    /// Resize the agent PTY.
    pub fn resize(&self, rows: u16, cols: u16) {
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        if let Ok(mut p) = self.parser.write() {
            p.set_size(rows, cols);
        }
    }

    /// Send raw bytes to the agent PTY (keystrokes).
    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }

    /// Scroll the agent pane. Positive = into history, negative = toward present.
    pub fn scroll(&self, delta: i32) {
        if let Ok(mut p) = self.parser.write() {
            let current = p.screen().scrollback() as i32;
            let new = current.saturating_add(delta).max(0) as usize;
            p.set_scrollback(new);
        }
    }

    /// Reset scrollback to the present (bottom).
    pub fn scroll_to_bottom(&self) {
        if let Ok(mut p) = self.parser.write() {
            p.set_scrollback(0);
        }
    }
}

impl Drop for AgentState {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
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
