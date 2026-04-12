use super::config::BlogConfig;

/// Render social links as nav HTML.
pub fn render_links(config: &BlogConfig) -> String {
    let Some(links) = &config.links else {
        return String::new();
    };
    if links.is_empty() {
        return String::new();
    }
    let items: Vec<String> = links
        .iter()
        .map(|link| {
            format!(
                r#"<a href="{}" target="_blank" rel="noopener">{}</a>"#,
                link.url, link.label
            )
        })
        .collect();
    format!(r#"<nav class="social-links">{}</nav>"#, items.join(" &middot; "))
}

/// Render tag filter buttons.
pub fn render_tags(config: &BlogConfig) -> String {
    let Some(tags) = &config.tags else {
        return String::new();
    };
    if tags.is_empty() {
        return String::new();
    }
    let mut buttons = vec![r#"<button class="tag-btn active" data-tag="all">all</button>"#.to_string()];
    for tag in tags {
        buttons.push(format!(r#"<button class="tag-btn" data-tag="{tag}">{tag}</button>"#));
    }
    buttons.join(" &middot; ")
}

/// Render guide badges.
pub fn render_guides(config: &BlogConfig) -> String {
    let Some(guides) = &config.guides else {
        return String::new();
    };
    if guides.is_empty() {
        return String::new();
    }
    let items: Vec<String> = guides
        .iter()
        .map(|guide| {
            format!(r#"<a href="{}" class="guide-badge" target="_blank" rel="noopener">{}</a>"#, guide.url, guide.title)
        })
        .collect();
    format!(r#"<span class="guides">{}</span>"#, items.join(" &middot; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_links() -> BlogConfig {
        toml::from_str(
            r#"
title = "Test"
url = "https://test.com"
author = "Author"
lang = "en"

[[links]]
label = "github"
url = "https://github.com/user"

[[links]]
label = "linkedin"
url = "https://linkedin.com/in/user"
"#,
        )
        .unwrap()
    }

    fn config_with_tags() -> BlogConfig {
        toml::from_str(
            r#"
title = "Test"
url = "https://test.com"
author = "Author"
lang = "en"
tags = ["ruby", "rust", "docker"]
"#,
        )
        .unwrap()
    }

    fn config_with_guides() -> BlogConfig {
        toml::from_str(
            r#"
title = "Test"
url = "https://test.com"
author = "Author"
lang = "en"

[[guides]]
title = "Web 101"
url = "https://web101.test.com"

[[guides]]
title = "AWS 101"
url = "https://aws101.test.com"
"#,
        )
        .unwrap()
    }

    fn config_empty() -> BlogConfig {
        toml::from_str(
            r#"
title = "Test"
url = "https://test.com"
author = "Author"
lang = "en"
"#,
        )
        .unwrap()
    }

    // --- render_links ---

    #[test]
    fn render_links_generates_nav_with_links() {
        let result = render_links(&config_with_links());
        assert!(result.contains(r#"<nav class="social-links">"#));
        assert!(result.contains("github"));
        assert!(result.contains("linkedin"));
        assert!(result.contains(" &middot; "));
    }

    #[test]
    fn render_links_returns_empty_when_no_links() {
        assert!(render_links(&config_empty()).is_empty());
    }

    // --- render_tags ---

    #[test]
    fn render_tags_generates_buttons() {
        let result = render_tags(&config_with_tags());
        assert!(result.contains(r#"data-tag="ruby""#));
        assert!(result.contains(r#"data-tag="rust""#));
        assert!(result.contains(r#"data-tag="docker""#));
        assert!(result.contains("active")); // "all" button
    }

    #[test]
    fn render_tags_returns_empty_when_no_tags() {
        assert!(render_tags(&config_empty()).is_empty());
    }

    // --- render_guides ---

    #[test]
    fn render_guides_generates_badge_list() {
        let result = render_guides(&config_with_guides());
        assert!(result.contains(r#"class="guide-badge""#));
        assert!(result.contains("Web 101"));
        assert!(result.contains("AWS 101"));
    }

    #[test]
    fn render_guides_returns_empty_when_no_guides() {
        assert!(render_guides(&config_empty()).is_empty());
    }
}
