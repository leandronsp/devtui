use crossterm::event::{KeyCode, KeyModifiers};

use crate::buffer::Buffer;

#[derive(PartialEq, Clone)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    VisualBlock,
    Command,
}

pub enum Action {
    Continue,
    Quit,
}

#[derive(Clone)]
enum Pending {
    Operator(char),  // d, c, y waiting for motion
    G,               // gg
    Replace,         // r waiting for char
    FindForward,     // f waiting for char
    FindBackward,    // F waiting for char
    TilForward,      // t waiting for char
    TilBackward,     // T waiting for char
}

pub struct Vim {
    pub mode: Mode,
    pub register: Vec<char>,
    pub visual_anchor: usize,
    pub command_buf: String,
    pub status_msg: String,
    pending: Option<Pending>,
    count_buf: String,
}

impl Vim {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            register: Vec::new(),
            visual_anchor: 0,
            command_buf: String::new(),
            status_msg: String::new(),
            pending: None,
            count_buf: String::new(),
        }
    }

    fn take_count(&mut self) -> usize {
        let n = self.count_buf.parse::<usize>().unwrap_or(1);
        self.count_buf.clear();
        n
    }

    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers, buf: &mut Buffer) -> Action {
        if code == KeyCode::Char('c') && modifiers == KeyModifiers::CONTROL {
            return Action::Quit;
        }

        // Ctrl+[ as Esc
        if code == KeyCode::Char('[') && modifiers == KeyModifiers::CONTROL {
            self.escape(buf);
            return Action::Continue;
        }

        match self.mode {
            Mode::Normal => self.handle_normal(code, modifiers, buf),
            Mode::Insert => self.handle_insert(code, buf),
            Mode::Visual => self.handle_visual(code, modifiers, buf),
            Mode::VisualLine => self.handle_visual_line(code, modifiers, buf),
            Mode::VisualBlock => self.handle_visual_block(code, modifiers, buf),
            Mode::Command => return self.handle_command(code, buf),
        }

        Action::Continue
    }

    fn escape(&mut self, buf: &mut Buffer) {
        match self.mode {
            Mode::Insert => self.to_normal(buf),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock | Mode::Command => {
                self.mode = Mode::Normal;
                self.command_buf.clear();
                self.pending = None;
                self.count_buf.clear();
            }
            Mode::Normal => {
                self.pending = None;
                self.count_buf.clear();
            }
        }
    }

    fn to_normal(&mut self, buf: &mut Buffer) {
        self.mode = Mode::Normal;
        self.pending = None;
        self.count_buf.clear();
        if buf.cursor > 0 && buf.char_at(buf.cursor - 1) != Some('\n') {
            buf.cursor -= 1;
        }
    }

    // ── Normal mode ──

    fn handle_normal(&mut self, code: KeyCode, modifiers: KeyModifiers, buf: &mut Buffer) {
        // Ctrl combos
        if modifiers == KeyModifiers::CONTROL {
            match code {
                KeyCode::Char('d') => { self.pending = None; for _ in 0..10 { buf.move_vertical(1); } }
                KeyCode::Char('u') => { self.pending = None; for _ in 0..10 { buf.move_vertical(-1); } }
                KeyCode::Char('r') => {
                    if buf.redo() {
                        self.status_msg = "Redo".to_string();
                    }
                }
                KeyCode::Char('v') => {
                    self.visual_anchor = buf.cursor;
                    self.mode = Mode::VisualBlock;
                }
                _ => {}
            }
            return;
        }

        // Handle pending states
        if let Some(pending) = self.pending.clone() {
            self.pending = None;
            match pending {
                Pending::Operator(op) => {
                    self.handle_operator_motion(op, code, buf);
                    return;
                }
                Pending::G => {
                    if code == KeyCode::Char('g') { buf.move_to_top(); }
                    return;
                }
                Pending::Replace => {
                    if let KeyCode::Char(c) = code {
                        buf.snapshot();
                        buf.replace_char(c);
                    }
                    return;
                }
                Pending::FindForward => {
                    if let KeyCode::Char(c) = code { self.find_char_forward(c, buf, false); }
                    return;
                }
                Pending::FindBackward => {
                    if let KeyCode::Char(c) = code { self.find_char_backward(c, buf, false); }
                    return;
                }
                Pending::TilForward => {
                    if let KeyCode::Char(c) = code { self.find_char_forward(c, buf, true); }
                    return;
                }
                Pending::TilBackward => {
                    if let KeyCode::Char(c) = code { self.find_char_backward(c, buf, true); }
                    return;
                }
            }
        }

        // Count prefix (digits 1-9, or 0 if already accumulating)
        if let KeyCode::Char(c @ '1'..='9') = code {
            self.count_buf.push(c);
            return;
        }
        if let KeyCode::Char('0') = code {
            if !self.count_buf.is_empty() {
                self.count_buf.push('0');
                return;
            }
        }

        let count = self.take_count();

        match code {
            // Mode transitions
            KeyCode::Char('i') => self.mode = Mode::Insert,
            KeyCode::Char('a') => {
                if let Some(ch) = buf.char_at(buf.cursor) {
                    if ch != '\n' { buf.cursor += 1; }
                }
                self.mode = Mode::Insert;
            }
            KeyCode::Char('A') => {
                let row = buf.cursor_row();
                buf.cursor = buf.line_start(row) + buf.line_len(row);
                self.mode = Mode::Insert;
            }
            KeyCode::Char('I') => {
                buf.move_to_line_start();
                self.mode = Mode::Insert;
            }
            KeyCode::Char('o') => {
                buf.snapshot();
                buf.open_line_below();
                self.mode = Mode::Insert;
            }
            KeyCode::Char('O') => {
                buf.snapshot();
                buf.open_line_above();
                self.mode = Mode::Insert;
            }
            KeyCode::Char('s') => {
                buf.snapshot();
                if let Some(ch) = buf.delete_char_at_cursor() {
                    self.register = vec![ch];
                }
                self.mode = Mode::Insert;
            }
            KeyCode::Char('S') => {
                buf.snapshot();
                self.register = buf.change_line();
                self.mode = Mode::Insert;
            }

            // Visual modes
            KeyCode::Char('v') => {
                self.visual_anchor = buf.cursor;
                self.mode = Mode::Visual;
            }
            KeyCode::Char('V') => {
                self.visual_anchor = buf.cursor;
                self.mode = Mode::VisualLine;
            }

            // Command mode
            KeyCode::Char(':') => {
                self.command_buf.clear();
                self.mode = Mode::Command;
            }

            // Motions
            KeyCode::Char('h') | KeyCode::Left => { for _ in 0..count { buf.move_left(); } }
            KeyCode::Char('l') | KeyCode::Right => { for _ in 0..count { buf.move_right(); } }
            KeyCode::Char('j') | KeyCode::Down => { for _ in 0..count { buf.move_vertical(1); } }
            KeyCode::Char('k') | KeyCode::Up => { for _ in 0..count { buf.move_vertical(-1); } }
            KeyCode::Char('w') => { for _ in 0..count { buf.word_forward(); } }
            KeyCode::Char('b') => { for _ in 0..count { buf.word_backward(); } }
            KeyCode::Char('e') => { for _ in 0..count { buf.word_end(); } }
            KeyCode::Char('0') => buf.move_to_line_start(),
            KeyCode::Char('^') => buf.move_to_line_start(), // TODO: first non-blank
            KeyCode::Char('$') => buf.move_to_line_end(),
            KeyCode::Char('G') => buf.move_to_bottom(),
            KeyCode::Char('g') => { self.pending = Some(Pending::G); }

            // Find on line
            KeyCode::Char('f') => { self.pending = Some(Pending::FindForward); }
            KeyCode::Char('F') => { self.pending = Some(Pending::FindBackward); }
            KeyCode::Char('t') => { self.pending = Some(Pending::TilForward); }
            KeyCode::Char('T') => { self.pending = Some(Pending::TilBackward); }

            // Operators (waiting for motion)
            KeyCode::Char('d') => { self.pending = Some(Pending::Operator('d')); }
            KeyCode::Char('c') => { self.pending = Some(Pending::Operator('c')); }
            KeyCode::Char('y') => { self.pending = Some(Pending::Operator('y')); }

            // Shortcuts (operator+motion in one key)
            KeyCode::Char('D') => {
                buf.snapshot();
                self.register = buf.delete_to_end_of_line();
            }
            KeyCode::Char('C') => {
                buf.snapshot();
                self.register = buf.change_to_end_of_line();
                self.mode = Mode::Insert;
            }

            // Single-char operations
            KeyCode::Char('x') => {
                buf.snapshot();
                for _ in 0..count {
                    if let Some(ch) = buf.delete_char_at_cursor() {
                        self.register = vec![ch];
                    }
                }
            }
            KeyCode::Char('r') => { self.pending = Some(Pending::Replace); }
            KeyCode::Char('~') => {
                buf.snapshot();
                for _ in 0..count { buf.toggle_case(); }
            }
            KeyCode::Char('J') => {
                buf.snapshot();
                for _ in 0..count { buf.join_line(); }
            }

            // Paste
            KeyCode::Char('p') => { buf.snapshot(); buf.paste_after(&self.register); }
            KeyCode::Char('P') => { buf.snapshot(); buf.paste_before(&self.register); }

            // Undo
            KeyCode::Char('u') => {
                if buf.undo() {
                    self.status_msg = "Undo".to_string();
                } else {
                    self.status_msg = "Already at oldest change".to_string();
                }
            }

            _ => {}
        }
    }

    fn handle_operator_motion(&mut self, op: char, code: KeyCode, buf: &mut Buffer) {
        buf.snapshot();
        match (op, code) {
            // dd, cc, yy
            ('d', KeyCode::Char('d')) => { self.register = buf.delete_line(); }
            ('c', KeyCode::Char('c')) => {
                self.register = buf.change_line();
                self.mode = Mode::Insert;
            }
            ('y', KeyCode::Char('y')) => {
                self.register = buf.yank_line();
                self.status_msg = "1 line yanked".to_string();
            }

            // d/c/y + w (word)
            (op, KeyCode::Char('w')) => {
                if op == 'c' {
                    self.register = buf.change_word_forward();
                    self.mode = Mode::Insert;
                } else {
                    let removed = buf.delete_word_forward();
                    if op == 'y' {
                        buf.undo(); // yank doesn't delete
                        self.register = removed;
                        self.status_msg = "yanked".to_string();
                    } else {
                        self.register = removed;
                    }
                }
            }

            // d/c/y + e (end of word)
            (op, KeyCode::Char('e')) => {
                let start = buf.cursor;
                buf.word_end();
                let end = buf.cursor;
                buf.cursor = start;
                if end >= start {
                    if op == 'y' {
                        self.register = buf.chars[start..=end].to_vec();
                        self.status_msg = "yanked".to_string();
                    } else {
                        self.register = buf.delete_range(start, end);
                        if op == 'c' { self.mode = Mode::Insert; }
                    }
                }
            }

            // d/c/y + $ (end of line)
            (op, KeyCode::Char('$')) => {
                if op == 'c' {
                    self.register = buf.change_to_end_of_line();
                    self.mode = Mode::Insert;
                } else if op == 'y' {
                    let end = buf.line_end_pos();
                    if buf.cursor < end {
                        self.register = buf.chars[buf.cursor..end].to_vec();
                    }
                    self.status_msg = "yanked".to_string();
                } else {
                    self.register = buf.delete_to_end_of_line();
                }
            }

            // d/c/y + 0 (start of line)
            (op, KeyCode::Char('0')) => {
                if op == 'y' {
                    let row = buf.cursor_row();
                    let start = buf.line_start(row);
                    if start < buf.cursor {
                        self.register = buf.chars[start..buf.cursor].to_vec();
                    }
                    self.status_msg = "yanked".to_string();
                } else {
                    self.register = buf.delete_to_start_of_line();
                    if op == 'c' { self.mode = Mode::Insert; }
                }
            }

            // d/c/y + b (word backward)
            (op, KeyCode::Char('b')) => {
                let end = buf.cursor;
                buf.word_backward();
                let start = buf.cursor;
                if start < end {
                    if op == 'y' {
                        self.register = buf.chars[start..end].to_vec();
                        buf.cursor = end;
                        self.status_msg = "yanked".to_string();
                    } else {
                        self.register = buf.delete_range(start, end - 1);
                        if op == 'c' { self.mode = Mode::Insert; }
                    }
                }
            }

            // d/c/y + G (to bottom)
            (op, KeyCode::Char('G')) => {
                let start = buf.line_start(buf.cursor_row());
                let end = buf.chars.len().saturating_sub(1);
                if op == 'y' {
                    self.register = buf.chars[start..=end].to_vec();
                    self.status_msg = "yanked".to_string();
                } else {
                    self.register = buf.delete_range(start, end);
                    if op == 'c' { self.mode = Mode::Insert; }
                }
            }

            // d/c/y + g (gg = to top)
            (op, KeyCode::Char('g')) => {
                let end_row = buf.cursor_row();
                let end = buf.line_start(end_row) + buf.line_len(end_row);
                let end = if end < buf.chars.len() { end } else { end.saturating_sub(1) };
                if op == 'y' {
                    self.register = buf.chars[0..=end].to_vec();
                    self.status_msg = "yanked".to_string();
                } else {
                    self.register = buf.delete_range(0, end);
                    if op == 'c' { self.mode = Mode::Insert; }
                }
            }

            // Unknown motion, undo the snapshot
            _ => { buf.undo(); }
        }
    }

    fn find_char_forward(&self, target: char, buf: &mut Buffer, til: bool) {
        let row = buf.cursor_row();
        let line_end = buf.line_start(row) + buf.line_len(row);
        for pos in (buf.cursor + 1)..line_end {
            if buf.chars[pos] == target {
                buf.cursor = if til { pos - 1 } else { pos };
                return;
            }
        }
    }

    fn find_char_backward(&self, target: char, buf: &mut Buffer, til: bool) {
        let row = buf.cursor_row();
        let line_start = buf.line_start(row);
        if buf.cursor <= line_start { return; }
        for pos in (line_start..buf.cursor).rev() {
            if buf.chars[pos] == target {
                buf.cursor = if til { pos + 1 } else { pos };
                return;
            }
        }
    }

    // ── Insert mode ──

    fn handle_insert(&mut self, code: KeyCode, buf: &mut Buffer) {
        match code {
            KeyCode::Esc => self.to_normal(buf),
            KeyCode::Char(c) => buf.insert_char(c),
            KeyCode::Enter => buf.insert_char('\n'),
            KeyCode::Backspace => buf.delete_char_backward(),
            KeyCode::Left => { if buf.cursor > 0 { buf.cursor -= 1; } }
            KeyCode::Right => { if buf.cursor < buf.chars.len() { buf.cursor += 1; } }
            KeyCode::Up => buf.move_vertical(-1),
            KeyCode::Down => buf.move_vertical(1),
            _ => {}
        }
    }

    // ── Visual mode ──

    fn handle_visual(&mut self, code: KeyCode, modifiers: KeyModifiers, buf: &mut Buffer) {
        if modifiers == KeyModifiers::CONTROL {
            if code == KeyCode::Char('d') { for _ in 0..10 { buf.move_vertical(1); } }
            if code == KeyCode::Char('u') { for _ in 0..10 { buf.move_vertical(-1); } }
            return;
        }

        match code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Char('h') | KeyCode::Left => buf.move_left(),
            KeyCode::Char('l') | KeyCode::Right => buf.move_right(),
            KeyCode::Char('j') | KeyCode::Down => buf.move_vertical(1),
            KeyCode::Char('k') | KeyCode::Up => buf.move_vertical(-1),
            KeyCode::Char('w') => buf.word_forward(),
            KeyCode::Char('b') => buf.word_backward(),
            KeyCode::Char('e') => buf.word_end(),
            KeyCode::Char('0') => buf.move_to_line_start(),
            KeyCode::Char('$') => buf.move_to_line_end(),
            KeyCode::Char('G') => buf.move_to_bottom(),
            KeyCode::Char('g') => buf.move_to_top(),
            KeyCode::Char('d') | KeyCode::Char('x') => {
                buf.snapshot();
                let (start, end) = self.visual_range(buf);
                self.register = buf.delete_range(start, end);
                self.mode = Mode::Normal;
            }
            KeyCode::Char('c') => {
                buf.snapshot();
                let (start, end) = self.visual_range(buf);
                self.register = buf.delete_range(start, end);
                self.mode = Mode::Insert;
            }
            KeyCode::Char('y') => {
                let (start, end) = self.visual_range(buf);
                self.register = buf.chars[start..=end.min(buf.chars.len() - 1)].to_vec();
                buf.cursor = start;
                self.mode = Mode::Normal;
                self.status_msg = "yanked".to_string();
            }
            KeyCode::Char('~') => {
                buf.snapshot();
                let (start, end) = self.visual_range(buf);
                for i in start..=end.min(buf.chars.len() - 1) {
                    let ch = buf.chars[i];
                    if ch.is_uppercase() {
                        buf.chars[i] = ch.to_lowercase().next().unwrap_or(ch);
                    } else if ch.is_lowercase() {
                        buf.chars[i] = ch.to_uppercase().next().unwrap_or(ch);
                    }
                }
                buf.dirty = true;
                buf.cursor = start;
                self.mode = Mode::Normal;
            }
            KeyCode::Char('J') => {
                buf.snapshot();
                let (start, _end) = self.visual_range(buf);
                buf.cursor = start;
                buf.join_line();
                self.mode = Mode::Normal;
            }
            _ => {}
        }
    }

    fn handle_visual_line(&mut self, code: KeyCode, modifiers: KeyModifiers, buf: &mut Buffer) {
        if modifiers == KeyModifiers::CONTROL {
            if code == KeyCode::Char('d') { for _ in 0..10 { buf.move_vertical(1); } }
            if code == KeyCode::Char('u') { for _ in 0..10 { buf.move_vertical(-1); } }
            return;
        }

        match code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Char('j') | KeyCode::Down => buf.move_vertical(1),
            KeyCode::Char('k') | KeyCode::Up => buf.move_vertical(-1),
            KeyCode::Char('G') => buf.move_to_bottom(),
            KeyCode::Char('g') => buf.move_to_top(),
            KeyCode::Char('d') | KeyCode::Char('x') => {
                buf.snapshot();
                let (start, end) = self.visual_line_range(buf);
                self.register = buf.delete_range(start, end);
                self.mode = Mode::Normal;
            }
            KeyCode::Char('c') => {
                buf.snapshot();
                let (start, end) = self.visual_line_range(buf);
                self.register = buf.delete_range(start, end);
                self.mode = Mode::Insert;
            }
            KeyCode::Char('y') => {
                let (start, end) = self.visual_line_range(buf);
                self.register = buf.chars[start..=end.min(buf.chars.len() - 1)].to_vec();
                buf.cursor = start;
                self.mode = Mode::Normal;
                self.status_msg = "yanked".to_string();
            }
            _ => {}
        }
    }

    fn handle_visual_block(&mut self, code: KeyCode, modifiers: KeyModifiers, buf: &mut Buffer) {
        if modifiers == KeyModifiers::CONTROL {
            if code == KeyCode::Char('d') { for _ in 0..10 { buf.move_vertical(1); } }
            if code == KeyCode::Char('u') { for _ in 0..10 { buf.move_vertical(-1); } }
            return;
        }

        match code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Char('h') | KeyCode::Left => buf.move_left(),
            KeyCode::Char('l') | KeyCode::Right => buf.move_right(),
            KeyCode::Char('j') | KeyCode::Down => buf.move_vertical(1),
            KeyCode::Char('k') | KeyCode::Up => buf.move_vertical(-1),
            KeyCode::Char('d') | KeyCode::Char('x') => {
                buf.snapshot();
                let ranges = self.visual_block_ranges(buf);
                // delete from bottom to top to preserve positions
                let mut removed = Vec::new();
                for (start, end) in ranges.into_iter().rev() {
                    let mut chunk: Vec<char> = buf.chars.drain(start..=end).collect();
                    chunk.push('\n');
                    removed.splice(0..0, chunk);
                }
                self.register = removed;
                buf.cursor = buf.cursor.min(buf.chars.len().saturating_sub(1));
                buf.dirty = true;
                self.mode = Mode::Normal;
            }
            KeyCode::Char('y') => {
                let ranges = self.visual_block_ranges(buf);
                let mut yanked = Vec::new();
                for (i, (start, end)) in ranges.iter().enumerate() {
                    yanked.extend_from_slice(&buf.chars[*start..=*end]);
                    if i < ranges.len() - 1 { yanked.push('\n'); }
                }
                self.register = yanked;
                self.mode = Mode::Normal;
                self.status_msg = "block yanked".to_string();
            }
            _ => {}
        }
    }

    // ── Command mode ──

    fn handle_command(&mut self, code: KeyCode, buf: &mut Buffer) -> Action {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buf.clear();
            }
            KeyCode::Enter => {
                let cmd = self.command_buf.clone();
                self.mode = Mode::Normal;
                self.command_buf.clear();
                return self.execute_command(&cmd, buf);
            }
            KeyCode::Backspace => {
                if self.command_buf.pop().is_none() {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Char(c) => self.command_buf.push(c),
            _ => {}
        }
        Action::Continue
    }

    fn execute_command(&mut self, cmd: &str, buf: &mut Buffer) -> Action {
        match cmd.trim() {
            "w" => {
                match buf.save() {
                    Ok(()) => self.status_msg = format!("\"{}\" written", buf.file_path.display()),
                    Err(e) => self.status_msg = format!("Error: {}", e),
                }
                Action::Continue
            }
            "q" => {
                if buf.dirty {
                    self.status_msg = "No write since last change (add ! to override)".to_string();
                    Action::Continue
                } else {
                    Action::Quit
                }
            }
            "q!" => Action::Quit,
            "wq" | "x" => {
                match buf.save() {
                    Ok(()) => Action::Quit,
                    Err(e) => {
                        self.status_msg = format!("Error: {}", e);
                        Action::Continue
                    }
                }
            }
            _ => {
                self.status_msg = format!("Not a command: {}", cmd);
                Action::Continue
            }
        }
    }

    // ── Range helpers ──

    pub fn visual_range(&self, buf: &Buffer) -> (usize, usize) {
        let start = self.visual_anchor.min(buf.cursor);
        let end = self.visual_anchor.max(buf.cursor);
        (start, end)
    }

    pub fn visual_line_range(&self, buf: &Buffer) -> (usize, usize) {
        let anchor_row = buf.chars[..self.visual_anchor]
            .iter()
            .filter(|&&c| c == '\n')
            .count();
        let cursor_row = buf.cursor_row();
        let first_row = anchor_row.min(cursor_row);
        let last_row = anchor_row.max(cursor_row);

        let start = buf.line_start(first_row);
        let mut end = buf.line_start(last_row) + buf.line_len(last_row);
        if end < buf.chars.len() && buf.chars[end] == '\n' {
            end += 1;
        }
        (start, end.saturating_sub(1))
    }

    pub fn visual_block_ranges(&self, buf: &Buffer) -> Vec<(usize, usize)> {
        let anchor_row = buf.chars[..self.visual_anchor]
            .iter()
            .filter(|&&c| c == '\n')
            .count();
        let anchor_col = {
            let before = &buf.chars[..self.visual_anchor];
            match before.iter().rposition(|&c| c == '\n') {
                Some(pos) => self.visual_anchor - pos - 1,
                None => self.visual_anchor,
            }
        };
        let cursor_row = buf.cursor_row();
        let cursor_col = buf.cursor_col();

        let first_row = anchor_row.min(cursor_row);
        let last_row = anchor_row.max(cursor_row);
        let left_col = anchor_col.min(cursor_col);
        let right_col = anchor_col.max(cursor_col);

        let mut ranges = Vec::new();
        for row in first_row..=last_row {
            let ls = buf.line_start(row);
            let ll = buf.line_len(row);
            if left_col < ll {
                let start = ls + left_col;
                let end = ls + right_col.min(ll.saturating_sub(1));
                ranges.push((start, end));
            }
        }
        ranges
    }
}
