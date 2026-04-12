use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

fn h1_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

fn strip_frontmatter(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("---\n") else {
        return content;
    };
    let Some(end) = rest.find("\n---\n") else {
        return content;
    };
    &rest[end + "\n---\n".len()..]
}

fn frontmatter_date(content: &str) -> Option<String> {
    let raw = frontmatter_field(content, "published_at")
        .or_else(|| frontmatter_field(content, "date"))?;
    let head = raw.get(..10)?;
    if head.chars().all(|c| c.is_ascii_digit() || c == '-') {
        Some(head.to_string())
    } else {
        Some(raw)
    }
}

/// Replace or insert a field in the YAML frontmatter block.
/// Returns the content unchanged if no frontmatter block is found.
pub(crate) fn set_frontmatter_field(content: &str, field: &str, value: &str) -> String {
    let Some(rest) = content.strip_prefix("---\n") else {
        return content.to_string();
    };
    let Some(end) = rest.find("\n---") else {
        return content.to_string();
    };
    let prefix = format!("{field}:");
    let mut lines: Vec<String> = rest[..end].lines().map(|l| l.to_string()).collect();
    let mut found = false;
    for line in &mut lines {
        if line.trim_start().starts_with(&prefix) {
            *line = format!("{field}: {value}");
            found = true;
            break;
        }
    }
    if !found {
        lines.push(format!("{field}: {value}"));
    }
    let mut result = String::from("---\n");
    for line in &lines {
        result.push_str(line);
        result.push('\n');
    }
    result.push_str(&rest[end + 1..]);
    result
}

pub(crate) fn frontmatter_field(content: &str, field: &str) -> Option<String> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    let prefix = format!("{field}:");
    rest[..end].lines().find_map(|line| {
        let value = line.trim_start().strip_prefix(&prefix)?.trim();
        let value = value.trim_matches('"');
        if value.is_empty() { None } else { Some(value.to_string()) }
    })
}

/// Returns (rendered_lines, source_to_rendered_offset).
/// source_to_rendered_offset[i] = how many rendered lines exist before source line i.
pub fn render_with_offsets(
    content: &str,
    author: Option<&str>,
) -> (Vec<Line<'static>>, Vec<u16>) {
    let frontmatter_author = frontmatter_field(content, "author");
    let header_author = frontmatter_author.as_deref().or(author);
    let title = frontmatter_field(content, "title");
    let subtitle = frontmatter_field(content, "subtitle");
    let date = frontmatter_date(content);
    let language = frontmatter_field(content, "language").map(|s| s.to_uppercase());
    let date_line = match (&date, &language) {
        (Some(d), Some(l)) => Some(format!("{d} · {l}")),
        (Some(d), None) => Some(d.clone()),
        (None, Some(l)) => Some(l.clone()),
        (None, None) => None,
    };
    let original_line_count = content.lines().count().max(1);
    let body = strip_frontmatter(content);
    let frontmatter_line_count = original_line_count - body.lines().count().max(1);
    let content = body;
    let source_line_count = content.lines().count().max(1);
    let has_header = header_author.is_some() || title.is_some() || date_line.is_some();
    let header_len: u16 = header_author.is_some() as u16
        + title.is_some() as u16
        + subtitle.is_some() as u16
        + date_line.is_some() as u16
        + has_header as u16;
    let mut source_to_rendered: Vec<u16> = vec![header_len; source_line_count + 1];

    let options =
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES;

    let parser = Parser::new_ext(content, options).into_offset_iter();
    let mut lines: Vec<Line<'static>> = Vec::new();
    if has_header {
        let dim = Style::default().fg(Color::DarkGray);
        if let Some(author) = header_author {
            lines.push(Line::from(Span::styled(author.to_string(), dim)));
        }
        if let Some(title) = title {
            lines.push(Line::from(Span::styled(title, h1_style())));
        }
        if let Some(subtitle) = subtitle {
            let subtitle_style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC);
            lines.push(Line::from(Span::styled(subtitle, subtitle_style)));
        }
        if let Some(date_line) = date_line {
            lines.push(Line::from(Span::styled(date_line, dim)));
        }
        lines.push(Line::from(""));
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut current_source_line: usize = 0;
    let mut list_depth: usize = 0;
    let mut in_blockquote = false;
    let mut in_code_block = false;

    for (event, range) in parser {
        // Track which source line this event comes from
        let byte_offset = range.start;
        let src_line = content[..byte_offset].matches('\n').count();
        if src_line != current_source_line && src_line > current_source_line {
            // Preserve source vertical rhythm: pad rendered `lines` with the
            // blank lines the user actually typed between blocks. pulldown-cmark
            // collapses runs of blank lines at the parser level, so without
            // this padding the preview drifts relative to vim's scroll.
            // handle_tag_end already added one blank via push_blank_line after
            // the previous block, so we only owe (delta - 2) additional blanks.
            if !in_code_block {
                let delta = src_line - current_source_line;
                for _ in 0..delta.saturating_sub(2) {
                    lines.push(Line::from(""));
                }
            }
            let rendered = lines.len() as u16;
            for entry in &mut source_to_rendered[(current_source_line + 1)..=src_line.min(source_line_count)] {
                *entry = rendered;
            }
            current_source_line = src_line;
        }

        match event {
            Event::Start(ref tag) => {
                if matches!(tag, Tag::List(_)) && !spans.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut spans)));
                }
                if matches!(tag, Tag::BlockQuote(_)) {
                    in_blockquote = true;
                }
                if matches!(tag, Tag::CodeBlock(_)) {
                    in_code_block = true;
                }
                let style = if in_blockquote && matches!(tag, Tag::Paragraph) {
                    *style_stack.last().unwrap_or(&Style::default())
                } else {
                    style_for_tag(tag, &mut spans, &mut list_depth)
                };
                style_stack.push(style);
            }
            Event::End(tag_end) => {
                style_stack.pop();
                if matches!(tag_end, TagEnd::BlockQuote(_)) {
                    in_blockquote = false;
                }
                if matches!(tag_end, TagEnd::CodeBlock) {
                    in_code_block = false;
                }
                handle_tag_end(tag_end, &mut spans, &mut lines, &mut list_depth);
            }
            Event::Text(text) => {
                let style = *style_stack.last().unwrap_or(&Style::default());
                if in_code_block {
                    let parts: Vec<&str> = text.split('\n').collect();
                    for (i, part) in parts.iter().enumerate() {
                        if !part.is_empty() {
                            spans.push(Span::styled(part.to_string(), style));
                        }
                        if i < parts.len() - 1 {
                            lines.push(Line::from(std::mem::take(&mut spans)));
                        }
                    }
                } else {
                    spans.push(Span::styled(text.to_string(), style));
                }
            }
            Event::Code(code) => {
                spans.push(Span::styled(
                    format!("`{}`", code),
                    Style::default().fg(Color::Red),
                ));
            }
            Event::SoftBreak | Event::HardBreak => {
                lines.push(Line::from(std::mem::take(&mut spans)));
                if in_blockquote {
                    spans.push(Span::styled(
                        "  > ",
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }
            Event::Rule => {
                if !spans.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut spans)));
                }
                lines.push(Line::from(Span::styled(
                    "────────────────────────────────",
                    Style::default().fg(Color::DarkGray),
                )));
                push_blank_line(&mut lines);
            }
            _ => {}
        }
    }

    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }

    // Fill remaining source lines
    let rendered = lines.len() as u16;
    for entry in &mut source_to_rendered[(current_source_line + 1)..=source_line_count] {
        *entry = rendered;
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "Start typing...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    if frontmatter_line_count > 0 {
        // Re-index offsets by original content lines (including the stripped
        // frontmatter block) so callers passing vim's line('w0') — which
        // counts from the top of the file including frontmatter — land on
        // the correct rendered line.
        let mut reindexed = vec![0u16; original_line_count + 1];
        for (i, v) in source_to_rendered.iter().enumerate() {
            let idx = i + frontmatter_line_count;
            if idx < reindexed.len() {
                reindexed[idx] = *v;
            }
        }
        source_to_rendered = reindexed;
    }

    (lines, source_to_rendered)
}


fn style_for_tag(tag: &Tag<'_>, spans: &mut Vec<Span<'static>>, list_depth: &mut usize) -> Style {
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

fn push_blank_line(lines: &mut Vec<Line<'_>>) {
    if lines.last().is_none_or(|l| !l.spans.is_empty()) {
        lines.push(Line::from(""));
    }
}

fn handle_tag_end(tag_end: TagEnd, spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>, list_depth: &mut usize) {
    match tag_end {
        TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::BlockQuote(_) => {
            lines.push(Line::from(std::mem::take(spans)));
            push_blank_line(lines);
        }
        TagEnd::List(_) => {
            *list_depth = list_depth.saturating_sub(1);
            if *list_depth == 0 {
                push_blank_line(lines);
            }
        }
        TagEnd::Item => {
            if !spans.is_empty() {
                lines.push(Line::from(std::mem::take(spans)));
            }
        }
        TagEnd::CodeBlock => {
            lines.push(Line::from(std::mem::take(spans)));
            push_blank_line(lines);
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

    // Frontmatter

    #[test]
    fn strip_frontmatter_removes_leading_yaml_block() {
        let content = "---\ntitle: Hello\nsubtitle: World\n---\nBody text";
        assert_eq!(strip_frontmatter(content), "Body text");
    }

    #[test]
    fn strip_frontmatter_passthrough_when_no_block() {
        let content = "Just a plain body\nwith no frontmatter";
        assert_eq!(strip_frontmatter(content), content);
    }

    #[test]
    fn frontmatter_title_rendered_as_header_at_top() {
        let md = "---\ntitle: Hello World\n---\n\nBody.";
        let (lines, _) = render_with_offsets(md, None);
        assert_eq!(line_text(&lines[0]), "Hello World");
        assert_eq!(line_text(&lines[1]), "");
    }

    #[test]
    fn offset_map_indexed_by_original_content_lines_with_frontmatter() {
        // Original content: 5 lines
        //   line 0: ---
        //   line 1: title: Hello
        //   line 2: ---
        //   line 3: (blank)
        //   line 4: Body paragraph
        // vim passes original-content line numbers via titlestring's line('w0').
        // The offset map must therefore be indexed by original lines, not
        // post-strip body lines.
        let md = "---\ntitle: Hello\n---\n\nBody paragraph";
        let (_, offsets) = render_with_offsets(md, None);
        assert!(offsets.len() >= 6, "offset map too short: {}", offsets.len());
        assert_eq!(offsets[1], 0, "frontmatter line maps to header");
        assert!(offsets[4] >= 2, "body line must skip header: {}", offsets[4]);
    }

    #[test]
    fn header_author_from_param_rendered_above_title() {
        let md = "---\ntitle: Hello\n---\n\nBody.";
        let (lines, _) = render_with_offsets(md, Some("Leandro"));
        assert_eq!(line_text(&lines[0]), "Leandro");
        assert_eq!(line_text(&lines[1]), "Hello");
        assert_eq!(line_text(&lines[2]), "");
    }

    #[test]
    fn header_author_from_frontmatter_overrides_param() {
        let md = "---\ntitle: Hello\nauthor: Jane\n---\n\nBody.";
        let (lines, _) = render_with_offsets(md, Some("Bob"));
        assert_eq!(line_text(&lines[0]), "Jane");
        assert_eq!(line_text(&lines[1]), "Hello");
    }

    #[test]
    fn header_date_falls_back_to_date_field() {
        let md = "---\ntitle: Hello\ndate: 2024-03-05\n---\n\nBody.";
        let (lines, _) = render_with_offsets(md, None);
        assert_eq!(line_text(&lines[0]), "Hello");
        assert_eq!(line_text(&lines[1]), "2024-03-05");
    }

    #[test]
    fn header_language_appended_to_date_line() {
        let md = "---\ntitle: Hello\npublished_at: 2024-01-02\nlanguage: en\n---\n\nBody.";
        let (lines, _) = render_with_offsets(md, None);
        assert_eq!(line_text(&lines[1]), "2024-01-02 · EN");
    }

    #[test]
    fn header_language_alone_renders_as_its_own_line() {
        let md = "---\ntitle: Hello\nlanguage: pt\n---\n\nBody.";
        let (lines, _) = render_with_offsets(md, None);
        assert_eq!(line_text(&lines[0]), "Hello");
        assert_eq!(line_text(&lines[1]), "PT");
    }

    #[test]
    fn header_full_order_author_title_date() {
        let md = "---\ntitle: Hello\npublished_at: \"2022-07-12\"\n---\n\nBody.";
        let (lines, _) = render_with_offsets(md, Some("Leandro"));
        assert_eq!(line_text(&lines[0]), "Leandro");
        assert_eq!(line_text(&lines[1]), "Hello");
        assert_eq!(line_text(&lines[2]), "2022-07-12");
        assert_eq!(line_text(&lines[3]), "");
    }

    #[test]
    fn frontmatter_published_at_rendered_as_date_under_title() {
        let md = "---\ntitle: Hello\npublished_at: \"2022-07-12 06:11:29Z\"\n---\n\nBody.";
        let (lines, _) = render_with_offsets(md, None);
        assert_eq!(line_text(&lines[0]), "Hello");
        assert_eq!(line_text(&lines[1]), "2022-07-12");
        assert_eq!(line_text(&lines[2]), "");
    }

    #[test]
    fn frontmatter_title_quoted_value_is_unquoted() {
        let md = "---\ntitle: \"Hello World\"\n---\n\nBody.";
        let (lines, _) = render_with_offsets(md, None);
        assert_eq!(line_text(&lines[0]), "Hello World");
    }

    #[test]
    fn no_title_header_when_no_frontmatter() {
        let md = "Just a plain body.";
        let (lines, _) = render_with_offsets(md, None);
        assert_eq!(line_text(&lines[0]), "Just a plain body.");
    }

    #[test]
    fn frontmatter_block_is_not_rendered_as_body() {
        let md = "---\ntitle: Hello\nsubtitle: World\n---\n\nReal body paragraph.";
        let (lines, _) = render_with_offsets(md, None);
        let combined: String = all_text(&lines).join("\n");
        assert!(!combined.contains("title: Hello"), "frontmatter leaked: {}", combined);
        assert!(!combined.contains("subtitle: World"), "subtitle leaked: {}", combined);
        assert!(combined.contains("Real body paragraph"));
    }

    #[test]
    fn strip_frontmatter_ignores_mid_document_rules() {
        // A `---` used as a markdown horizontal rule further down the file
        // must not be treated as the end of a frontmatter block.
        let content = "Body paragraph\n\n---\n\nAfter rule";
        assert_eq!(strip_frontmatter(content), content);
    }

    // Empty / minimal

    #[test]
    fn empty_content_shows_placeholder() {
        let (lines, _) = render_with_offsets("", None);
        assert_eq!(line_text(&lines[0]), "Start typing...");
    }

    #[test]
    fn plain_text_renders_as_paragraph() {
        let (lines, _) = render_with_offsets("Hello world", None);
        assert_eq!(line_text(&lines[0]), "Hello world");
    }

    // Headings

    #[test]
    fn heading_h1() {
        let (lines, _) = render_with_offsets("# Title", None);
        let text = line_text(&lines[0]);
        assert!(text.contains("# "), "H1 prefix");
        assert!(text.contains("Title"));
        assert_eq!(line_text(&lines[1]), "", "blank line after heading");
    }

    #[test]
    fn heading_h2() {
        let (lines, _) = render_with_offsets("## Subtitle", None);
        assert!(line_text(&lines[0]).contains("## "));
    }

    #[test]
    fn heading_h3() {
        let (lines, _) = render_with_offsets("### Section", None);
        assert!(line_text(&lines[0]).contains("### "));
    }

    #[test]
    fn heading_h4() {
        let (lines, _) = render_with_offsets("#### Deep", None);
        assert!(line_text(&lines[0]).contains("#### "));
    }

    // Inline formatting

    #[test]
    fn bold_text() {
        let (lines, _) = render_with_offsets("Some **bold** here", None);
        let text = line_text(&lines[0]);
        assert!(text.contains("bold"));
    }

    #[test]
    fn italic_text() {
        let (lines, _) = render_with_offsets("Some *italic* here", None);
        let text = line_text(&lines[0]);
        assert!(text.contains("italic"));
    }

    #[test]
    fn strikethrough_text() {
        let (lines, _) = render_with_offsets("Some ~~deleted~~ here", None);
        let text = line_text(&lines[0]);
        assert!(text.contains("deleted"));
    }

    #[test]
    fn inline_code() {
        let (lines, _) = render_with_offsets("Use `mix test` to run", None);
        let text = line_text(&lines[0]);
        assert!(text.contains("`mix test`"));
    }

    #[test]
    fn mixed_formatting() {
        let (lines, _) = render_with_offsets("**bold** and *italic* and `code`", None);
        let text = line_text(&lines[0]);
        assert!(text.contains("bold"));
        assert!(text.contains("italic"));
        assert!(text.contains("`code`"));
    }

    // Lists

    #[test]
    fn simple_list() {
        let (lines, _) = render_with_offsets("- One\n- Two\n- Three", None);
        let texts = all_text(&lines);
        assert!(texts[0].contains("- ") && texts[0].contains("One"));
        assert!(texts[1].contains("- ") && texts[1].contains("Two"));
        assert!(texts[2].contains("- ") && texts[2].contains("Three"));
    }

    #[test]
    fn nested_list_two_levels() {
        let (lines, _) = render_with_offsets("- Parent\n  - Child A\n  - Child B", None);
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
        let (lines, _) = render_with_offsets(md, None);
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
        let (lines, _) = render_with_offsets(md, None);
        let texts = all_text(&lines);
        // No line should contain two item markers
        for text in &texts {
            let dash_count = text.matches("- ").count();
            assert!(dash_count <= 1, "Line has multiple items: |{}|", text);
        }
    }

    #[test]
    fn nested_list_no_consecutive_blanks() {
        let md = "- Parent\n  - Child A\n  - Child B\n- Another\n  - Sub";
        let (lines, _) = render_with_offsets(md, None);
        let texts = all_text(&lines);
        for i in 1..texts.len() {
            if texts[i].is_empty() && texts[i - 1].is_empty() {
                panic!("consecutive blank lines at index {}", i);
            }
        }
    }

    // Blockquotes

    #[test]
    fn blockquote_single_line() {
        let (lines, _) = render_with_offsets("> Hello", None);
        let text = line_text(&lines[0]);
        assert!(text.contains(">"), "quote marker");
        assert!(text.contains("Hello"));
    }

    #[test]
    fn blockquote_multiline() {
        let (lines, _) = render_with_offsets("> Line one\n> Line two", None);
        let texts = all_text(&lines);
        assert!(texts[0].contains(">") && texts[0].contains("Line one"));
        assert!(texts[1].contains(">") && texts[1].contains("Line two"));
        // Text spans should carry blockquote style (DarkGray + Italic)
        let expected_style = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC);
        let text_span = lines[0].spans.iter().find(|s| s.content.contains("Line one"));
        assert_eq!(text_span.unwrap().style, expected_style, "blockquote text style");
    }

    // Code blocks

    #[test]
    fn fenced_code_block() {
        let md = "```\nlet x = 1;\nlet y = 2;\n```";
        let (lines, _) = render_with_offsets(md, None);
        let texts = all_text(&lines);
        // Each code line must be on its own rendered line
        assert!(texts.iter().any(|t| t == "let x = 1;"), "x on own line");
        assert!(texts.iter().any(|t| t == "let y = 2;"), "y on own line");
    }

    #[test]
    fn code_block_with_language() {
        let md = "```rust\nfn main() {}\n```";
        let (lines, _) = render_with_offsets(md, None);
        let combined: String = all_text(&lines).join("\n");
        assert!(combined.contains("fn main()"));
    }

    // Horizontal rule

    #[test]
    fn horizontal_rule() {
        let md = "Above\n\n---\n\nBelow";
        let (lines, _) = render_with_offsets(md, None);
        let combined: String = all_text(&lines).join("\n");
        assert!(combined.contains("────"), "rule line");
        assert!(combined.contains("Above"));
        assert!(combined.contains("Below"));
    }

    // Links

    #[test]
    fn inline_link() {
        let md = "Visit [my site](https://example.com) today";
        let (lines, _) = render_with_offsets(md, None);
        let text = line_text(&lines[0]);
        assert!(text.contains("["), "link opening bracket");
        assert!(text.contains("my site"));
        assert!(text.contains("]"), "link closing bracket");
    }

    // Paragraphs

    #[test]
    fn preserves_multiple_blank_lines_between_paragraphs() {
        // User types 3 blank lines between paragraphs. Preview must mirror
        // the source's vertical rhythm so vim and preview stay aligned while
        // scrolling, even though markdown semantically collapses whitespace.
        let md = "First\n\n\n\nSecond";
        let (lines, _) = render_with_offsets(md, None);
        let texts = all_text(&lines);
        assert!(texts[0].contains("First"));
        assert_eq!(texts[1], "");
        assert_eq!(texts[2], "");
        assert_eq!(texts[3], "");
        assert!(texts[4].contains("Second"));
    }

    #[test]
    fn paragraphs_separated_by_blank_line() {
        let md = "First paragraph.\n\nSecond paragraph.";
        let (lines, _) = render_with_offsets(md, None);
        let texts = all_text(&lines);
        assert!(texts[0].contains("First"));
        assert_eq!(texts[1], "", "blank separator");
        assert!(texts[2].contains("Second"));
    }

    // Source-to-rendered offset mapping

    #[test]
    fn offset_mapping_plain_lines() {
        let md = "line one\nline two\nline three";
        let (_, offsets) = render_with_offsets(md, None);
        // Source line 0 -> rendered line 0
        assert_eq!(offsets[0], 0);
    }

    #[test]
    fn offset_mapping_with_heading() {
        let md = "# Title\n\nParagraph here";
        let (lines, offsets) = render_with_offsets(md, None);
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
        let (lines, offsets) = render_with_offsets(&md, None);
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

    #[test]
    fn subtitle_rendered_below_title() {
        let md = "---\ntitle: Hello\nsubtitle: A short tagline\n---\n\nBody.";
        let (lines, _) = render_with_offsets(md, None);
        assert_eq!(line_text(&lines[0]), "Hello");
        assert_eq!(line_text(&lines[1]), "A short tagline");
        assert_eq!(line_text(&lines[2]), "");
    }

    #[test]
    fn no_subtitle_no_extra_line() {
        let md = "---\ntitle: Hello\n---\n\nBody.";
        let (lines, _) = render_with_offsets(md, None);
        assert_eq!(line_text(&lines[0]), "Hello");
        assert_eq!(line_text(&lines[1]), "");
        assert_eq!(line_text(&lines[2]), "Body.");
    }

    // --- set_frontmatter_field ---

    #[test]
    fn set_frontmatter_field_replaces_existing() {
        let content = "---\ntitle: Old Title\npublished_at: 2026-01-01\n---\n\nBody.";
        let result = set_frontmatter_field(content, "title", "New Title");
        assert!(result.contains("title: New Title"));
        assert!(!result.contains("Old Title"));
        assert!(result.contains("published_at: 2026-01-01"));
        assert!(result.contains("Body."));
    }

    #[test]
    fn set_frontmatter_field_inserts_when_missing() {
        let content = "---\ntitle: Hello\n---\n\nBody.";
        let result = set_frontmatter_field(content, "subtitle", "A tagline");
        assert!(result.contains("subtitle: A tagline"));
        assert!(result.contains("title: Hello"));
        assert!(result.contains("Body."));
    }

    #[test]
    fn set_frontmatter_field_no_frontmatter_returns_unchanged() {
        let content = "Just plain text.";
        let result = set_frontmatter_field(content, "title", "New");
        assert_eq!(result, content);
    }
}
