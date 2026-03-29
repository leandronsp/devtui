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
    let mut list_depth: usize = 0;
    let mut in_blockquote = false;

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
            Event::Start(ref tag) => {
                if matches!(tag, Tag::List(_)) && !spans.is_empty() {
                    lines.push(Line::from(spans.drain(..).collect::<Vec<_>>()));
                }
                if matches!(tag, Tag::BlockQuote(_)) {
                    in_blockquote = true;
                }
                let style = style_for_tag(tag, &mut spans, &mut list_depth);
                style_stack.push(style);
            }
            Event::End(tag_end) => {
                style_stack.pop();
                if matches!(tag_end, TagEnd::BlockQuote(_)) {
                    in_blockquote = false;
                }
                handle_tag_end(tag_end, &mut spans, &mut lines, &mut list_depth);
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
                if in_blockquote {
                    spans.push(Span::styled(
                        "  > ",
                        Style::default().fg(Color::DarkGray),
                    ));
                }
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


fn style_for_tag<'a>(tag: &Tag<'a>, spans: &mut Vec<Span<'a>>, list_depth: &mut usize) -> Style {
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
        Tag::List(_) => {
            *list_depth += 1;
            Style::default()
        }
        Tag::Item => {
            let indent = "  ".repeat(*list_depth);
            spans.push(Span::styled(
                format!("{}- ", indent),
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

fn handle_tag_end<'a>(tag_end: TagEnd, spans: &mut Vec<Span<'a>>, lines: &mut Vec<Line<'a>>, list_depth: &mut usize) {
    match tag_end {
        TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::BlockQuote(_) => {
            lines.push(Line::from(spans.drain(..).collect::<Vec<_>>()));
            lines.push(Line::from(""));
        }
        TagEnd::List(_) => {
            *list_depth = list_depth.saturating_sub(1);
            if *list_depth == 0 {
                lines.push(Line::from(""));
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn all_text(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| line_text(l)).collect()
    }

    // Empty / minimal

    #[test]
    fn empty_content_shows_placeholder() {
        let (lines, _) = render_with_offsets("");
        assert_eq!(line_text(&lines[0]), "Start typing...");
    }

    #[test]
    fn plain_text_renders_as_paragraph() {
        let (lines, _) = render_with_offsets("Hello world");
        assert_eq!(line_text(&lines[0]), "Hello world");
    }

    // Headings

    #[test]
    fn heading_h1() {
        let (lines, _) = render_with_offsets("# Title");
        let text = line_text(&lines[0]);
        assert!(text.contains("# "), "H1 prefix");
        assert!(text.contains("Title"));
        assert_eq!(line_text(&lines[1]), "", "blank line after heading");
    }

    #[test]
    fn heading_h2() {
        let (lines, _) = render_with_offsets("## Subtitle");
        assert!(line_text(&lines[0]).contains("## "));
    }

    #[test]
    fn heading_h3() {
        let (lines, _) = render_with_offsets("### Section");
        assert!(line_text(&lines[0]).contains("### "));
    }

    #[test]
    fn heading_h4() {
        let (lines, _) = render_with_offsets("#### Deep");
        assert!(line_text(&lines[0]).contains("#### "));
    }

    // Inline formatting

    #[test]
    fn bold_text() {
        let (lines, _) = render_with_offsets("Some **bold** here");
        let text = line_text(&lines[0]);
        assert!(text.contains("bold"));
    }

    #[test]
    fn italic_text() {
        let (lines, _) = render_with_offsets("Some *italic* here");
        let text = line_text(&lines[0]);
        assert!(text.contains("italic"));
    }

    #[test]
    fn strikethrough_text() {
        let (lines, _) = render_with_offsets("Some ~~deleted~~ here");
        let text = line_text(&lines[0]);
        assert!(text.contains("deleted"));
    }

    #[test]
    fn inline_code() {
        let (lines, _) = render_with_offsets("Use `mix test` to run");
        let text = line_text(&lines[0]);
        assert!(text.contains("`mix test`"));
    }

    #[test]
    fn mixed_formatting() {
        let (lines, _) = render_with_offsets("**bold** and *italic* and `code`");
        let text = line_text(&lines[0]);
        assert!(text.contains("bold"));
        assert!(text.contains("italic"));
        assert!(text.contains("`code`"));
    }

    // Lists

    #[test]
    fn simple_list() {
        let (lines, _) = render_with_offsets("- One\n- Two\n- Three");
        let texts = all_text(&lines);
        assert!(texts[0].contains("- ") && texts[0].contains("One"));
        assert!(texts[1].contains("- ") && texts[1].contains("Two"));
        assert!(texts[2].contains("- ") && texts[2].contains("Three"));
    }

    #[test]
    fn nested_list_two_levels() {
        let (lines, _) = render_with_offsets("- Parent\n  - Child A\n  - Child B");
        let texts = all_text(&lines);
        assert!(texts[0].contains("Parent"), "parent item");
        assert!(texts[1].contains("Child A"), "first child");
        assert!(texts[2].contains("Child B"), "second child");
        // Child should have more indentation than parent
        let parent_indent = texts[0].find('-').unwrap();
        let child_indent = texts[1].find('-').unwrap();
        assert!(child_indent > parent_indent, "child should be more indented");
    }

    #[test]
    fn nested_list_three_levels() {
        let md = "- L1\n  - L2\n    - L3";
        let (lines, _) = render_with_offsets(md);
        let texts = all_text(&lines);
        assert!(texts[0].contains("L1"));
        assert!(texts[1].contains("L2"));
        assert!(texts[2].contains("L3"));
        let i1 = texts[0].find('-').unwrap();
        let i2 = texts[1].find('-').unwrap();
        let i3 = texts[2].find('-').unwrap();
        assert!(i3 > i2 && i2 > i1, "progressive indentation");
    }

    #[test]
    fn list_items_each_on_own_line() {
        let md = "- Alpha\n- Beta\n  - Gamma\n  - Delta\n- Epsilon";
        let (lines, _) = render_with_offsets(md);
        let texts = all_text(&lines);
        // No line should contain two item markers
        for text in &texts {
            let dash_count = text.matches("- ").count();
            assert!(dash_count <= 1, "Line has multiple items: |{}|", text);
        }
    }

    // Blockquotes

    #[test]
    fn blockquote_single_line() {
        let (lines, _) = render_with_offsets("> Hello");
        let text = line_text(&lines[0]);
        assert!(text.contains(">"), "quote marker");
        assert!(text.contains("Hello"));
    }

    #[test]
    fn blockquote_multiline() {
        let (lines, _) = render_with_offsets("> Line one\n> Line two");
        let texts = all_text(&lines);
        assert!(texts[0].contains(">") && texts[0].contains("Line one"));
        assert!(texts[1].contains(">") && texts[1].contains("Line two"));
    }

    // Code blocks

    #[test]
    fn fenced_code_block() {
        let md = "```\nlet x = 1;\nlet y = 2;\n```";
        let (lines, _) = render_with_offsets(md);
        let combined: String = all_text(&lines).join("\n");
        assert!(combined.contains("let x = 1;"));
        assert!(combined.contains("let y = 2;"));
    }

    #[test]
    fn code_block_with_language() {
        let md = "```rust\nfn main() {}\n```";
        let (lines, _) = render_with_offsets(md);
        let combined: String = all_text(&lines).join("\n");
        assert!(combined.contains("fn main()"));
    }

    // Horizontal rule

    #[test]
    fn horizontal_rule() {
        let md = "Above\n\n---\n\nBelow";
        let (lines, _) = render_with_offsets(md);
        let combined: String = all_text(&lines).join("\n");
        assert!(combined.contains("────"), "rule line");
        assert!(combined.contains("Above"));
        assert!(combined.contains("Below"));
    }

    // Links

    #[test]
    fn inline_link() {
        let md = "Visit [my site](https://example.com) today";
        let (lines, _) = render_with_offsets(md);
        let text = line_text(&lines[0]);
        assert!(text.contains("["), "link opening bracket");
        assert!(text.contains("my site"));
        assert!(text.contains("]"), "link closing bracket");
    }

    // Paragraphs

    #[test]
    fn paragraphs_separated_by_blank_line() {
        let md = "First paragraph.\n\nSecond paragraph.";
        let (lines, _) = render_with_offsets(md);
        let texts = all_text(&lines);
        assert!(texts[0].contains("First"));
        assert_eq!(texts[1], "", "blank separator");
        assert!(texts[2].contains("Second"));
    }

    // Source-to-rendered offset mapping

    #[test]
    fn offset_mapping_plain_lines() {
        let md = "line one\nline two\nline three";
        let (_, offsets) = render_with_offsets(md);
        // Source line 0 -> rendered line 0
        assert_eq!(offsets[0], 0);
    }

    #[test]
    fn offset_mapping_with_heading() {
        let md = "# Title\n\nParagraph here";
        let (lines, offsets) = render_with_offsets(md);
        // Title renders to line 0, blank line at 1, paragraph at 2
        // Source line 2 ("Paragraph here") should map to rendered line 2
        assert!(offsets[2] >= 2, "paragraph offset after heading: {}", offsets[2]);
        let texts = all_text(&lines);
        assert!(texts[0].contains("Title"));
    }

    // Full article integration

    #[test]
    fn full_article_renders_all_sections() {
        let md = std::fs::read_to_string("test-article.md").unwrap();
        let (lines, offsets) = render_with_offsets(&md);
        let combined: String = all_text(&lines).join("\n");

        // All headings present
        assert!(combined.contains("# The Art of Writing Clean Code"));
        assert!(combined.contains("## Principles That Matter"));
        assert!(combined.contains("## Code Examples"));
        assert!(combined.contains("## Formatting"));
        assert!(combined.contains("## Lists Galore"));
        assert!(combined.contains("## Final Thoughts"));

        // Formatting preserved
        assert!(combined.contains("communication"));
        assert!(combined.contains("bold italic"));
        assert!(combined.contains("strikethrough"));

        // Lists rendered
        assert!(combined.contains("Keep functions small"));
        assert!(combined.contains("Elixir"));
        assert!(combined.contains("Level 4"));

        // Code block
        assert!(combined.contains("defmodule Blog"));

        // Blockquote
        assert!(combined.contains(">"));

        // Rule
        assert!(combined.contains("────"));

        // Offsets are monotonically increasing
        for i in 1..offsets.len() {
            assert!(offsets[i] >= offsets[i - 1], "offsets not monotonic at {}", i);
        }

        assert!(lines.len() > 50, "full article should have many rendered lines");
    }
}
