use std::fs;
use std::path::PathBuf;

pub struct Buffer {
    pub chars: Vec<char>,
    pub cursor: usize,
    pub scroll_offset: u16,
    pub file_path: PathBuf,
    pub dirty: bool,
    undo_stack: Vec<(Vec<char>, usize)>,
    redo_stack: Vec<(Vec<char>, usize)>,
}

impl Buffer {
    pub fn open(file_path: PathBuf) -> Self {
        let chars = fs::read_to_string(&file_path)
            .map(|s| s.chars().collect())
            .unwrap_or_default();

        Self {
            chars,
            cursor: 0,
            scroll_offset: 0,
            file_path,
            dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn content(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn save(&mut self) -> Result<(), String> {
        fs::write(&self.file_path, self.content())
            .map(|()| self.dirty = false)
            .map_err(|e| e.to_string())
    }

    pub fn lines(&self) -> Vec<Vec<char>> {
        let mut result: Vec<Vec<char>> = vec![Vec::new()];
        for &ch in &self.chars {
            if ch == '\n' {
                result.push(Vec::new());
            } else {
                result.last_mut().unwrap().push(ch);
            }
        }
        result
    }

    pub fn cursor_row(&self) -> usize {
        self.chars[..self.cursor]
            .iter()
            .filter(|&&c| c == '\n')
            .count()
    }

    pub fn cursor_col(&self) -> usize {
        let before = &self.chars[..self.cursor];
        match before.iter().rposition(|&c| c == '\n') {
            Some(pos) => self.cursor - pos - 1,
            None => self.cursor,
        }
    }

    pub fn line_start(&self, row: usize) -> usize {
        let mut start = 0;
        for _ in 0..row {
            match self.chars[start..].iter().position(|&c| c == '\n') {
                Some(pos) => start += pos + 1,
                None => return self.chars.len(),
            }
        }
        start
    }

    pub fn line_len(&self, row: usize) -> usize {
        let lines = self.lines();
        if row < lines.len() {
            lines[row].len()
        } else {
            0
        }
    }

    pub fn total_lines(&self) -> usize {
        self.lines().len()
    }

    pub fn char_at(&self, pos: usize) -> Option<char> {
        self.chars.get(pos).copied()
    }

    // Undo/Redo

    pub fn snapshot(&mut self) {
        self.undo_stack.push((self.chars.clone(), self.cursor));
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some((chars, cursor)) = self.undo_stack.pop() {
            self.redo_stack.push((self.chars.clone(), self.cursor));
            self.chars = chars;
            self.cursor = cursor.min(self.chars.len().saturating_sub(1));
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some((chars, cursor)) = self.redo_stack.pop() {
            self.undo_stack.push((self.chars.clone(), self.cursor));
            self.chars = chars;
            self.cursor = cursor.min(self.chars.len().saturating_sub(1));
            self.dirty = true;
            true
        } else {
            false
        }
    }

    // Navigation

    pub fn move_left(&mut self) {
        if self.cursor > 0 && self.char_at(self.cursor - 1) != Some('\n') {
            self.cursor -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.chars.len() && self.char_at(self.cursor) != Some('\n') {
            self.cursor += 1;
        }
    }

    pub fn move_vertical(&mut self, delta: i32) {
        let row = self.cursor_row() as i32;
        let col = self.cursor_col();
        let new_row = (row + delta).max(0) as usize;
        if new_row >= self.total_lines() {
            return;
        }
        let new_col = col.min(self.line_len(new_row));
        self.cursor = self.line_start(new_row) + new_col;
    }

    pub fn move_to_line_start(&mut self) {
        let row = self.cursor_row();
        self.cursor = self.line_start(row);
    }

    pub fn move_to_line_end(&mut self) {
        let row = self.cursor_row();
        let start = self.line_start(row);
        let end = start + self.line_len(row);
        self.cursor = if end > start { end - 1 } else { end };
    }

    pub fn move_to_top(&mut self) {
        self.cursor = 0;
    }

    pub fn move_to_bottom(&mut self) {
        let last = self.total_lines() - 1;
        self.cursor = self.line_start(last);
    }

    pub fn word_forward(&mut self) {
        let len = self.chars.len();
        let mut pos = self.cursor;
        while pos < len && self.chars[pos] != ' ' && self.chars[pos] != '\n' {
            pos += 1;
        }
        while pos < len && (self.chars[pos] == ' ' || self.chars[pos] == '\n') {
            pos += 1;
        }
        self.cursor = pos;
    }

    pub fn word_backward(&mut self) {
        let mut pos = self.cursor;
        if pos > 0 {
            pos -= 1;
        }
        while pos > 0 && (self.chars[pos] == ' ' || self.chars[pos] == '\n') {
            pos -= 1;
        }
        while pos > 0 && self.chars[pos - 1] != ' ' && self.chars[pos - 1] != '\n' {
            pos -= 1;
        }
        self.cursor = pos;
    }

    pub fn adjust_scroll(&mut self, viewport_height: u16) {
        let row = self.cursor_row() as u16;
        if row < self.scroll_offset {
            self.scroll_offset = row;
        } else if row >= self.scroll_offset + viewport_height {
            self.scroll_offset = row - viewport_height + 1;
        }
    }

    // Mutations

    pub fn insert_char(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
        self.dirty = true;
    }

    pub fn delete_char_backward(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
            self.dirty = true;
        }
    }

    pub fn delete_char_at_cursor(&mut self) -> Option<char> {
        if self.cursor < self.chars.len() {
            let ch = self.chars.remove(self.cursor);
            if self.cursor >= self.chars.len() && self.cursor > 0 {
                self.cursor -= 1;
            }
            self.dirty = true;
            Some(ch)
        } else {
            None
        }
    }

    pub fn open_line_below(&mut self) {
        let row = self.cursor_row();
        let end = self.line_start(row) + self.line_len(row);
        self.chars.insert(end, '\n');
        self.cursor = end + 1;
        self.dirty = true;
    }

    pub fn open_line_above(&mut self) {
        let row = self.cursor_row();
        let start = self.line_start(row);
        self.chars.insert(start, '\n');
        self.cursor = start;
        self.dirty = true;
    }

    pub fn delete_line(&mut self) -> Vec<char> {
        let row = self.cursor_row();
        let start = self.line_start(row);
        let mut end = start + self.line_len(row);
        if end < self.chars.len() && self.chars[end] == '\n' {
            end += 1;
        }
        let removed: Vec<char> = self.chars.drain(start..end).collect();
        self.dirty = true;

        if self.chars.is_empty() {
            self.cursor = 0;
        } else {
            self.cursor = start.min(self.chars.len() - 1);
            let new_row = self.cursor_row();
            self.cursor = self.line_start(new_row);
        }
        removed
    }

    pub fn yank_line(&self) -> Vec<char> {
        let row = self.cursor_row();
        let start = self.line_start(row);
        let mut end = start + self.line_len(row);
        if end < self.chars.len() && self.chars[end] == '\n' {
            end += 1;
        }
        self.chars[start..end].to_vec()
    }

    pub fn delete_range(&mut self, start: usize, end: usize) -> Vec<char> {
        let removed: Vec<char> = self
            .chars
            .drain(start..=end.min(self.chars.len() - 1))
            .collect();
        self.cursor = start.min(self.chars.len().saturating_sub(1));
        self.dirty = true;
        removed
    }

    pub fn paste_after(&mut self, register: &[char]) {
        if register.is_empty() {
            return;
        }
        let has_newline = register.contains(&'\n');
        if has_newline {
            let row = self.cursor_row();
            let end = self.line_start(row) + self.line_len(row);
            let insert_pos = if end < self.chars.len() {
                end + 1
            } else {
                self.chars.push('\n');
                self.chars.len()
            };
            for (i, &ch) in register.iter().enumerate() {
                self.chars.insert(insert_pos + i, ch);
            }
            self.cursor = insert_pos;
        } else {
            let insert_pos = (self.cursor + 1).min(self.chars.len());
            for (i, &ch) in register.iter().enumerate() {
                self.chars.insert(insert_pos + i, ch);
            }
            self.cursor = insert_pos + register.len() - 1;
        }
        self.dirty = true;
    }

    pub fn word_end(&mut self) {
        let len = self.chars.len();
        let mut pos = self.cursor;
        if pos < len {
            pos += 1;
        }
        while pos < len && (self.chars[pos] == ' ' || self.chars[pos] == '\n') {
            pos += 1;
        }
        while pos < len
            && pos + 1 < len
            && self.chars[pos + 1] != ' '
            && self.chars[pos + 1] != '\n'
        {
            pos += 1;
        }
        self.cursor = pos.min(len.saturating_sub(1));
    }

    /// Position after word_forward (exclusive end for operator+motion)
    pub fn word_end_pos(&self) -> usize {
        let len = self.chars.len();
        let mut pos = self.cursor;
        while pos < len && self.chars[pos] != ' ' && self.chars[pos] != '\n' {
            pos += 1;
        }
        pos
    }

    pub fn line_end_pos(&self) -> usize {
        let row = self.cursor_row();
        self.line_start(row) + self.line_len(row)
    }

    pub fn replace_char(&mut self, c: char) {
        if self.cursor < self.chars.len() && self.chars[self.cursor] != '\n' {
            self.chars[self.cursor] = c;
            self.dirty = true;
        }
    }

    pub fn delete_to_end_of_line(&mut self) -> Vec<char> {
        let end = self.line_end_pos();
        if self.cursor >= end {
            return Vec::new();
        }
        let removed: Vec<char> = self.chars.drain(self.cursor..end).collect();
        self.dirty = true;
        if self.cursor > 0 && self.cursor >= self.chars.len() {
            self.cursor -= 1;
        }
        removed
    }

    pub fn delete_to_start_of_line(&mut self) -> Vec<char> {
        let row = self.cursor_row();
        let start = self.line_start(row);
        if self.cursor <= start {
            return Vec::new();
        }
        let removed: Vec<char> = self.chars.drain(start..self.cursor).collect();
        self.cursor = start;
        self.dirty = true;
        removed
    }

    pub fn delete_word_forward(&mut self) -> Vec<char> {
        let end = self.word_end_pos();
        // also eat trailing whitespace
        let mut actual_end = end;
        while actual_end < self.chars.len()
            && (self.chars[actual_end] == ' ')
        {
            actual_end += 1;
        }
        if self.cursor >= actual_end {
            return Vec::new();
        }
        let removed: Vec<char> = self.chars.drain(self.cursor..actual_end).collect();
        self.dirty = true;
        if self.cursor >= self.chars.len() && self.cursor > 0 {
            self.cursor -= 1;
        }
        removed
    }

    pub fn change_word_forward(&mut self) -> Vec<char> {
        let end = self.word_end_pos();
        if self.cursor >= end {
            return Vec::new();
        }
        let removed: Vec<char> = self.chars.drain(self.cursor..end).collect();
        self.dirty = true;
        removed
    }

    pub fn change_to_end_of_line(&mut self) -> Vec<char> {
        let end = self.line_end_pos();
        if self.cursor >= end {
            return Vec::new();
        }
        let removed: Vec<char> = self.chars.drain(self.cursor..end).collect();
        self.dirty = true;
        removed
    }

    pub fn change_line(&mut self) -> Vec<char> {
        let row = self.cursor_row();
        let start = self.line_start(row);
        let end = start + self.line_len(row);
        let removed: Vec<char> = self.chars.drain(start..end).collect();
        self.cursor = start;
        self.dirty = true;
        removed
    }

    pub fn join_line(&mut self) {
        let row = self.cursor_row();
        let end = self.line_start(row) + self.line_len(row);
        if end < self.chars.len() && self.chars[end] == '\n' {
            self.chars[end] = ' ';
            // remove leading whitespace from joined line
            while end + 1 < self.chars.len() && self.chars[end + 1] == ' ' {
                self.chars.remove(end + 1);
            }
            self.cursor = end;
            self.dirty = true;
        }
    }

    pub fn toggle_case(&mut self) {
        if self.cursor < self.chars.len() {
            let ch = self.chars[self.cursor];
            if ch.is_alphabetic() {
                self.chars[self.cursor] = if ch.is_uppercase() {
                    ch.to_lowercase().next().unwrap_or(ch)
                } else {
                    ch.to_uppercase().next().unwrap_or(ch)
                };
                self.dirty = true;
            }
            self.move_right();
        }
    }

    pub fn paste_before(&mut self, register: &[char]) {
        if register.is_empty() {
            return;
        }
        let has_newline = register.contains(&'\n');
        if has_newline {
            let row = self.cursor_row();
            let insert_pos = self.line_start(row);
            for (i, &ch) in register.iter().enumerate() {
                self.chars.insert(insert_pos + i, ch);
            }
            self.cursor = insert_pos;
        } else {
            for (i, &ch) in register.iter().enumerate() {
                self.chars.insert(self.cursor + i, ch);
            }
            self.cursor += register.len().saturating_sub(1);
        }
        self.dirty = true;
    }
}
