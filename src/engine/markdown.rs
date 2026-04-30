use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Convert markdown to HTML using pulldown-cmark.
/// Pre-processes emoji shortcodes before parsing.
pub fn markdown_to_html(markdown: &str) -> String {
    let preprocessed = replace_emoji_shortcodes(markdown);
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(&preprocessed, options);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);
    html
}

/// Extract a plain-text snippet from markdown, truncated at word boundary.
/// Uses explicit description if provided, otherwise extracts text from markdown body.
pub fn post_snippet(body: &str, description: Option<&str>, limit: usize) -> String {
    if let Some(desc) = description {
        if !desc.is_empty() {
            return desc.to_string();
        }
    }
    let text = extract_plain_text(body);
    truncate_at_word_boundary(&text, limit)
}

/// Parse markdown and extract only visible text content, skipping code blocks
/// and image alt text. Image alt is metadata, not body content; including it
/// pollutes the auto-generated description for image-heavy posts.
fn extract_plain_text(markdown: &str) -> String {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = Parser::new_ext(markdown, options);
    let mut text = String::new();
    let mut in_code_block = false;
    let mut in_image = false;

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
            Event::End(TagEnd::CodeBlock) => in_code_block = false,
            Event::Start(Tag::Image { .. }) => in_image = true,
            Event::End(TagEnd::Image) => in_image = false,
            Event::Text(t) if !in_code_block && !in_image => {
                if !text.is_empty() { text.push(' '); }
                text.push_str(&t);
            }
            Event::SoftBreak | Event::HardBreak if !in_code_block && !in_image => text.push(' '),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_at_word_boundary(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }

    // Find the last space before or at the limit, respecting UTF-8
    let mut end = limit;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }

    // Try to break at a word boundary
    if let Some(last_space) = text[..end].rfind(' ') {
        text[..last_space].to_string()
    } else {
        text[..end].to_string()
    }
}

/// Replace :emoji: shortcodes with <span class="emoji" data-emoji="name">CHAR</span>.
/// Skips shortcodes inside code fences and inline code spans.
fn replace_emoji_shortcodes(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_code_fence = false;

    for line in text.lines() {
        if line.starts_with("```") {
            in_code_fence = !in_code_fence;
        }
        if in_code_fence || line.starts_with("```") {
            result.push_str(line);
        } else {
            replace_emoji_in_line(line, &mut result);
        }
        result.push('\n');
    }

    if !text.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }
    result
}

fn replace_emoji_in_line(line: &str, result: &mut String) {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut idx = 0;
    let mut in_inline_code = false;

    while idx < len {
        if chars[idx] == '`' {
            in_inline_code = !in_inline_code;
            result.push('`');
            idx += 1;
            continue;
        }
        if in_inline_code || chars[idx] != ':' {
            result.push(chars[idx]);
            idx += 1;
            continue;
        }
        if let Some(end) = find_shortcode_end(&chars, idx) {
            let name: String = chars[idx + 1..end].iter().collect();
            if let Some(emoji) = gh_emoji::get(&name) {
                result.push_str(&format!(
                    r#"<span class="emoji" data-emoji="{name}">{emoji}</span>"#
                ));
                idx = end + 1;
                continue;
            }
        }
        result.push(chars[idx]);
        idx += 1;
    }
}

fn find_shortcode_end(chars: &[char], start: usize) -> Option<usize> {
    let mut idx = start + 1;
    while idx < chars.len() {
        if chars[idx] == ':' {
            return Some(idx);
        }
        if !chars[idx].is_alphanumeric() && chars[idx] != '_' && chars[idx] != '-' && chars[idx] != '+' {
            return None;
        }
        idx += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- markdown_to_html ---

    #[test]
    fn markdown_to_html_renders_heading() {
        let result = markdown_to_html("## Section Title\n");
        assert!(result.contains("<h2>Section Title</h2>"));
    }

    #[test]
    fn markdown_to_html_renders_bold() {
        let result = markdown_to_html("Some **bold** text\n");
        assert!(result.contains("<strong>bold</strong>"));
    }

    #[test]
    fn markdown_to_html_renders_code_block() {
        let result = markdown_to_html("```ruby\nputs 'hi'\n```\n");
        assert!(result.contains("<code"));
        assert!(result.contains("puts 'hi'"));
    }

    #[test]
    fn markdown_to_html_renders_hr_before_heading_as_h2() {
        let md = "Some text\n\n---\n\n## Section After Rule\n";
        let result = markdown_to_html(md);
        assert!(result.contains("<h2>Section After Rule</h2>"));
    }

    #[test]
    fn markdown_to_html_hr_not_in_table() {
        let md = "Some text\n\n---\n\n## Section After Rule\n";
        let result = markdown_to_html(md);
        assert!(!result.contains("<th"));
    }

    #[test]
    fn markdown_to_html_converts_emoji_shortcodes() {
        let result = markdown_to_html("Hello :wave: world :bulb:\n");
        assert!(result.contains(r#"data-emoji="wave""#));
        assert!(result.contains(r#"data-emoji="bulb""#));
    }

    #[test]
    fn markdown_to_html_preserves_emoji_in_code_blocks() {
        let result = markdown_to_html("```\n:wave:\n```\n");
        // Inside code blocks, emoji shortcodes should NOT be converted
        assert!(!result.contains("data-emoji"));
    }

    #[test]
    fn markdown_to_html_renders_lists_without_blank_line() {
        // post_body already inserts blank lines, but test the HTML output
        let md = "Some text\n\n* item one\n* item two\n";
        let result = markdown_to_html(md);
        assert!(result.contains("<li>"));
    }

    #[test]
    fn markdown_to_html_renders_blockquote() {
        let md = "Some text\n\n> a quote\n";
        let result = markdown_to_html(md);
        assert!(result.contains("<blockquote>"));
    }

    #[test]
    fn markdown_to_html_renders_links() {
        let md = "Visit [my site](https://example.com)\n";
        let result = markdown_to_html(md);
        assert!(result.contains(r#"<a href="https://example.com">my site</a>"#));
    }

    // --- post_snippet ---

    #[test]
    fn post_snippet_uses_description_when_available() {
        let result = post_snippet("body text", Some("A description"), 300);
        assert_eq!(result, "A description");
    }

    #[test]
    fn post_snippet_falls_back_to_body() {
        let result = post_snippet("Content without description.", None, 300);
        assert!(result.contains("Content without description"));
    }

    #[test]
    fn post_snippet_strips_bold_italic_code() {
        let result = post_snippet("Some **bold** and *italic* and `code` text", None, 300);
        assert!(result.contains("bold"));
        assert!(!result.contains("**"));
        assert!(!result.contains("`"));
    }

    #[test]
    fn post_snippet_strips_links() {
        let result = post_snippet("Visit [my site](https://example.com) now", None, 300);
        assert!(result.contains("my site"));
        assert!(!result.contains("https://example.com"));
    }

    #[test]
    fn post_snippet_strips_horizontal_rules() {
        let result = post_snippet("Before\n\n---\n\nAfter", None, 300);
        assert!(!result.contains("---"));
    }

    #[test]
    fn post_snippet_strips_strikethrough() {
        let result = post_snippet("~~shame~~ courage", None, 300);
        assert!(result.contains("courage"));
        assert!(!result.contains("~~"));
    }

    #[test]
    fn post_snippet_custom_limit_truncates() {
        let body = "This is a long body text that should be truncated at the word boundary when the limit is reached.";
        let result = post_snippet(body, None, 50);
        assert!(result.len() <= 50);
    }

    #[test]
    fn post_snippet_preserves_utf8() {
        let result = post_snippet("Sentado no sofá, então lá vai", None, 300);
        assert!(result.contains("sofá"));
        assert!(result.contains("então"));
        assert!(result.contains("lá"));
    }

    // --- markdown_to_html edge cases ---

    #[test]
    fn markdown_to_html_renders_empty_input() {
        let result = markdown_to_html("");
        assert!(result.is_empty());
    }

    #[test]
    fn markdown_to_html_renders_table() {
        let md = "| col1 | col2 |\n|------|------|\n| a    | b    |\n";
        let result = markdown_to_html(md);
        assert!(result.contains("<table>"));
        assert!(result.contains("<td>a</td>"));
    }

    #[test]
    fn markdown_to_html_renders_italic() {
        let result = markdown_to_html("Some *italic* text\n");
        assert!(result.contains("<em>italic</em>"));
    }

    #[test]
    fn markdown_to_html_renders_inline_code() {
        let result = markdown_to_html("Use `cargo build`\n");
        assert!(result.contains("<code>cargo build</code>"));
    }

    #[test]
    fn markdown_to_html_preserves_emoji_in_inline_code() {
        let result = markdown_to_html("Use `:wave:` shortcode\n");
        // Inside inline backticks, shortcodes should NOT be converted
        assert!(result.contains(":wave:"));
    }

    #[test]
    fn markdown_to_html_renders_nested_emphasis() {
        let result = markdown_to_html("Some ***bold italic*** text\n");
        assert!(result.contains("<em><strong>bold italic</strong></em>"));
    }

    // --- post_snippet edge cases ---

    #[test]
    fn post_snippet_empty_description_falls_back_to_body() {
        let result = post_snippet("Body content here.", Some(""), 300);
        assert!(result.contains("Body content here"));
    }

    #[test]
    fn post_snippet_skips_code_blocks() {
        let body = "Intro text\n\n```rust\nfn main() {}\n```\n\nAfter code.";
        let result = post_snippet(body, None, 300);
        assert!(result.contains("Intro text"));
        assert!(result.contains("After code"));
        assert!(!result.contains("fn main"));
    }

    #[test]
    fn post_snippet_skips_image_alt_text() {
        // Image-only posts shouldn't leak the alt slug into the description.
        let body = "![my-screenshot-2026](/images/my-screenshot-2026.png)";
        let result = post_snippet(body, None, 300);
        assert!(!result.contains("my-screenshot"));
        assert!(!result.contains("2026"));
    }

    #[test]
    fn post_snippet_keeps_prose_around_images() {
        // Body text adjacent to an image should still survive.
        let body = "Intro before image.\n\n![alt-tag](/images/x.png)\n\nProse after.";
        let result = post_snippet(body, None, 300);
        assert!(result.contains("Intro before image"));
        assert!(result.contains("Prose after"));
        assert!(!result.contains("alt-tag"));
    }

    #[test]
    fn post_snippet_does_not_break_multibyte_at_boundary() {
        // UTF-8 boundary stress test
        let body = "á".repeat(100);
        let result = post_snippet(&body, None, 160);
        assert!(result.len() <= 160);
        // Verify it's valid UTF-8 (would panic if not)
        let _ = result.chars().count();
    }
}
