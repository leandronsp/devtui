use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::analytics;
use super::config::{self, BlogConfig, Post};
use super::feed;
use super::index;
use super::markdown;
use super::minify;
use super::seo;
use super::template;

pub struct BuildReport {
    pub built: usize,
    pub skipped: usize,
}

/// Build a blog from blog_dir to dist_dir.
/// engine_dir is the path to engine/ (templates and themes).
pub fn build(blog_dir: &Path, dist_dir: &Path, engine_dir: &Path) -> Result<BuildReport, String> {
    let config_path = blog_dir.join("blog.toml");
    let cfg = BlogConfig::from_file(&config_path)?;
    let posts_dir = blog_dir.join("posts");

    let theme = cfg.theme.as_deref().unwrap_or("paper");
    let theme_dir = engine_dir.join("themes").join(theme);

    fs::create_dir_all(dist_dir).map_err(|e| e.to_string())?;

    let articles_prefix = cfg.articles_path.as_deref().unwrap_or("");
    let articles_dir = if articles_prefix.is_empty() {
        dist_dir.to_path_buf()
    } else {
        dist_dir.join(articles_prefix)
    };
    fs::create_dir_all(&articles_dir).map_err(|e| e.to_string())?;

    let mut posts = config::collect_posts(&posts_dir, &cfg.date_field)?;

    let (rebuilt_articles, sitemap_entries, skipped) = render_articles(
        &cfg, &posts, &articles_dir, articles_prefix, blog_dir, &theme_dir, engine_dir,
    )?;

    let css = minify::compile_css(blog_dir, &theme_dir)?;
    fs::write(dist_dir.join("style.css"), &css).map_err(|e| e.to_string())?;

    copy_static_assets(blog_dir, dist_dir)?;

    posts.sort_by(|a, b| b.date.cmp(&a.date));

    index::build(&cfg, dist_dir, &posts, articles_prefix, blog_dir, &theme_dir, engine_dir)?;
    seo::sitemap(&cfg, dist_dir, &sitemap_entries)?;
    feed::generate(&cfg, dist_dir, &posts, articles_prefix)?;
    seo::generate_files(&cfg, dist_dir, blog_dir, &theme_dir, engine_dir)?;
    analytics::inject(&cfg, &rebuilt_articles)?;
    minify::minify_and_inline(dist_dir, &css, &rebuilt_articles)?;

    Ok(BuildReport {
        built: rebuilt_articles.len(),
        skipped,
    })
}

// --- Article rendering ---

fn render_articles(
    cfg: &BlogConfig,
    posts: &[Post],
    articles_dir: &Path,
    articles_prefix: &str,
    blog_dir: &Path,
    theme_dir: &Path,
    engine_dir: &Path,
) -> Result<(Vec<String>, String, usize), String> {
    let article_tpl_path = template::resolve_file(
        "article.html",
        &blog_dir.join("templates"),
        &theme_dir.join("templates"),
        &engine_dir.join("templates"),
    )
    .ok_or("article.html template not found")?;
    let article_tpl = fs::read_to_string(&article_tpl_path).map_err(|e| e.to_string())?;

    let mut sitemap_entries = String::new();
    let mut rebuilt_articles: Vec<String> = Vec::new();
    let mut skipped = 0;

    for post in posts {
        let html_out = articles_dir.join(format!("{}.html", post.slug));

        if html_out.exists() && is_newer(&html_out, &post.path) && is_newer(&html_out, &article_tpl_path) {
            skipped += 1;
        } else {
            render_article(cfg, post, articles_prefix, &article_tpl, &html_out)?;
            rebuilt_articles.push(html_out.to_string_lossy().to_string());
        }

        let article_url = article_url(&cfg.url, articles_prefix, &post.slug);
        sitemap_entries.push_str(&seo::sitemap_entry(&article_url, &post.date));
        sitemap_entries.push('\n');
    }

    Ok((rebuilt_articles, sitemap_entries, skipped))
}

fn render_article(
    cfg: &BlogConfig,
    post: &Post,
    articles_prefix: &str,
    article_tpl: &str,
    html_out: &Path,
) -> Result<(), String> {
    let rendered = render_article_html_from_post(cfg, post, articles_prefix, article_tpl);
    fs::write(html_out, &rendered).map_err(|e| e.to_string())
}

fn render_article_html_from_post(
    cfg: &BlogConfig,
    post: &Post,
    articles_prefix: &str,
    article_tpl: &str,
) -> String {
    let body = config::post_body(&post.content);
    let html_body = markdown::markdown_to_html(&body);
    let description = if post.description.is_empty() {
        markdown::post_snippet(&body, None, 160)
    } else {
        post.description.to_string()
    };

    let base_path = if articles_prefix.is_empty() { "" } else { "../" };
    let slug_with_prefix = if articles_prefix.is_empty() {
        post.slug.clone()
    } else {
        format!("{}/{}", articles_prefix, post.slug)
    };

    let og_image = config::resolve_og_image(post.image.as_deref(), None);
    let twitter_card = config::twitter_card(og_image);

    let vars = HashMap::from([
        ("title", post.title.as_str()),
        ("body", html_body.as_str()),
        ("date", post.date.as_str()),
        ("description", description.as_str()),
        ("site-title", cfg.title.as_str()),
        ("site-author", cfg.author.as_str()),
        ("site-url", cfg.url.as_str()),
        ("slug", slug_with_prefix.as_str()),
        ("lang", cfg.lang.as_str()),
        ("base-path", base_path),
        ("og-image", og_image),
        ("twitter-card", twitter_card),
    ]);

    template::template_render(article_tpl, &vars)
}

/// Render markdown content to final HTML string (with template applied).
/// Pure function, no disk I/O. Used by the CMS preview.
pub fn render_preview_html(
    content: &str,
    title: &str,
    cfg: &BlogConfig,
    article_tpl: &str,
) -> String {
    // Content may or may not have frontmatter. Try to extract body, fall back to raw content.
    let has_frontmatter = content.starts_with("---");
    let body = if has_frontmatter {
        config::post_body(content)
    } else {
        content.to_string()
    };
    let html_body = markdown::markdown_to_html(&body);
    let description = markdown::post_snippet(&body, None, 160);
    let date = if has_frontmatter {
        config::frontmatter_date(&cfg.date_field, content).unwrap_or_default()
    } else {
        String::new()
    };

    let vars = HashMap::from([
        ("title", title),
        ("body", html_body.as_str()),
        ("date", date.as_str()),
        ("description", description.as_str()),
        ("site-title", cfg.title.as_str()),
        ("site-author", cfg.author.as_str()),
        ("site-url", cfg.url.as_str()),
        ("slug", "preview"),
        ("lang", cfg.lang.as_str()),
        ("base-path", ""),
        ("og-image", ""),
        ("twitter-card", "summary"),
    ]);

    template::template_render(article_tpl, &vars)
}

fn article_url(base_url: &str, articles_prefix: &str, slug: &str) -> String {
    format!("{}/{}", base_url, config::article_href(articles_prefix, slug))
}

// --- Static assets ---

fn copy_static_assets(blog_dir: &Path, dist_dir: &Path) -> Result<(), String> {
    for dir_name in &["uploads", "images", "assets"] {
        let src = blog_dir.join(dir_name);
        if src.is_dir() {
            copy_dir_recursive(&src, &dist_dir.join(dir_name))?;
        }
    }
    Ok(())
}

// --- Helpers ---


fn is_newer(a: &Path, b: &Path) -> bool {
    let a_mod = fs::metadata(a).and_then(|m| m.modified()).ok();
    let b_mod = fs::metadata(b).and_then(|m| m.modified()).ok();
    match (a_mod, b_mod) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = dst.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::tempdir;
    use std::path::PathBuf;

    fn engine_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/engine")
    }

    fn setup_blog(tmp: &Path) -> (PathBuf, PathBuf) {
        let blog = tmp.join("blog");
        let dist = tmp.join("dist");
        let posts = blog.join("posts");
        fs::create_dir_all(&posts).expect("failed to create test blog dir");

        fs::write(
            blog.join("blog.toml"),
            r#"
title = "Test Blog"
subtitle = "a test subtitle"
url = "https://test-blog.com"
author = "Test Author"
date_field = "date"
lang = "en"
"#,
        )
        .expect("failed to write test blog.toml");

        fs::write(
            posts.join("2026-01-01-hello.md"),
            r#"---
title: Hello World
date: 2026-01-01
description: A hello post
language: en
tags: ["rust", "tdd"]
---

This is the **first** post.
"#,
        )
        .expect("failed to write test post");

        fs::write(
            posts.join("2026-01-02-second.md"),
            r#"---
title: Second Post
date: 2026-01-02
description: The second post
language: en
tags: ["rust"]
---

This is the second post.
"#,
        )
        .expect("failed to write test post");

        (blog, dist)
    }

    fn do_build(blog: &Path, dist: &Path) -> BuildReport {
        build(blog, dist, &engine_dir()).expect("build failed")
    }

    // --- Article generation ---

    #[test]
    fn build_generates_article_html_files() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        assert!(dist.join("2026-01-01-hello.html").exists());
        assert!(dist.join("2026-01-02-second.html").exists());
    }

    #[test]
    fn build_article_has_correct_title_tag() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-01-hello.html")).expect("read article");
        assert!(html.contains("<title>Hello World - Test Blog</title>"));
    }

    #[test]
    fn build_article_has_canonical_url() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-01-hello.html")).expect("read article");
        assert!(html.contains(r#"rel="canonical" href="https://test-blog.com/2026-01-01-hello.html""#));
    }

    #[test]
    fn build_article_has_open_graph_tags() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-01-hello.html")).expect("read article");
        assert!(html.contains("og:title"));
        assert!(html.contains("og:description"));
        assert!(html.contains("og:url"));
    }

    #[test]
    fn build_article_without_image_has_no_og_image() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-01-hello.html")).expect("read article");
        assert!(!html.contains("og:image"));
        assert!(!html.contains("twitter:image"));
        assert!(html.contains(r#"twitter:card" content="summary""#));
    }

    #[test]
    fn build_article_with_image_has_og_image() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        fs::write(
            blog.join("posts/2026-01-09-with-image.md"),
            "---\ntitle: Image Post\ndate: 2026-01-09\ndescription: A post with image\nimage: https://example.com/cover.png\n---\n\nContent.\n",
        )
        .expect("write test post");
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-09-with-image.html")).expect("read article");
        assert!(html.contains(r#"og:image" content="https://example.com/cover.png""#));
        assert!(html.contains(r#"twitter:image" content="https://example.com/cover.png""#));
        assert!(html.contains(r#"twitter:card" content="summary_large_image""#));
    }

    #[test]
    fn build_index_with_og_image_has_og_image() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        let toml_path = blog.join("blog.toml");
        let toml = fs::read_to_string(&toml_path).expect("read blog.toml");
        fs::write(&toml_path, format!("{toml}og_image = \"https://test-blog.com/og.png\"\n")).expect("write blog.toml");
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("index.html")).expect("read index");
        assert!(html.contains(r#"og:image" content="https://test-blog.com/og.png""#));
        assert!(html.contains(r#"twitter:card" content="summary_large_image""#));
    }

    #[test]
    fn build_article_with_site_og_image_does_not_inherit() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        let toml_path = blog.join("blog.toml");
        let toml = fs::read_to_string(&toml_path).expect("read blog.toml");
        fs::write(&toml_path, format!("{toml}og_image = \"https://test-blog.com/og.png\"\n")).expect("write blog.toml");
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-01-hello.html")).expect("read article");
        assert!(!html.contains("og:image"));
    }

    #[test]
    fn build_article_has_json_ld_schema() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-01-hello.html")).expect("read article");
        assert!(html.contains("BlogPosting"));
    }

    #[test]
    fn build_article_has_semantic_time_element() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-01-hello.html")).expect("read article");
        assert!(html.contains(r#"datetime="2026-01-01""#));
    }

    #[test]
    fn build_article_has_nav_link_back_to_index() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-01-hello.html")).expect("read article");
        assert!(html.contains(r#"href="index.html""#));
    }

    #[test]
    fn build_article_has_site_title_in_nav() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-01-hello.html")).expect("read article");
        assert!(html.contains(">Test Blog<"));
    }

    // --- Index page ---

    #[test]
    fn build_generates_index_html() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        assert!(dist.join("index.html").exists());
    }

    #[test]
    fn build_index_has_correct_title() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("index.html")).expect("read index");
        assert!(html.contains("<title>Test Blog</title>"));
    }

    #[test]
    fn build_index_has_h1_with_site_title() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("index.html")).expect("read index");
        assert!(html.contains("Test Blog"));
    }

    #[test]
    fn build_index_lists_posts() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("index.html")).expect("read index");
        assert!(html.contains("Hello World"));
        assert!(html.contains("Second Post"));
    }

    #[test]
    fn build_index_has_canonical_url() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("index.html")).expect("read index");
        assert!(html.contains(r#"canonical" href="https://test-blog.com/""#));
    }

    #[test]
    fn build_index_has_json_ld_blog_schema() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("index.html")).expect("read index");
        assert!(html.contains(r#""Blog""#));
    }

    #[test]
    fn build_index_has_filter_script() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("index.html")).expect("read index");
        assert!(html.contains("filterPosts"));
        assert!(html.contains("bindFilter"));
        assert!(html.contains("activeLang"));
        assert!(html.contains("activeTag"));
    }

    // --- Sitemap ---

    #[test]
    fn build_generates_sitemap_xml() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        assert!(dist.join("sitemap.xml").exists());
    }

    #[test]
    fn build_sitemap_has_index_url() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let xml = fs::read_to_string(dist.join("sitemap.xml")).expect("read sitemap");
        assert!(xml.contains("<loc>https://test-blog.com/</loc>"));
    }

    #[test]
    fn build_sitemap_has_article_urls() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let xml = fs::read_to_string(dist.join("sitemap.xml")).expect("read sitemap");
        assert!(xml.contains("test-blog.com/2026-01-01-hello.html"));
        assert!(xml.contains("test-blog.com/2026-01-02-second.html"));
    }

    #[test]
    fn build_sitemap_has_lastmod_dates() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let xml = fs::read_to_string(dist.join("sitemap.xml")).expect("read sitemap");
        assert!(xml.contains("<lastmod>2026-01-01</lastmod>"));
        assert!(xml.contains("<lastmod>2026-01-02</lastmod>"));
    }

    // --- Robots.txt ---

    #[test]
    fn build_generates_robots_txt() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        assert!(dist.join("robots.txt").exists());
    }

    #[test]
    fn build_robots_txt_references_sitemap() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let txt = fs::read_to_string(dist.join("robots.txt")).expect("read robots.txt");
        assert!(txt.contains("Sitemap: https://test-blog.com/sitemap.xml"));
    }

    // --- Feed ---

    #[test]
    fn build_generates_feed_xml() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        assert!(dist.join("feed.xml").exists());
    }

    #[test]
    fn build_feed_has_channel_title() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let xml = fs::read_to_string(dist.join("feed.xml")).expect("read feed");
        assert!(xml.contains("<title>Test Blog</title>"));
    }

    #[test]
    fn build_feed_has_atom_self_link() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let xml = fs::read_to_string(dist.join("feed.xml")).expect("read feed");
        assert!(xml.contains(r#"href="https://test-blog.com/feed.xml""#));
    }

    #[test]
    fn build_feed_has_items() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let xml = fs::read_to_string(dist.join("feed.xml")).expect("read feed");
        assert!(xml.contains("<title>Hello World</title>"));
        assert!(xml.contains("<title>Second Post</title>"));
    }

    #[test]
    fn build_feed_items_have_correct_links() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let xml = fs::read_to_string(dist.join("feed.xml")).expect("read feed");
        assert!(xml.contains("<link>https://test-blog.com/2026-01-01-hello.html</link>"));
    }

    // --- Edge cases ---

    #[test]
    fn build_handles_post_without_description() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        fs::write(
            blog.join("posts/2026-01-03-no-desc.md"),
            "---\ntitle: No Description Post\ndate: 2026-01-03\nlanguage: en\n---\n\nContent without description.\n",
        )
        .expect("write test post");
        do_build(&blog, &dist);
        assert!(dist.join("2026-01-03-no-desc.html").exists());
    }

    #[test]
    fn build_generates_description_from_body_when_missing() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        fs::write(
            blog.join("posts/2026-01-03-no-desc.md"),
            "---\ntitle: No Description Post\ndate: 2026-01-03\nlanguage: en\n---\n\nContent without explicit description field.\n",
        )
        .expect("write test post");
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-03-no-desc.html")).expect("read article");
        assert!(html.contains(r#"content="Content without explicit description field."#));
    }

    #[test]
    fn build_heading_after_hr_renders_as_h2() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        fs::write(
            blog.join("posts/2026-01-04-hr-heading.md"),
            "---\ntitle: HR Heading Test\ndate: 2026-01-04\n---\n\nSome text\n\n---\n\n## Section After Rule\n",
        )
        .expect("write test post");
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-04-hr-heading.html")).expect("read article");
        assert!(html.contains("<h2"));
        assert!(html.contains("Section After Rule"));
    }

    #[test]
    fn build_deduplicates_posts_with_same_title() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        fs::write(
            blog.join("posts/2026-01-05-my-post.md"),
            "---\ntitle: Duplicate Post\ndate: 2026-01-05\nlanguage: en\n---\n\nFirst version.\n",
        )
        .expect("write test post");
        fs::write(
            blog.join("posts/2026-01-05-my-post-abc.md"),
            "---\ntitle: Duplicate Post\ndate: 2026-01-05\nlanguage: en\n---\n\nSecond version.\n",
        )
        .expect("write test post");
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("index.html")).expect("read index");
        assert_eq!(html.matches("Duplicate Post").count(), 1);
    }

    // --- Minification ---

    #[test]
    fn build_inlines_css_into_html() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        assert!(!dist.join("style.css").exists());
        let html = fs::read_to_string(dist.join("index.html")).expect("read index");
        assert!(html.contains("<style>"));
    }

    #[test]
    fn build_minified_html_has_no_comments() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-01-hello.html")).expect("read article");
        assert!(!html.contains("<!--"));
    }

    #[test]
    fn build_articles_have_inlined_css() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-01-hello.html")).expect("read article");
        assert!(html.contains("<style>"));
    }

    // --- Incremental builds ---

    #[test]
    fn build_skips_unchanged_articles() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let report = do_build(&blog, &dist);
        assert!(report.skipped > 0);
    }

    #[test]
    fn build_rebuilds_changed_article() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let md = blog.join("posts/2026-01-01-hello.md");
        let content = fs::read_to_string(&md).expect("read md");
        fs::write(&md, content).expect("write md");
        let report = do_build(&blog, &dist);
        assert!(report.built > 0);
    }

    #[test]
    fn build_always_rebuilds_index() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        assert!(dist.join("index.html").exists());
    }

    // --- 404 page ---

    #[test]
    fn build_generates_404_html() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        assert!(dist.join("404.html").exists());
    }

    #[test]
    fn build_404_has_site_title() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("404.html")).expect("read 404");
        assert!(html.contains("Test Blog"));
    }

    #[test]
    fn build_404_has_back_to_home_link() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("404.html")).expect("read 404");
        assert!(html.contains("index.html"));
    }

    #[test]
    fn build_404_has_inlined_css() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("404.html")).expect("read 404");
        assert!(html.contains("<style>"));
    }

    // --- Footer ---

    #[test]
    fn build_index_footer_has_rss_link() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("index.html")).expect("read index");
        assert!(html.contains("feed.xml"));
        assert!(html.contains("rss"));
    }

    #[test]
    fn build_article_footer_has_rss_link() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-01-hello.html")).expect("read article");
        assert!(html.contains("feed.xml"));
    }

    // --- Emoji ---

    #[test]
    fn build_converts_emoji_shortcodes() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        fs::write(
            blog.join("posts/2026-01-06-emoji.md"),
            "---\ntitle: Emoji Test\ndate: 2026-01-06\ndescription: Testing emojis\n---\n\nHello :wave: world :bulb:\n",
        )
        .expect("write test post");
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-06-emoji.html")).expect("read article");
        assert!(html.contains(r#"data-emoji="wave""#));
        assert!(html.contains(r#"data-emoji="bulb""#));
    }

    // --- List preprocessing ---

    #[test]
    fn build_renders_list_items_without_blank_line() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        fs::write(
            blog.join("posts/2026-01-07-list.md"),
            "---\ntitle: List Test\ndate: 2026-01-07\n---\n\nSome text:\n* item one\n* item two\n",
        )
        .expect("write test post");
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-07-list.html")).expect("read article");
        assert!(html.contains("<li>"));
    }

    #[test]
    fn build_renders_blockquote_without_blank_line() {
        let tmp = tempdir();
        let (blog, dist) = setup_blog(&tmp);
        fs::write(
            blog.join("posts/2026-01-08-quote.md"),
            "---\ntitle: Quote Test\ndate: 2026-01-08\n---\n\nSome text\n> a quote\n",
        )
        .expect("write test post");
        do_build(&blog, &dist);
        let html = fs::read_to_string(dist.join("2026-01-08-quote.html")).expect("read article");
        assert!(html.contains("<blockquote>"));
    }
}
