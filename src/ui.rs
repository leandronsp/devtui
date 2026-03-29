use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::buffer::Buffer;
use crate::preview;
use crate::vim::{Mode, Vim};

pub fn draw(frame: &mut Frame, buf: &mut Buffer, vim: &Vim) {
    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    let panes =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);

    let inner_height = panes[0].height.saturating_sub(2);
    buf.adjust_scroll(inner_height);

    draw_editor(frame, panes[0], buf, vim);
    draw_preview(frame, panes[1], buf);
    draw_status(frame, chunks[1], buf, vim);
}

fn draw_editor(frame: &mut Frame, area: ratatui::layout::Rect, buf: &Buffer, vim: &Vim) {
    let (label, style) = mode_indicator(&vim.mode);

    let title = Line::from(vec![
        Span::styled(label, style),
        Span::raw(" EDITOR"),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(title);

    let lines = editor_lines(buf, vim);
    let widget = Paragraph::new(lines)
        .block(block)
        .scroll((buf.scroll_offset, 0));

    frame.render_widget(widget, area);
}

fn draw_preview(frame: &mut Frame, area: ratatui::layout::Rect, buf: &Buffer) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" PREVIEW ");

    let content = buf.content();
    let lines = preview::render(&content);
    let widget = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((buf.scroll_offset, 0));

    frame.render_widget(widget, area);
}

fn draw_status(frame: &mut Frame, area: ratatui::layout::Rect, buf: &Buffer, vim: &Vim) {
    let line = if vim.mode == Mode::Command {
        Line::from(Span::raw(format!(":{}", vim.command_buf)))
    } else if !vim.status_msg.is_empty() {
        Line::from(Span::styled(
            format!(" {}", vim.status_msg),
            Style::default().fg(Color::Yellow),
        ))
    } else {
        let row = buf.cursor_row() + 1;
        let col = buf.cursor_col() + 1;
        let total = buf.total_lines();
        let dirty = if buf.dirty { " [+]" } else { "" };
        Line::from(Span::styled(
            format!(" {}{} | {}:{} / {}", buf.file_path.display(), dirty, row, col, total),
            Style::default().fg(Color::DarkGray),
        ))
    };

    frame.render_widget(Paragraph::new(line), area);
}

fn mode_indicator(mode: &Mode) -> (&'static str, Style) {
    match mode {
        Mode::Normal => (" NORMAL ", Style::default().fg(Color::Black).bg(Color::Blue)),
        Mode::Insert => (" INSERT ", Style::default().fg(Color::Black).bg(Color::Green)),
        Mode::Visual => (" VISUAL ", Style::default().fg(Color::Black).bg(Color::Magenta)),
        Mode::VisualLine => (" V-LINE ", Style::default().fg(Color::Black).bg(Color::Magenta)),
        Mode::VisualBlock => (" V-BLOCK ", Style::default().fg(Color::Black).bg(Color::Magenta)),
        Mode::Command => (" COMMAND ", Style::default().fg(Color::Black).bg(Color::Yellow)),
    }
}

fn editor_lines<'a>(buf: &'a Buffer, vim: &Vim) -> Vec<Line<'a>> {
    let lines = buf.lines();
    let cursor_row = buf.cursor_row();
    let cursor_col = buf.cursor_col();

    let visual_range = match vim.mode {
        Mode::Visual => {
            let (start, end) = vim.visual_range(buf);
            Some((start, end))
        }
        Mode::VisualLine => {
            let (start, end) = vim.visual_line_range(buf);
            Some((start, end))
        }
        _ => None,
    };

    let block_ranges = if vim.mode == Mode::VisualBlock {
        Some(vim.visual_block_ranges(buf))
    } else {
        None
    };

    let cursor_style = Style::default().bg(Color::White).fg(Color::Black);
    let selection_style = Style::default().bg(Color::LightBlue).fg(Color::Black);
    let mut char_offset = 0;

    lines
        .iter()
        .enumerate()
        .map(|(i, line_chars)| {
            let line_start_offset = char_offset;
            char_offset += line_chars.len() + 1;

            if let Some(ref ranges) = block_ranges {
                // Visual Block: highlight column ranges per row
                let mut spans = Vec::new();
                for (j, &ch) in line_chars.iter().enumerate() {
                    let abs_pos = line_start_offset + j;
                    let is_cursor = i == cursor_row && j == cursor_col;
                    let in_block = ranges.iter().any(|(s, e)| abs_pos >= *s && abs_pos <= *e);

                    let style = if is_cursor {
                        cursor_style
                    } else if in_block {
                        selection_style
                    } else {
                        Style::default()
                    };
                    spans.push(Span::styled(ch.to_string(), style));
                }
                if i == cursor_row && cursor_col >= line_chars.len() {
                    spans.push(Span::styled(" ", cursor_style));
                }
                Line::from(spans)
            } else if let Some((sel_start, sel_end)) = visual_range {
                let mut spans = Vec::new();
                for (j, &ch) in line_chars.iter().enumerate() {
                    let abs_pos = line_start_offset + j;
                    let is_cursor = i == cursor_row && j == cursor_col;
                    let in_selection = abs_pos >= sel_start && abs_pos <= sel_end;

                    let style = if is_cursor {
                        cursor_style
                    } else if in_selection {
                        selection_style
                    } else {
                        Style::default()
                    };
                    spans.push(Span::styled(ch.to_string(), style));
                }
                if i == cursor_row && cursor_col >= line_chars.len() {
                    spans.push(Span::styled(" ", cursor_style));
                }
                Line::from(spans)
            } else if i == cursor_row {
                if cursor_col < line_chars.len() {
                    let before: String = line_chars[..cursor_col].iter().collect();
                    let cursor_char = line_chars[cursor_col].to_string();
                    let after: String = line_chars[cursor_col + 1..].iter().collect();
                    Line::from(vec![
                        Span::raw(before),
                        Span::styled(cursor_char, cursor_style),
                        Span::raw(after),
                    ])
                } else {
                    let text: String = line_chars.iter().collect();
                    Line::from(vec![
                        Span::raw(text),
                        Span::styled(" ", cursor_style),
                    ])
                }
            } else {
                let text: String = line_chars.iter().collect();
                Line::from(text)
            }
        })
        .collect()
}
