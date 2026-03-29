use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Returns (rendered_lines, source_to_rendered_offset).
/// source_to_rendered_offset[i] = how many rendered lines exist before source line i.
pub fn render_with_offsets(content: &str) -> (Vec<Line<'_>>, Vec<u16>) {
    let source_line_count = content.lines().count().max(1);
    let mut source_to_rendered: Vec<u16> = vec![0; source_line_count + 1];

    let options =
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES;

    let parser = Parser::new_ext(content, options).into_offset_iter();
    let mut lines: Vec<Line> = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut current_source_line: usize = 0;

    for (event, range) in parser {
        // Track which source line this event comes from
        let byte_offset = range.start;
        let src_line = content[..byte_offset].matches('\n').count();
        if src_line != current_source_line {
            // Record rendered line count at each source line boundary
            for sl in (current_source_line + 1)..=src_line.min(source_line_count) {
                source_to_rendered[sl] = lines.len() as u16;
            }
            current_source_line = src_line;
        }

        match event {
            Event::Start(tag) => {
                let style = style_for_tag(&tag, &mut spans);
                style_stack.push(style);
            }
            Event::End(tag_end) => {
                style_stack.pop();
                handle_tag_end(tag_end, &mut spans, &mut lines);
            }
            Event::Text(text) => {
                let style = *style_stack.last().unwrap_or(&Style::default());
                spans.push(Span::styled(text.to_string(), style));
            }
            Event::Code(code) => {
                spans.push(Span::styled(
                    format!("`{}`", code),
                    Style::default().fg(Color::Red),
                ));
            }
            Event::SoftBreak | Event::HardBreak => {
                lines.push(Line::from(spans.drain(..).collect::<Vec<_>>()));
            }
            Event::Rule => {
                lines.push(Line::from(spans.drain(..).collect::<Vec<_>>()));
                lines.push(Line::from(Span::styled(
                    "────────────────────────────────",
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(""));
            }
            _ => {}
        }
    }

    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }

    // Fill remaining source lines
    for sl in (current_source_line + 1)..=source_line_count {
        source_to_rendered[sl] = lines.len() as u16;
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "Start typing...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    (lines, source_to_rendered)
}


fn style_for_tag<'a>(tag: &Tag<'a>, spans: &mut Vec<Span<'a>>) -> Style {
    match tag {
        Tag::Heading { level, .. } => {
            let prefix = match level {
                HeadingLevel::H1 => "# ",
                HeadingLevel::H2 => "## ",
                HeadingLevel::H3 => "### ",
                _ => "#### ",
            };
            let style = Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD);
            spans.push(Span::styled(prefix.to_string(), style));
            style
        }
        Tag::Emphasis => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::ITALIC),
        Tag::Strong => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        Tag::Strikethrough => Style::default().add_modifier(Modifier::CROSSED_OUT),
        Tag::CodeBlock(_) => Style::default().fg(Color::Red),
        Tag::BlockQuote(_) => {
            spans.push(Span::styled(
                "  > ",
                Style::default().fg(Color::DarkGray),
            ));
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC)
        }
        Tag::List(_) => Style::default(),
        Tag::Item => {
            spans.push(Span::styled(
                "  - ",
                Style::default().fg(Color::Magenta),
            ));
            Style::default()
        }
        Tag::Link { .. } => {
            spans.push(Span::styled("[", Style::default().fg(Color::Blue)));
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED)
        }
        _ => Style::default(),
    }
}

fn handle_tag_end<'a>(tag_end: TagEnd, spans: &mut Vec<Span<'a>>, lines: &mut Vec<Line<'a>>) {
    match tag_end {
        TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::BlockQuote(_) => {
            lines.push(Line::from(spans.drain(..).collect::<Vec<_>>()));
            lines.push(Line::from(""));
        }
        TagEnd::Item => {
            lines.push(Line::from(spans.drain(..).collect::<Vec<_>>()));
        }
        TagEnd::CodeBlock => {
            lines.push(Line::from(spans.drain(..).collect::<Vec<_>>()));
            lines.push(Line::from(""));
        }
        TagEnd::Link => {
            spans.push(Span::styled("]", Style::default().fg(Color::Blue)));
        }
        _ => {}
    }
}
