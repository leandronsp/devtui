use std::fs;

use super::config::BlogConfig;

/// Inject Google Analytics script into rebuilt article HTML files.
pub fn inject(cfg: &BlogConfig, rebuilt_articles: &[String]) -> Result<(), String> {
    let analytics_id = cfg.analytics_id.as_deref().unwrap_or("");
    if analytics_id.is_empty() {
        return Ok(());
    }
    let ga_script = ga_script_tag(analytics_id);
    for html_path in rebuilt_articles {
        let content = fs::read_to_string(html_path).map_err(|e| e.to_string())?;
        let content = content.replace("</body>", &format!("{ga_script}</body>"));
        fs::write(html_path, content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Generate a lazy-loaded Google Analytics script tag.
/// Returns empty string if analytics_id is empty.
pub fn ga_script_tag(analytics_id: &str) -> String {
    if analytics_id.is_empty() {
        return String::new();
    }
    format!("<script>window.addEventListener('load',function(){{setTimeout(function(){{var s=document.createElement('script');s.src='https://www.googletagmanager.com/gtag/js?id={analytics_id}';s.async=true;document.head.appendChild(s);s.onload=function(){{window.dataLayer=window.dataLayer||[];function g(){{dataLayer.push(arguments)}}g('js',new Date());g('config','{analytics_id}')}}}},2000)}})</script>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{tempdir, test_blog_config};

    #[test]
    fn ga_script_tag_returns_empty_for_empty_id() {
        assert!(ga_script_tag("").is_empty());
    }

    #[test]
    fn ga_script_tag_contains_analytics_id() {
        let result = ga_script_tag("G-ABC123");
        assert!(result.contains("G-ABC123"));
        assert!(result.contains("<script>"));
        assert!(result.contains("gtag"));
    }

    #[test]
    fn ga_script_tag_contains_lazy_load_pattern() {
        let result = ga_script_tag("G-XYZ");
        assert!(result.contains("addEventListener('load'"));
        assert!(result.contains("setTimeout"));
    }

    // --- inject ---

    #[test]
    fn inject_skips_when_no_analytics_id() {
        let cfg = test_blog_config();
        inject(&cfg, &[]).unwrap();
    }

    #[test]
    fn inject_inserts_script_before_body_close() {
        let tmp = tempdir();
        let html_path = tmp.join("test.html");
        std::fs::write(&html_path, "<html><body><p>hi</p></body></html>").unwrap();

        let cfg = BlogConfig {
            analytics_id: Some("G-TEST123".to_string()),
            ..test_blog_config()
        };

        inject(&cfg, &[html_path.to_string_lossy().to_string()]).unwrap();

        let content = std::fs::read_to_string(&html_path).unwrap();
        assert!(content.contains("G-TEST123"));
        assert!(content.contains("<script>"));
        assert!(content.ends_with("</body></html>"));
    }

}
