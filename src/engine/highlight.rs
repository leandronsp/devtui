use std::sync::OnceLock;
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Highlight a code block. Returns HTML spans with TextMate scope classes
/// (e.g. `keyword`, `string`, `comment`). Falls back to plain HTML-escaped
/// text when the language is unknown or syntect fails on the input.
pub fn highlight(code: &str, lang: &str) -> String {
    let ss = syntax_set();
    let syntax = lookup_syntax(ss, lang);
    let mut generator = ClassedHTMLGenerator::new_with_class_style(syntax, ss, ClassStyle::Spaced);
    for line in LinesWithEndings::from(code) {
        if generator
            .parse_html_for_line_which_includes_newline(line)
            .is_err()
        {
            return escape_html(code);
        }
    }
    generator.finalize()
}

fn lookup_syntax<'a>(
    ss: &'a SyntaxSet,
    lang: &str,
) -> &'a syntect::parsing::SyntaxReference {
    if lang.is_empty() {
        return ss.find_syntax_plain_text();
    }
    let token = lang.split_whitespace().next().unwrap_or(lang);
    ss.find_syntax_by_token(token)
        .or_else(|| ss.find_syntax_by_extension(token))
        .or_else(|| ss.find_syntax_by_name(token))
        .unwrap_or_else(|| ss.find_syntax_plain_text())
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_rust_emits_keyword_span() {
        let html = highlight("fn main() {}\n", "rust");
        assert!(html.contains("class=\"storage type function rust\""));
    }

    #[test]
    fn highlight_bash_emits_string_span() {
        let html = highlight("echo \"hello\"\n", "bash");
        assert!(html.contains("string quoted double shell"));
    }

    #[test]
    fn highlight_unknown_language_falls_back_to_plain_text() {
        let html = highlight("some text\n", "this-lang-does-not-exist");
        assert!(html.contains("some text"));
    }

    #[test]
    fn highlight_empty_lang_uses_plain_text() {
        let html = highlight("anything\n", "");
        assert!(html.contains("anything"));
    }

    #[test]
    fn highlight_escapes_html_special_chars_in_plain_fallback() {
        let html = highlight("a < b && c > d\n", "totally-unknown");
        assert!(html.contains("&lt;"));
        assert!(html.contains("&amp;"));
        assert!(html.contains("&gt;"));
    }
}
