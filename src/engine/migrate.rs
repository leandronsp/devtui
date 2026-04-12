use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct MigrationReport {
    pub rewritten: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

pub fn migrate_posts(posts_dir: &Path) -> io::Result<MigrationReport> {
    let mut report = MigrationReport::default();
    for entry in fs::read_dir(posts_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let original = fs::read_to_string(&path)?;
        let normalized = normalize_frontmatter(&original);
        if normalized == original {
            report.skipped.push(path);
        } else {
            fs::write(&path, normalized)?;
            report.rewritten.push(path);
        }
    }
    Ok(report)
}

const CANONICAL_ORDER: &[&str] = &[
    "title",
    "subtitle",
    "description",
    "slug",
    "published_at",
    "language",
    "tags",
    "image",
    "status",
    "pinned",
];

pub fn normalize_frontmatter(content: &str) -> String {
    let Some((frontmatter, body)) = split_frontmatter(content) else {
        return content.to_string();
    };
    let fields: Vec<(String, String)> = frontmatter
        .lines()
        .filter_map(parse_field)
        .map(|(key, value)| (rename_key(&key), normalize_value(&value)))
        .collect();
    let ordered = reorder_canonical(fields);
    let body_block: String = ordered
        .into_iter()
        .map(|(key, value)| format!("{key}: {value}\n"))
        .collect();
    format!("---\n{body_block}---\n{body}")
}

fn parse_field(line: &str) -> Option<(String, String)> {
    let (key, value) = line.split_once(':')?;
    Some((key.trim().to_string(), value.trim().to_string()))
}

fn rename_key(key: &str) -> String {
    if key == "date" {
        return "published_at".to_string();
    }
    key.to_string()
}

fn normalize_value(value: &str) -> String {
    if value.is_empty() || is_already_quoted_or_collection(value) {
        return value.to_string();
    }
    format!("\"{value}\"")
}

fn is_already_quoted_or_collection(value: &str) -> bool {
    value.starts_with('"') || value.starts_with('[') || value.starts_with('\'')
}

fn reorder_canonical(fields: Vec<(String, String)>) -> Vec<(String, String)> {
    let canonical_index = |key: &str| {
        CANONICAL_ORDER
            .iter()
            .position(|&k| k == key)
            .map(|i| i as i32)
            .unwrap_or(i32::MAX)
    };
    let mut ordered = fields;
    ordered.sort_by_key(|(key, _)| canonical_index(key));
    ordered
}

fn split_frontmatter(content: &str) -> Option<(String, String)> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    let frontmatter = &rest[..=end];
    let body = &rest[end + "\n---\n".len()..];
    Some((frontmatter.to_string(), body.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::tempdir;
    use std::fs;

    #[test]
    fn migrate_posts_skips_canonical_files() {
        let dir = tempdir();
        let post = dir.join("canonical.md");
        let canonical = "---\ntitle: \"Foo\"\npublished_at: \"2026-03-28\"\n---\n\nBody.\n";
        fs::write(&post, canonical).unwrap();

        let report = migrate_posts(&dir).unwrap();

        assert_eq!(fs::read_to_string(&post).unwrap(), canonical);
        assert_eq!(report.skipped, vec![post]);
        assert!(report.rewritten.is_empty());
    }

    #[test]
    fn migrate_posts_ignores_non_markdown_files() {
        let dir = tempdir();
        let txt = dir.join("notes.txt");
        fs::write(&txt, "---\ndate: x\n---\n").unwrap();

        let report = migrate_posts(&dir).unwrap();

        assert_eq!(fs::read_to_string(&txt).unwrap(), "---\ndate: x\n---\n");
        assert!(report.rewritten.is_empty());
        assert!(report.skipped.is_empty());
    }

    #[test]
    fn migrate_posts_rewrites_non_canonical_files() {
        let dir = tempdir();
        let post = dir.join("minimal.md");
        fs::write(&post, "---\ntitle: Foo\ndate: 2026-03-28\n---\n\nBody.\n").unwrap();

        let report = migrate_posts(&dir).unwrap();

        let after = fs::read_to_string(&post).unwrap();
        assert_eq!(
            after,
            "---\ntitle: \"Foo\"\npublished_at: \"2026-03-28\"\n---\n\nBody.\n"
        );
        assert_eq!(report.rewritten, vec![post]);
        assert!(report.skipped.is_empty());
    }


    #[test]
    fn normalize_renames_date_to_published_at() {
        let input = "---\ndate: 2026-03-28\n---\n";
        let expected = "---\npublished_at: \"2026-03-28\"\n---\n";
        assert_eq!(normalize_frontmatter(input), expected);
    }

    #[test]
    fn normalize_quotes_unquoted_title() {
        let input = "---\ntitle: Foo\n---\n";
        let expected = "---\ntitle: \"Foo\"\n---\n";
        assert_eq!(normalize_frontmatter(input), expected);
    }

    #[test]
    fn normalize_only_renames_exact_date_field() {
        let input = "---\nlast_date: 2026-03-28\n---\n";
        let expected = "---\nlast_date: \"2026-03-28\"\n---\n";
        assert_eq!(normalize_frontmatter(input), expected);
    }

    #[test]
    fn normalize_leaves_canonical_leandronsp_post_unchanged() {
        let input = "---\n\
title: \"A brief history\"\n\
slug: \"a-brief-history\"\n\
published_at: \"2022-07-12 06:11:29Z\"\n\
language: \"en\"\n\
status: \"published\"\n\
tags: [\"unix\", \"linux\"]\n\
---\n\n\
Body text.\n";
        // canonical order is title, slug(after description), published_at, language, tags(before image), status
        let expected = "---\n\
title: \"A brief history\"\n\
slug: \"a-brief-history\"\n\
published_at: \"2022-07-12 06:11:29Z\"\n\
language: \"en\"\n\
tags: [\"unix\", \"linux\"]\n\
status: \"published\"\n\
---\n\n\
Body text.\n";
        assert_eq!(normalize_frontmatter(input), expected);
    }

    #[test]
    fn normalize_rewrites_minimal_acme_post_to_canonical() {
        let input = "---\ntitle: Why I Live in the Terminal\ndate: 2026-03-28\n---\n\nBody.\n";
        let expected =
            "---\ntitle: \"Why I Live in the Terminal\"\npublished_at: \"2026-03-28\"\n---\n\nBody.\n";
        assert_eq!(normalize_frontmatter(input), expected);
    }

    #[test]
    fn normalize_passes_through_unknown_fields() {
        let input = "---\ntitle: Foo\ncustom: bar\n---\n";
        let expected = "---\ntitle: \"Foo\"\ncustom: \"bar\"\n---\n";
        assert_eq!(normalize_frontmatter(input), expected);
    }

    #[test]
    fn normalize_reorders_fields_to_canonical() {
        let input = "---\npublished_at: \"2026-03-28\"\ntitle: \"Foo\"\n---\n";
        let expected = "---\ntitle: \"Foo\"\npublished_at: \"2026-03-28\"\n---\n";
        assert_eq!(normalize_frontmatter(input), expected);
    }

    #[test]
    fn normalize_is_idempotent_on_canonical_input() {
        let canonical = "---\ntitle: \"Foo\"\npublished_at: \"2026-03-28\"\n---\n\nBody\n";
        let once = normalize_frontmatter(canonical);
        let twice = normalize_frontmatter(&once);
        assert_eq!(once, canonical);
        assert_eq!(twice, canonical);
    }

    #[test]
    fn normalize_does_not_touch_body_text() {
        let input = "---\ndate: 2026-03-28\n---\n\nthe date: was yesterday\n";
        let expected = "---\npublished_at: \"2026-03-28\"\n---\n\nthe date: was yesterday\n";
        assert_eq!(normalize_frontmatter(input), expected);
    }
}
