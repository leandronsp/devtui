pub fn normalize_frontmatter(content: &str) -> String {
    let Some((frontmatter, body)) = split_frontmatter(content) else {
        return content.to_string();
    };
    let rewritten: String = frontmatter
        .lines()
        .map(rewrite_line)
        .map(|line| format!("{line}\n"))
        .collect();
    format!("---\n{rewritten}---\n{body}")
}

fn rewrite_line(line: &str) -> String {
    let Some((key, value)) = line.split_once(':') else {
        return line.to_string();
    };
    let key = if key == "date" { "published_at" } else { key };
    let trimmed = value.trim();
    if trimmed.is_empty() || is_already_quoted_or_collection(trimmed) {
        return format!("{key}:{value}");
    }
    format!("{key}: \"{trimmed}\"")
}

fn is_already_quoted_or_collection(value: &str) -> bool {
    value.starts_with('"') || value.starts_with('[') || value.starts_with('\'')
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
    fn normalize_does_not_touch_body_text() {
        let input = "---\ndate: 2026-03-28\n---\n\nthe date: was yesterday\n";
        let expected = "---\npublished_at: \"2026-03-28\"\n---\n\nthe date: was yesterday\n";
        assert_eq!(normalize_frontmatter(input), expected);
    }
}
