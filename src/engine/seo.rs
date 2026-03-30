use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::config::BlogConfig;
use super::template;

pub fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn sitemap_entry(url: &str, lastmod: &str) -> String {
    format!("<url><loc>{url}</loc><lastmod>{lastmod}</lastmod></url>")
}

pub fn robots_txt(base_url: &str) -> String {
    format!("User-agent: *\nAllow: /\nSitemap: {base_url}/sitemap.xml\n")
}

/// Generate sitemap.xml with index URL and article entries.
pub fn sitemap(cfg: &BlogConfig, dist_dir: &Path, sitemap_entries: &str) -> Result<(), String> {
    let sitemap = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n<url><loc>{}/</loc></url>\n{}</urlset>",
        cfg.url, sitemap_entries
    );
    fs::write(dist_dir.join("sitemap.xml"), &sitemap).map_err(|e| e.to_string())
}

/// Generate robots.txt and 404.html.
pub fn generate_files(
    cfg: &BlogConfig,
    dist_dir: &Path,
    blog_dir: &Path,
    theme_dir: &Path,
    engine_dir: &Path,
) -> Result<(), String> {
    fs::write(dist_dir.join("robots.txt"), robots_txt(&cfg.url)).map_err(|e| e.to_string())?;

    let error_tpl_path = template::resolve_file(
        "404.html",
        &blog_dir.join("templates"),
        &theme_dir.join("templates"),
        &engine_dir.join("templates"),
    )
    .ok_or("404.html template not found")?;
    let error_tpl = fs::read_to_string(&error_tpl_path).map_err(|e| e.to_string())?;
    let error_vars = HashMap::from([("title", cfg.title.as_str()), ("lang", cfg.lang.as_str())]);
    let error_html = template::template_render(&error_tpl, &error_vars);
    fs::write(dist_dir.join("404.html"), &error_html).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- xml_escape ---

    #[test]
    fn xml_escape_escapes_ampersand() {
        assert_eq!(xml_escape("A & B"), "A &amp; B");
    }

    #[test]
    fn xml_escape_escapes_angle_brackets() {
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
    }

    #[test]
    fn xml_escape_escapes_quotes() {
        assert_eq!(xml_escape(r#"say "hi""#), "say &quot;hi&quot;");
    }

    #[test]
    fn xml_escape_preserves_utf8() {
        assert_eq!(xml_escape("café"), "café");
    }

    // --- sitemap_entry ---

    #[test]
    fn sitemap_entry_generates_valid_xml() {
        let result = sitemap_entry("https://test.com/post.html", "2026-03-29");
        assert_eq!(
            result,
            "<url><loc>https://test.com/post.html</loc><lastmod>2026-03-29</lastmod></url>"
        );
    }

    // --- robots_txt ---

    #[test]
    fn robots_txt_contains_user_agent_allow_and_sitemap() {
        let result = robots_txt("https://test.com");
        assert!(result.contains("User-agent: *"));
        assert!(result.contains("Allow: /"));
        assert!(result.contains("Sitemap: https://test.com/sitemap.xml"));
    }

}
