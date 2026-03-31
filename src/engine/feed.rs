use std::fs;
use std::path::Path;

use super::config::{self, BlogConfig, Post};
use super::markdown;
use super::seo::xml_escape;

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

/// Generate RSS feed.xml with the 20 most recent posts.
pub fn generate(cfg: &BlogConfig, dist_dir: &Path, posts: &[Post], articles_prefix: &str) -> Result<(), String> {
    let subtitle = cfg.subtitle.as_deref().unwrap_or("");
    let mut feed = rss_header(&cfg.title, &cfg.url, subtitle);
    for post in posts.iter().take(20) {
        let body = config::post_body(&post.content);
        let html_body = markdown::markdown_to_html(&body);
        let prefix_segment = if articles_prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", articles_prefix)
        };
        let article_url = format!("{}/{}{}.html", cfg.url, prefix_segment, post.slug);
        feed.push_str(&rss_item(&post.title, &article_url, &html_body, &post.date));
    }
    feed.push_str("</channel></rss>\n");
    fs::write(dist_dir.join("feed.xml"), &feed).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::tempdir;

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

    // --- rss_header edge cases ---

    #[test]
    fn rss_header_contains_xml_declaration() {
        let result = rss_header("Blog", "https://test.com", "desc");
        assert!(result.starts_with(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
    }

    #[test]
    fn rss_header_contains_description() {
        let result = rss_header("Blog", "https://test.com", "My awesome blog");
        assert!(result.contains("<description>My awesome blog</description>"));
    }

    // --- generate ---

    #[test]
    fn generate_writes_feed_xml() {
        use std::path::PathBuf;
        let tmp = tempdir();
        let cfg = BlogConfig {
            title: "Test Blog".to_string(),
            subtitle: Some("A subtitle".to_string()),
            url: "https://test.com".to_string(),
            author: "Author".to_string(),
            date_field: "date".to_string(),
            lang: "en".to_string(),
            articles_path: None,
            theme: None,
            analytics_id: None,
            license: None,
            license_url: None,
            og_image: None,
            tags: None,
            links: None,
            guides: None,
        };
        let posts = vec![Post {
            slug: "hello".to_string(),
            title: "Hello World".to_string(),
            date: "2026-03-29".to_string(),
            description: "A test post".to_string(),
            image: None,
            content: "---\ntitle: Hello World\ndate: 2026-03-29\n---\n\nSome **bold** content.\n".to_string(),
            path: PathBuf::from("hello.md"),
        }];

        generate(&cfg, &tmp, &posts, "articles").unwrap();

        let feed = std::fs::read_to_string(tmp.join("feed.xml")).unwrap();
        assert!(feed.contains("<title>Test Blog</title>"));
        assert!(feed.contains("<title>Hello World</title>"));
        assert!(feed.contains("articles/hello.html"));
        assert!(feed.contains("</channel></rss>"));
    }

    #[test]
    fn generate_limits_to_20_posts() {
        use std::path::PathBuf;
        let tmp = tempdir();
        let cfg = BlogConfig {
            title: "Blog".to_string(),
            subtitle: None,
            url: "https://test.com".to_string(),
            author: "A".to_string(),
            date_field: "date".to_string(),
            lang: "en".to_string(),
            articles_path: None,
            theme: None,
            analytics_id: None,
            license: None,
            license_url: None,
            og_image: None,
            tags: None,
            links: None,
            guides: None,
        };
        let posts: Vec<Post> = (0..25)
            .map(|i| Post {
                slug: format!("post-{i}"),
                title: format!("Post {i}"),
                date: "2026-03-29".to_string(),
                description: String::new(),
                image: None,
                content: format!("---\ntitle: Post {i}\ndate: 2026-03-29\n---\n\nBody {i}.\n"),
                path: PathBuf::from(format!("post-{i}.md")),
            })
            .collect();

        generate(&cfg, &tmp, &posts, "").unwrap();

        let feed = std::fs::read_to_string(tmp.join("feed.xml")).unwrap();
        let item_count = feed.matches("<item>").count();
        assert_eq!(item_count, 20);
    }

    #[test]
    fn generate_empty_articles_prefix() {
        use std::path::PathBuf;
        let tmp = tempdir();
        let cfg = BlogConfig {
            title: "Blog".to_string(),
            subtitle: None,
            url: "https://test.com".to_string(),
            author: "A".to_string(),
            date_field: "date".to_string(),
            lang: "en".to_string(),
            articles_path: None,
            theme: None,
            analytics_id: None,
            license: None,
            license_url: None,
            og_image: None,
            tags: None,
            links: None,
            guides: None,
        };
        let posts = vec![Post {
            slug: "hello".to_string(),
            title: "Hello".to_string(),
            date: "2026-03-29".to_string(),
            description: String::new(),
            image: None,
            content: "---\ntitle: Hello\ndate: 2026-03-29\n---\n\nBody.\n".to_string(),
            path: PathBuf::from("hello.md"),
        }];

        generate(&cfg, &tmp, &posts, "").unwrap();

        let feed = std::fs::read_to_string(tmp.join("feed.xml")).unwrap();
        assert!(feed.contains("https://test.com/hello.html"));
    }

}
