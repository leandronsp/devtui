use std::io;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use pulldown_cmark::{Event as MdEvent, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    DefaultTerminal,
};

struct App {
    chars: Vec<char>,
    cursor: usize,
    scroll_offset: u16,
    mode: Mode,
}

#[derive(PartialEq)]
enum Mode {
    Normal,
    Insert,
}

impl App {
    fn new() -> Self {
        Self {
            chars: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            mode: Mode::Normal,
        }
    }

    fn content(&self) -> String {
        self.chars.iter().collect()
    }

    fn lines(&self) -> Vec<Vec<char>> {
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

    fn cursor_row(&self) -> usize {
        self.chars[..self.cursor].iter().filter(|&&c| c == '\n').count()
    }

    fn cursor_col(&self) -> usize {
        let before = &self.chars[..self.cursor];
        match before.iter().rposition(|&c| c == '\n') {
            Some(pos) => self.cursor - pos - 1,
            None => self.cursor,
        }
    }

    fn line_start(&self, row: usize) -> usize {
        let mut start = 0;
        for _ in 0..row {
            match self.chars[start..].iter().position(|&c| c == '\n') {
                Some(pos) => start += pos + 1,
                None => return self.chars.len(),
            }
        }
        start
    }

    fn line_len(&self, row: usize) -> usize {
        let lines = self.lines();
        if row < lines.len() {
            lines[row].len()
        } else {
            0
        }
    }

    fn total_lines(&self) -> usize {
        self.lines().len()
    }

    fn char_at(&self, pos: usize) -> Option<char> {
        self.chars.get(pos).copied()
    }

    fn handle_normal(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        match code {
            KeyCode::Char('i') => self.mode = Mode::Insert,
            KeyCode::Char('a') => {
                if let Some(ch) = self.char_at(self.cursor) {
                    if ch != '\n' {
                        self.cursor += 1;
                    }
                }
                self.mode = Mode::Insert;
            }
            KeyCode::Char('A') => {
                let row = self.cursor_row();
                self.cursor = self.line_start(row) + self.line_len(row);
                self.mode = Mode::Insert;
            }
            KeyCode::Char('I') => {
                let row = self.cursor_row();
                self.cursor = self.line_start(row);
                self.mode = Mode::Insert;
            }
            KeyCode::Char('o') => {
                let row = self.cursor_row();
                let end = self.line_start(row) + self.line_len(row);
                self.chars.insert(end, '\n');
                self.cursor = end + 1;
                self.mode = Mode::Insert;
            }
            KeyCode::Char('O') => {
                let row = self.cursor_row();
                let start = self.line_start(row);
                self.chars.insert(start, '\n');
                self.cursor = start;
                self.mode = Mode::Insert;
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if self.cursor > 0 && self.char_at(self.cursor - 1) != Some('\n') {
                    self.cursor -= 1;
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if self.cursor < self.chars.len() && self.char_at(self.cursor) != Some('\n') {
                    self.cursor += 1;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_vertical(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_vertical(-1),
            KeyCode::Char('0') => {
                let row = self.cursor_row();
                self.cursor = self.line_start(row);
            }
            KeyCode::Char('$') => {
                let row = self.cursor_row();
                let start = self.line_start(row);
                let end = start + self.line_len(row);
                self.cursor = if end > start { end - 1 } else { end };
            }
            KeyCode::Char('G') => {
                let last = self.total_lines() - 1;
                self.cursor = self.line_start(last);
            }
            KeyCode::Char('g') if modifiers == KeyModifiers::NONE => {
                self.cursor = 0;
            }
            KeyCode::Char('w') => self.word_forward(),
            KeyCode::Char('b') => self.word_backward(),
            KeyCode::Char('x') => {
                if self.cursor < self.chars.len() {
                    self.chars.remove(self.cursor);
                    if self.cursor >= self.chars.len() && self.cursor > 0 {
                        self.cursor -= 1;
                    }
                }
            }
            KeyCode::Char('d') if modifiers == KeyModifiers::CONTROL => {
                for _ in 0..10 {
                    self.move_vertical(1);
                }
            }
            KeyCode::Char('u') if modifiers == KeyModifiers::CONTROL => {
                for _ in 0..10 {
                    self.move_vertical(-1);
                }
            }
            _ => {}
        }
    }

    fn handle_insert(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                if self.cursor > 0 && self.char_at(self.cursor - 1) != Some('\n') {
                    self.cursor -= 1;
                }
            }
            KeyCode::Char(c) => {
                self.chars.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Enter => {
                self.chars.insert(self.cursor, '\n');
                self.cursor += 1;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.chars.remove(self.cursor);
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor < self.chars.len() {
                    self.cursor += 1;
                }
            }
            KeyCode::Up => self.move_vertical(-1),
            KeyCode::Down => self.move_vertical(1),
            _ => {}
        }
    }

    fn move_vertical(&mut self, delta: i32) {
        let row = self.cursor_row() as i32;
        let col = self.cursor_col();
        let new_row = (row + delta).max(0) as usize;
        if new_row >= self.total_lines() {
            return;
        }
        let new_col = col.min(self.line_len(new_row));
        self.cursor = self.line_start(new_row) + new_col;
    }

    fn word_forward(&mut self) {
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

    fn word_backward(&mut self) {
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

    fn adjust_scroll(&mut self, viewport_height: u16) {
        let row = self.cursor_row() as u16;
        if row < self.scroll_offset {
            self.scroll_offset = row;
        } else if row >= self.scroll_offset + viewport_height {
            self.scroll_offset = row - viewport_height + 1;
        }
    }

    fn editor_lines(&self) -> Vec<Line<'_>> {
        let lines = self.lines();
        let cursor_row = self.cursor_row();
        let cursor_col = self.cursor_col();

        lines
            .iter()
            .enumerate()
            .map(|(i, line_chars)| {
                let line_str: String = line_chars.iter().collect();
                if i == cursor_row {
                    if cursor_col < line_chars.len() {
                        let before: String = line_chars[..cursor_col].iter().collect();
                        let cursor_char = line_chars[cursor_col].to_string();
                        let after: String = line_chars[cursor_col + 1..].iter().collect();
                        Line::from(vec![
                            Span::raw(before),
                            Span::styled(
                                cursor_char,
                                Style::default().bg(Color::White).fg(Color::Black),
                            ),
                            Span::raw(after),
                        ])
                    } else {
                        Line::from(vec![
                            Span::raw(line_str),
                            Span::styled(
                                " ",
                                Style::default().bg(Color::White).fg(Color::Black),
                            ),
                        ])
                    }
                } else {
                    Line::from(line_str)
                }
            })
            .collect()
    }

    fn render_preview(&self) -> Vec<Line<'_>> {
        let content = self.content();
        let options = Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TABLES
            | Options::ENABLE_FOOTNOTES;

        let parser = Parser::new_ext(&content, options);
        let mut lines: Vec<Line> = Vec::new();
        let mut current_spans: Vec<Span> = Vec::new();
        let mut style_stack: Vec<Style> = vec![Style::default()];

        for event in parser {
            match event {
                MdEvent::Start(tag) => {
                    let style = match &tag {
                        Tag::Heading { level, .. } => {
                            let prefix = match level {
                                HeadingLevel::H1 => "# ",
                                HeadingLevel::H2 => "## ",
                                HeadingLevel::H3 => "### ",
                                _ => "#### ",
                            };
                            current_spans.push(Span::styled(
                                prefix.to_string(),
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            ));
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        }
                        Tag::Emphasis => Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                        Tag::Strong => Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                        Tag::Strikethrough => {
                            Style::default().add_modifier(Modifier::CROSSED_OUT)
                        }
                        Tag::CodeBlock(_) => Style::default().fg(Color::Red),
                        Tag::BlockQuote(_) => {
                            current_spans.push(Span::styled(
                                "  > ",
                                Style::default().fg(Color::DarkGray),
                            ));
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::ITALIC)
                        }
                        Tag::List(_) => Style::default(),
                        Tag::Item => {
                            current_spans.push(Span::styled(
                                "  - ",
                                Style::default().fg(Color::Magenta),
                            ));
                            Style::default()
                        }
                        Tag::Link { dest_url, .. } => {
                            current_spans
                                .push(Span::styled("[", Style::default().fg(Color::Blue)));
                            let _ = dest_url;
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::UNDERLINED)
                        }
                        _ => *style_stack.last().unwrap_or(&Style::default()),
                    };
                    style_stack.push(style);
                }
                MdEvent::End(tag_end) => {
                    style_stack.pop();
                    match tag_end {
                        TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::BlockQuote(_) => {
                            lines.push(Line::from(
                                current_spans.drain(..).collect::<Vec<_>>(),
                            ));
                            lines.push(Line::from(""));
                        }
                        TagEnd::Item => {
                            lines.push(Line::from(
                                current_spans.drain(..).collect::<Vec<_>>(),
                            ));
                        }
                        TagEnd::CodeBlock => {
                            lines.push(Line::from(
                                current_spans.drain(..).collect::<Vec<_>>(),
                            ));
                            lines.push(Line::from(""));
                        }
                        TagEnd::Link => {
                            current_spans
                                .push(Span::styled("]", Style::default().fg(Color::Blue)));
                        }
                        _ => {}
                    }
                }
                MdEvent::Text(text) => {
                    let style = *style_stack.last().unwrap_or(&Style::default());
                    current_spans.push(Span::styled(text.to_string(), style));
                }
                MdEvent::Code(code) => {
                    current_spans.push(Span::styled(
                        format!("`{}`", code),
                        Style::default().fg(Color::Red),
                    ));
                }
                MdEvent::SoftBreak | MdEvent::HardBreak => {
                    lines.push(Line::from(current_spans.drain(..).collect::<Vec<_>>()));
                }
                MdEvent::Rule => {
                    lines.push(Line::from(current_spans.drain(..).collect::<Vec<_>>()));
                    lines.push(Line::from(Span::styled(
                        "────────────────────────────────",
                        Style::default().fg(Color::DarkGray),
                    )));
                    lines.push(Line::from(""));
                }
                _ => {}
            }
        }

        if !current_spans.is_empty() {
            lines.push(Line::from(current_spans));
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "Start typing...",
                Style::default().fg(Color::DarkGray),
            )));
        }

        lines
    }
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();

    loop {
        terminal.draw(|frame| {
            let chunks =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(frame.area());

            let editor_area = chunks[0];
            let inner_height = editor_area.height.saturating_sub(2);
            app.adjust_scroll(inner_height);

            let mode_label = match app.mode {
                Mode::Normal => " NORMAL ",
                Mode::Insert => " INSERT ",
            };
            let mode_style = match app.mode {
                Mode::Normal => Style::default().fg(Color::Black).bg(Color::Blue),
                Mode::Insert => Style::default().fg(Color::Black).bg(Color::Green),
            };

            let editor_title = Line::from(vec![
                Span::styled(mode_label, mode_style),
                Span::raw(" EDITOR"),
            ]);

            let editor_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(editor_title);

            let editor_lines = app.editor_lines();
            let editor = Paragraph::new(editor_lines)
                .block(editor_block)
                .scroll((app.scroll_offset, 0));

            frame.render_widget(editor, chunks[0]);

            let preview_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" PREVIEW ");

            let preview_lines = app.render_preview();
            let preview = Paragraph::new(preview_lines)
                .block(preview_block)
                .wrap(Wrap { trim: false })
                .scroll((app.scroll_offset, 0));

            frame.render_widget(preview, chunks[1]);
        })?;

        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                break;
            }

            match app.mode {
                Mode::Normal => {
                    if key.code == KeyCode::Char('q') {
                        break;
                    }
                    app.handle_normal(key.code, key.modifiers);
                }
                Mode::Insert => app.handle_insert(key.code),
            }
        }
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result
}
