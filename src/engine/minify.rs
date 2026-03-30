/// Minify CSS: strip comments, collapse whitespace, remove unnecessary chars.
pub fn minify_css(css: &str) -> String {
    let mut result = String::with_capacity(css.len());
    let chars: Vec<char> = css.chars().collect();
    let len = chars.len();
    let mut idx = 0;

    // Strip comments
    while idx < len {
        if idx + 1 < len && chars[idx] == '/' && chars[idx + 1] == '*' {
            idx += 2;
            while idx + 1 < len && !(chars[idx] == '*' && chars[idx + 1] == '/') {
                idx += 1;
            }
            idx += 2; // skip */
        } else {
            result.push(chars[idx]);
            idx += 1;
        }
    }

    // Collapse whitespace to single space
    let mut collapsed = String::with_capacity(result.len());
    let mut prev_ws = false;
    for ch in result.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                collapsed.push(' ');
            }
            prev_ws = true;
        } else {
            collapsed.push(ch);
            prev_ws = false;
        }
    }

    // Remove whitespace around operators
    let operators = ['{', '}', ':', ';', ',', '>', '~', '+'];
    let mut minified = collapsed;
    for op in operators {
        minified = minified.replace(&format!(" {op}"), &format!("{op}"));
        minified = minified.replace(&format!("{op} "), &format!("{op}"));
    }

    // Remove trailing semicolons before }
    minified = minified.replace(";}", "}");
    minified.trim().to_string()
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
    let mut result = html.to_string();

    // Extract and preserve <pre>, <script>, <style> blocks
    let preserve_tags = ["pre", "script", "style"];
    let mut preserved: Vec<String> = Vec::new();

    for tag in &preserve_tags {
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        loop {
            let Some(start) = result.find(&open) else {
                break;
            };
            let Some(end_offset) = result[start..].find(&close) else {
                break;
            };
            let end = start + end_offset + close.len();
            let block = result[start..end].to_string();
            let placeholder = format!("__PRESERVE_{}__", preserved.len());
            preserved.push(block);
            result.replace_range(start..end, &placeholder);
        }
    }

    // Strip HTML comments
    loop {
        let Some(start) = result.find("<!--") else {
            break;
        };
        let Some(end_offset) = result[start..].find("-->") else {
            break;
        };
        result.replace_range(start..start + end_offset + 3, "");
    }

    // Collapse whitespace between tags: >   < becomes ><
    let mut collapsed = String::with_capacity(result.len());
    let mut chars = result.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '>' {
            collapsed.push(ch);
            // Skip whitespace until next <
            let mut ws = String::new();
            while let Some(&next) = chars.peek() {
                if next.is_whitespace() {
                    ws.push(next);
                    chars.next();
                } else {
                    break;
                }
            }
            if chars.peek() != Some(&'<') && !ws.is_empty() {
                // Not followed by <, keep one space
                collapsed.push(' ');
            }
        } else {
            collapsed.push(ch);
        }
    }

    // Collapse runs of whitespace to single space
    let mut final_result = String::with_capacity(collapsed.len());
    let mut prev_ws = false;
    for ch in collapsed.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                final_result.push(' ');
            }
            prev_ws = true;
        } else {
            final_result.push(ch);
            prev_ws = false;
        }
    }

    // Restore preserved blocks
    for (idx, block) in preserved.iter().enumerate() {
        let placeholder = format!("__PRESERVE_{idx}__");
        final_result = final_result.replace(&placeholder, block);
    }

    final_result
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
}
