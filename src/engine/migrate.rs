pub fn normalize_frontmatter(content: &str) -> String {
    content.replace("date:", "published_at:")
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
}
