use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Link {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Guide {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlogConfig {
    pub title: String,
    pub subtitle: Option<String>,
    pub url: String,
    pub author: String,
    pub date_field: String,
    pub lang: String,
    pub articles_path: Option<String>,
    pub theme: Option<String>,
    pub analytics_id: Option<String>,
    pub license: Option<String>,
    pub license_url: Option<String>,
    pub og_image: Option<String>,
    pub tags: Option<Vec<String>>,
    pub links: Option<Vec<Link>>,
    pub guides: Option<Vec<Guide>>,
}

impl BlogConfig {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
        toml::from_str(&content).map_err(|e| format!("invalid TOML in {}: {}", path.display(), e))
    }
}

/// Extract a frontmatter field value from markdown content.
/// Handles both quoted and unquoted values.
pub fn frontmatter(field: &str, content: &str) -> Option<String> {
    let mut in_frontmatter = false;
    for line in content.lines() {
        if line == "---" {
            if in_frontmatter {
                return None; // End of frontmatter, field not found
            }
            in_frontmatter = true;
            continue;
        }
        if !in_frontmatter {
            continue;
        }
        if let Some(rest) = line.strip_prefix(&format!("{}:", field)) {
            let value = rest.trim();
            // Strip surrounding quotes
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            if value.is_empty() {
                return None;
            }
            return Some(value.to_string());
        }
    }
    None
}

/// Extract date from frontmatter, stripping time portion.
pub fn frontmatter_date(field: &str, content: &str) -> Option<String> {
    frontmatter(field, content).map(|v| v.split_whitespace().next().unwrap_or(&v).to_string())
}

/// Extract post body (everything after the closing --- of frontmatter).
/// Inserts blank lines before list items and blockquotes without preceding blank lines.
pub fn post_body(content: &str) -> String {
    let body_lines = skip_frontmatter(content);
    fix_markdown_spacing(&body_lines)
}

fn skip_frontmatter(content: &str) -> Vec<&str> {
    let mut lines = content.lines();
    let mut count = 0;
    for line in lines.by_ref() {
        if line == "---" {
            count += 1;
            if count == 2 { break; }
        }
    }
    lines.collect()
}

/// Insert blank lines before list items and blockquotes that lack them.
fn fix_markdown_spacing(lines: &[&str]) -> String {
    let mut result = String::new();
    let mut prev_line = "";
    for line in lines {
        let needs_blank = !prev_line.is_empty()
            && (line.starts_with("* ") || line.starts_with("- ") || line.starts_with("> "));
        if needs_blank {
            result.push('\n');
        }
        result.push_str(line);
        result.push('\n');
        prev_line = line;
    }
    result
}

pub struct Post {
    pub slug: String,
    pub title: String,
    pub date: String,
    pub description: String,
    pub image: Option<String>,
    pub content: String,
    pub path: PathBuf,
}

/// Collect all markdown posts from a directory, extracting frontmatter fields.
pub fn collect_posts(posts_dir: &Path, date_field: &str) -> Result<Vec<Post>, String> {
    if !posts_dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries = markdown_entries(posts_dir)?;
    entries.sort_by_key(|e| e.file_name());
    entries.iter().map(|e| post_from_path(&e.path(), date_field)).collect()
}

fn markdown_entries(dir: &Path) -> Result<Vec<fs::DirEntry>, String> {
    Ok(fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .collect())
}

fn post_from_path(path: &Path, date_field: &str) -> Result<Post, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let slug = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    Ok(Post {
        title: frontmatter("title", &content).unwrap_or_default(),
        date: frontmatter_date(date_field, &content).unwrap_or_default(),
        description: frontmatter("description", &content).unwrap_or_default(),
        image: frontmatter("image", &content),
        slug,
        path: path.to_path_buf(),
        content,
    })
}

/// Resolve og:image URL: post image overrides site default.
pub fn resolve_og_image<'a>(post_image: Option<&'a str>, site_image: Option<&'a str>) -> &'a str {
    post_image.or(site_image).unwrap_or("")
}

/// Twitter card type based on whether an og:image is present.
pub fn twitter_card(og_image: &str) -> &'static str {
    if og_image.is_empty() { "summary" } else { "summary_large_image" }
}

/// Extract space-separated tags from post frontmatter.
pub fn extract_tags(content: &str) -> String {
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("tags:") {
            return rest
                .trim()
                .trim_matches(|c| c == '[' || c == ']')
                .replace('"', "")
                .split(',')
                .map(|t| t.trim())
                .collect::<Vec<_>>()
                .join(" ");
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn blog_toml() -> &'static str {
        r#"
title = "Test Blog"
subtitle = "a test subtitle"
url = "https://test.com"
author = "Test Author"
date_field = "date"
lang = "en"
"#
    }

    fn post_md() -> &'static str {
        "---\ntitle: My Test Post\ndate: 2026-03-29\ndescription: A test description\ntags: [\"rust\", \"tdd\"]\n---\n\nSome content here.\n"
    }

    fn post_alt_md() -> &'static str {
        "---\ntitle: \"Alt Date Post\"\npublished_at: \"2024-01-15 03:32:44Z\"\ndescription: \"Alt description\"\n---\n\nAlt content.\n"
    }

    fn post_no_desc_md() -> &'static str {
        "---\ntitle: No Description Post\ndate: 2026-03-29\n---\n\nContent without description.\n"
    }

    fn post_with_hr_md() -> &'static str {
        "---\ntitle: HR Post\ndate: 2026-03-29\n---\n\nBefore rule\n\n---\n\nAfter rule\n"
    }

    // --- BlogConfig ---

    #[test]
    fn cfg_reads_title_from_toml() {
        let tmp = tempdir();
        let path = tmp.join("blog.toml");
        fs::write(&path, blog_toml()).unwrap();
        let config = BlogConfig::from_file(&path).unwrap();
        assert_eq!(config.title, "Test Blog");
    }

    #[test]
    fn cfg_reads_subtitle_from_toml() {
        let tmp = tempdir();
        let path = tmp.join("blog.toml");
        fs::write(&path, blog_toml()).unwrap();
        let config = BlogConfig::from_file(&path).unwrap();
        assert_eq!(config.subtitle.unwrap(), "a test subtitle");
    }

    #[test]
    fn cfg_reads_url_from_toml() {
        let tmp = tempdir();
        let path = tmp.join("blog.toml");
        fs::write(&path, blog_toml()).unwrap();
        let config = BlogConfig::from_file(&path).unwrap();
        assert_eq!(config.url, "https://test.com");
    }

    #[test]
    fn cfg_reads_author_from_toml() {
        let tmp = tempdir();
        let path = tmp.join("blog.toml");
        fs::write(&path, blog_toml()).unwrap();
        let config = BlogConfig::from_file(&path).unwrap();
        assert_eq!(config.author, "Test Author");
    }

    #[test]
    fn cfg_reads_lang_from_toml() {
        let tmp = tempdir();
        let path = tmp.join("blog.toml");
        fs::write(&path, blog_toml()).unwrap();
        let config = BlogConfig::from_file(&path).unwrap();
        assert_eq!(config.lang, "en");
    }

    #[test]
    fn cfg_returns_none_for_missing_optional_fields() {
        let tmp = tempdir();
        let path = tmp.join("blog.toml");
        fs::write(&path, blog_toml()).unwrap();
        let config = BlogConfig::from_file(&path).unwrap();
        assert!(config.theme.is_none());
        assert!(config.analytics_id.is_none());
        assert!(config.links.is_none());
    }

    // --- Frontmatter ---

    #[test]
    fn frontmatter_extracts_title() {
        assert_eq!(frontmatter("title", post_md()).unwrap(), "My Test Post");
    }

    #[test]
    fn frontmatter_extracts_date() {
        assert_eq!(frontmatter("date", post_md()).unwrap(), "2026-03-29");
    }

    #[test]
    fn frontmatter_extracts_description() {
        assert_eq!(frontmatter("description", post_md()).unwrap(), "A test description");
    }

    #[test]
    fn frontmatter_extracts_quoted_title() {
        assert_eq!(frontmatter("title", post_alt_md()).unwrap(), "Alt Date Post");
    }

    #[test]
    fn frontmatter_extracts_published_at() {
        assert_eq!(
            frontmatter("published_at", post_alt_md()).unwrap(),
            "2024-01-15 03:32:44Z"
        );
    }

    #[test]
    fn frontmatter_returns_none_for_missing_field() {
        assert!(frontmatter("description", post_no_desc_md()).is_none());
    }

    // --- Frontmatter Date ---

    #[test]
    fn frontmatter_date_strips_time() {
        assert_eq!(
            frontmatter_date("published_at", post_alt_md()).unwrap(),
            "2024-01-15"
        );
    }

    #[test]
    fn frontmatter_date_keeps_date_only() {
        assert_eq!(
            frontmatter_date("date", post_md()).unwrap(),
            "2026-03-29"
        );
    }

    // --- Post Body ---

    #[test]
    fn post_body_extracts_content_after_frontmatter() {
        let body = post_body(post_md());
        assert!(body.contains("Some content here."));
    }

    #[test]
    fn post_body_does_not_include_frontmatter() {
        let body = post_body(post_md());
        assert!(!body.contains("title:"));
    }

    #[test]
    fn post_body_handles_horizontal_rules_in_content() {
        let body = post_body(post_with_hr_md());
        assert!(body.contains("Before rule"));
        assert!(body.contains("---"));
        assert!(body.contains("After rule"));
    }

    #[test]
    fn post_body_inserts_blank_line_before_star_lists() {
        let content = "---\ntitle: Test\n---\n\nSome text\n* item one\n* item two\n";
        let body = post_body(content);
        assert!(body.contains("Some text\n\n* item one"));
    }

    #[test]
    fn post_body_inserts_blank_line_before_dash_lists() {
        let content = "---\ntitle: Test\n---\n\nSome text\n- item one\n- item two\n";
        let body = post_body(content);
        assert!(body.contains("Some text\n\n- item one"));
    }

    #[test]
    fn post_body_inserts_blank_line_before_blockquotes() {
        let content = "---\ntitle: Test\n---\n\nSome text\n> a quote\n";
        let body = post_body(content);
        assert!(body.contains("Some text\n\n> a quote"));
    }

    fn tempdir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "devtui-test-{}-{}",
            std::process::id(),
            id
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
