pub fn normalize_frontmatter(content: &str) -> String {
    let Some((frontmatter, body)) = split_frontmatter(content) else {
        return content.to_string();
    };
    let rewritten = frontmatter.replace("date:", "published_at:");
    format!("---\n{rewritten}---\n{body}")
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
        let expected = "---\npublished_at: 2026-03-28\n---\n";
        assert_eq!(normalize_frontmatter(input), expected);
    }

    #[test]
    fn normalize_does_not_touch_body_text() {
        let input = "---\ndate: 2026-03-28\n---\n\nthe date: was yesterday\n";
        let expected = "---\npublished_at: 2026-03-28\n---\n\nthe date: was yesterday\n";
        assert_eq!(normalize_frontmatter(input), expected);
    }
}
