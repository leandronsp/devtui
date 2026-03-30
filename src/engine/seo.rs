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

pub fn rss_header(title: &str, url: &str, description: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
<channel>
<title>{title}</title>
<link>{url}</link>
<description>{description}</description>
<atom:link href="{url}/feed.xml" rel="self" type="application/rss+xml"/>
"#
    )
}

pub fn rss_item(title: &str, link: &str, description: &str, date: &str) -> String {
    let escaped_title = xml_escape(title);
    format!(
        "<item>\n<title>{escaped_title}</title>\n<link>{link}</link>\n<guid>{link}</guid>\n<description><![CDATA[{description}]]></description>\n<pubDate>{date}</pubDate>\n</item>\n"
    )
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

    // --- rss_header ---

    #[test]
    fn rss_header_contains_channel_title() {
        let result = rss_header("My Blog", "https://test.com", "A blog");
        assert!(result.contains("<title>My Blog</title>"));
    }

    #[test]
    fn rss_header_contains_channel_link() {
        let result = rss_header("My Blog", "https://test.com", "A blog");
        assert!(result.contains("<link>https://test.com</link>"));
    }

    #[test]
    fn rss_header_contains_atom_self_link() {
        let result = rss_header("My Blog", "https://test.com", "A blog");
        assert!(result.contains(r#"href="https://test.com/feed.xml""#));
    }

    // --- rss_item ---

    #[test]
    fn rss_item_contains_title_and_link() {
        let result = rss_item("Post", "https://test.com/post.html", "desc", "2026-03-29");
        assert!(result.contains("<title>Post</title>"));
        assert!(result.contains("<link>https://test.com/post.html</link>"));
    }

    #[test]
    fn rss_item_contains_guid() {
        let result = rss_item("Post", "https://test.com/post.html", "desc", "2026-03-29");
        assert!(result.contains("<guid>https://test.com/post.html</guid>"));
    }

    #[test]
    fn rss_item_contains_pub_date() {
        let result = rss_item("Post", "https://test.com/post.html", "desc", "2026-03-29");
        assert!(result.contains("<pubDate>2026-03-29</pubDate>"));
    }

    #[test]
    fn rss_item_escapes_ampersand_in_title() {
        let result = rss_item("AI & Ruby", "https://test.com/post.html", "desc", "2026-03-29");
        assert!(result.contains("<title>AI &amp; Ruby</title>"));
    }

    #[test]
    fn rss_item_wraps_description_in_cdata() {
        let result = rss_item(
            "Post",
            "https://test.com/post.html",
            r#"A <b>bold</b> & "quoted""#,
            "2026-03-29",
        );
        assert!(result.contains(r#"<description><![CDATA[A <b>bold</b> & "quoted"]]></description>"#));
    }
}
