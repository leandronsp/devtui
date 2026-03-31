use std::fs;
use std::path::{Path, PathBuf};

/// Concatenate theme CSS modules (base, index, article, syntax, responsive).
/// Uses blog overrides if present, otherwise falls back to theme directory.
pub fn compile_css(blog_dir: &Path, theme_dir: &Path) -> Result<String, String> {
    let style_dir = if blog_dir.join("base.css").exists() {
        blog_dir.to_path_buf()
    } else {
        theme_dir.to_path_buf()
    };
    let mut css = String::new();
    for part in &["base", "index", "article", "syntax", "responsive"] {
        let css_file = style_dir.join(format!("{part}.css"));
        if css_file.exists() {
            css.push_str(&fs::read_to_string(&css_file).map_err(|e| e.to_string())?);
        }
    }
    Ok(css)
}

/// Minify CSS, inline into HTML files, minify HTML, then remove the standalone CSS file.
pub fn minify_and_inline(dist_dir: &Path, css: &str, rebuilt_articles: &[String]) -> Result<(), String> {
    let minified_css = minify_css(css);
    let mut process_files = vec![dist_dir.join("index.html")];
    for path in rebuilt_articles {
        process_files.push(PathBuf::from(path));
    }
    process_files.push(dist_dir.join("404.html"));

    for html_path in &process_files {
        if html_path.exists() {
            let content = fs::read_to_string(html_path).map_err(|e| e.to_string())?;
            let content = inline_css(&content, &minified_css);
            let content = minify_html(&content);
            fs::write(html_path, content).map_err(|e| e.to_string())?;
        }
    }

    let _ = fs::remove_file(dist_dir.join("style.css"));
    Ok(())
}

/// Minify CSS: strip comments, collapse whitespace, remove unnecessary chars.
pub fn minify_css(css: &str) -> String {
    let no_comments = strip_css_comments(css);
    let collapsed = collapse_whitespace_runs(&no_comments);
    strip_css_whitespace_around_operators(&collapsed)
}

fn strip_css_comments(css: &str) -> String {
    let mut result = String::with_capacity(css.len());
    let chars: Vec<char> = css.chars().collect();
    let len = chars.len();
    let mut idx = 0;
    while idx < len {
        if idx + 1 < len && chars[idx] == '/' && chars[idx + 1] == '*' {
            idx += 2;
            while idx + 1 < len && !(chars[idx] == '*' && chars[idx + 1] == '/') {
                idx += 1;
            }
            idx += 2;
        } else {
            result.push(chars[idx]);
            idx += 1;
        }
    }
    result
}

fn strip_css_whitespace_around_operators(css: &str) -> String {
    let operators = ['{', '}', ':', ';', ',', '>', '~', '+'];
    let mut result = css.to_string();
    for op in operators {
        result = result.replace(&format!(" {op}"), &format!("{op}"));
        result = result.replace(&format!("{op} "), &format!("{op}"));
    }
    result = result.replace(";}", "}");
    result.trim().to_string()
}

/// Replace <link rel="stylesheet"> with inline <style> block.
pub fn inline_css(html: &str, css: &str) -> String {
    let mut result = html.to_string();
    let mut search_from = 0;
    loop {
        let Some(start) = result[search_from..].find("<link") else {
            break;
        };
        let start = search_from + start;
        let Some(end_offset) = result[start..].find('>') else {
            break;
        };
        let tag = &result[start..start + end_offset + 1];
        if tag.contains("stylesheet") {
            let replacement = format!("<style>{css}</style>");
            result.replace_range(start..start + end_offset + 1, &replacement);
            break; // Only one stylesheet link expected
        }
        search_from = start + end_offset + 1;
    }
    result
}

/// Minify HTML: strip comments, collapse whitespace. Preserves <pre>, <script>, <style> content.
pub fn minify_html(html: &str) -> String {
    let (mut result, preserved) = extract_preserved_blocks(html);
    strip_html_comments(&mut result);
    let result = collapse_inter_tag_whitespace(&result);
    let result = collapse_whitespace_runs(&result);
    restore_preserved_blocks(&result, &preserved)
}

/// Extract <pre>, <script>, <style> blocks and replace with placeholders.
fn extract_preserved_blocks(html: &str) -> (String, Vec<String>) {
    let mut result = html.to_string();
    let mut preserved: Vec<String> = Vec::new();
    for tag in &["pre", "script", "style"] {
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        loop {
            let Some(start) = result.find(&open) else { break; };
            let Some(end_offset) = result[start..].find(&close) else { break; };
            let end = start + end_offset + close.len();
            let block = result[start..end].to_string();
            let placeholder = format!("__PRESERVE_{}__", preserved.len());
            preserved.push(block);
            result.replace_range(start..end, &placeholder);
        }
    }
    (result, preserved)
}

fn strip_html_comments(html: &mut String) {
    loop {
        let Some(start) = html.find("<!--") else { break; };
        let Some(end_offset) = html[start..].find("-->") else { break; };
        html.replace_range(start..start + end_offset + 3, "");
    }
}

/// Collapse whitespace between tags: `>   <` becomes `><`.
fn collapse_inter_tag_whitespace(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut chars = html.chars().peekable();
    while let Some(ch) = chars.next() {
        result.push(ch);
        if ch == '>' {
            let mut has_ws = false;
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                has_ws = true;
                chars.next();
            }
            if has_ws && chars.peek() != Some(&'<') {
                result.push(' ');
            }
        }
    }
    result
}

fn collapse_whitespace_runs(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut prev_ws = false;
    for ch in html.chars() {
        if ch.is_whitespace() {
            if !prev_ws { result.push(' '); }
            prev_ws = true;
        } else {
            result.push(ch);
            prev_ws = false;
        }
    }
    result
}

fn restore_preserved_blocks(html: &str, preserved: &[String]) -> String {
    let mut result = html.to_string();
    for (idx, block) in preserved.iter().enumerate() {
        let placeholder = format!("__PRESERVE_{idx}__");
        result = result.replace(&placeholder, block);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- minify_css ---

    #[test]
    fn minify_css_strips_comments() {
        let css = "/* comment */\nbody { color: red; }";
        let result = minify_css(css);
        assert!(!result.contains("comment"));
        assert!(result.contains("body{color:red}"));
    }

    #[test]
    fn minify_css_collapses_whitespace() {
        let css = "body {\n  color: red;\n  font-size: 16px;\n}";
        let result = minify_css(css);
        assert!(!result.contains('\n'));
        assert!(result.contains("body{color:red;font-size:16px}"));
    }

    // --- inline_css ---

    #[test]
    fn inline_css_replaces_link_tag_with_style() {
        let html = r#"<head><link rel="stylesheet" href="style.css"></head>"#;
        let css = "body{color:red}";
        let result = inline_css(html, css);
        assert!(result.contains("<style>body{color:red}</style>"));
        assert!(!result.contains("link"));
    }

    // --- minify_html ---

    #[test]
    fn minify_html_strips_comments() {
        let html = "<div><!-- comment --><p>text</p></div>";
        let result = minify_html(html);
        assert!(!result.contains("comment"));
        assert!(result.contains("<p>text</p>"));
    }

    #[test]
    fn minify_html_collapses_whitespace_between_tags() {
        let html = "<div>  \n  <p>text</p>  \n  </div>";
        let result = minify_html(html);
        assert!(result.contains("<div><p>text</p></div>"));
    }

    #[test]
    fn minify_html_preserves_pre_content() {
        let html = "<pre>  code\n  here  </pre><p>  text  </p>";
        let result = minify_html(html);
        assert!(result.contains("<pre>  code\n  here  </pre>"));
        assert!(result.contains("<p> text </p>"));
    }

    #[test]
    fn minify_html_preserves_script_content() {
        let html = "<script>  var x = 1;  </script><p>  text  </p>";
        let result = minify_html(html);
        assert!(result.contains("<script>  var x = 1;  </script>"));
    }

    #[test]
    fn minify_html_preserves_style_content() {
        let html = "<style>  body { color: red; }  </style><p>  text  </p>";
        let result = minify_html(html);
        assert!(result.contains("<style>  body { color: red; }  </style>"));
    }

    // --- minify_css edge cases ---

    #[test]
    fn minify_css_handles_empty_input() {
        assert_eq!(minify_css(""), "");
    }

    #[test]
    fn minify_css_handles_media_queries() {
        let css = "@media (max-width: 768px) {\n  body { font-size: 14px; }\n}";
        let result = minify_css(css);
        assert!(result.contains("@media"));
        assert!(result.contains("768px"));
    }

    #[test]
    fn minify_css_strips_multiline_comments() {
        let css = "/* first\n * comment\n */\nbody { color: red; }\n/* second */";
        let result = minify_css(css);
        assert!(!result.contains("comment"));
        assert!(!result.contains("second"));
        assert!(result.contains("body{color:red}"));
    }

    // --- inline_css edge cases ---

    #[test]
    fn inline_css_leaves_non_stylesheet_links() {
        let html = r#"<link rel="icon" href="favicon.ico"><link rel="stylesheet" href="style.css">"#;
        let result = inline_css(html, "body{}");
        assert!(result.contains(r#"<link rel="icon" href="favicon.ico">"#));
        assert!(result.contains("<style>body{}</style>"));
    }

    #[test]
    fn inline_css_no_stylesheet_link_returns_unchanged() {
        let html = r#"<link rel="icon" href="favicon.ico">"#;
        let result = inline_css(html, "body{}");
        assert_eq!(result, html);
    }

    // --- minify_html edge cases ---

    #[test]
    fn minify_html_handles_empty_input() {
        assert_eq!(minify_html(""), "");
    }

    #[test]
    fn minify_html_handles_multiple_preserved_blocks() {
        let html = "<pre>code1</pre><p>  text  </p><pre>code2</pre>";
        let result = minify_html(html);
        assert!(result.contains("<pre>code1</pre>"));
        assert!(result.contains("<pre>code2</pre>"));
    }

    #[test]
    fn minify_html_strips_multiple_comments() {
        let html = "<!-- first --><p>text</p><!-- second --><p>more</p>";
        let result = minify_html(html);
        assert!(!result.contains("first"));
        assert!(!result.contains("second"));
        assert!(result.contains("<p>text</p>"));
        assert!(result.contains("<p>more</p>"));
    }

    // --- compile_css ---

    #[test]
    fn compile_css_concatenates_theme_files() {
        let tmp = tempdir();
        let theme = tmp.join("theme");
        let blog = tmp.join("blog");
        std::fs::create_dir_all(&theme).unwrap();
        std::fs::create_dir_all(&blog).unwrap();

        std::fs::write(theme.join("base.css"), "body { margin: 0; }\n").unwrap();
        std::fs::write(theme.join("article.css"), "article { padding: 1rem; }\n").unwrap();

        let css = compile_css(&blog, &theme).unwrap();
        assert!(css.contains("body { margin: 0; }"));
        assert!(css.contains("article { padding: 1rem; }"));
    }

    #[test]
    fn compile_css_uses_blog_override_when_present() {
        let tmp = tempdir();
        let theme = tmp.join("theme");
        let blog = tmp.join("blog");
        std::fs::create_dir_all(&theme).unwrap();
        std::fs::create_dir_all(&blog).unwrap();

        std::fs::write(theme.join("base.css"), "theme-base").unwrap();
        std::fs::write(blog.join("base.css"), "blog-base").unwrap();

        let css = compile_css(&blog, &theme).unwrap();
        assert!(css.contains("blog-base"));
        assert!(!css.contains("theme-base"));
    }

    #[test]
    fn compile_css_returns_empty_for_no_css_files() {
        let tmp = tempdir();
        let theme = tmp.join("theme");
        let blog = tmp.join("blog");
        std::fs::create_dir_all(&theme).unwrap();
        std::fs::create_dir_all(&blog).unwrap();

        let css = compile_css(&blog, &theme).unwrap();
        assert!(css.is_empty());
    }

    fn tempdir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "devtui-min-{}-{}",
            std::process::id(),
            id
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
