use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::config::{self, BlogConfig};
use super::links;
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

    let mut posts = collect_posts(&posts_dir, &cfg.date_field)?;

    let (rebuilt_articles, sitemap_entries, skipped) = render_articles(
        &cfg, &posts, &articles_dir, articles_prefix, blog_dir, &theme_dir, engine_dir,
    )?;

    let css = compile_css(blog_dir, &theme_dir)?;
    fs::write(dist_dir.join("style.css"), &css).map_err(|e| e.to_string())?;

    copy_static_assets(blog_dir, dist_dir)?;

    posts.sort_by(|a, b| b.date.cmp(&a.date));

    build_index(&cfg, dist_dir, &posts, articles_prefix, blog_dir, &theme_dir, engine_dir)?;
    generate_sitemap(&cfg, dist_dir, &sitemap_entries)?;
    generate_feed(&cfg, dist_dir, &posts, articles_prefix)?;
    generate_seo_files(&cfg, dist_dir, blog_dir, &theme_dir, engine_dir)?;
    inject_analytics(&cfg, &rebuilt_articles)?;
    minify_and_inline(dist_dir, &css, &rebuilt_articles)?;

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
            let body = config::post_body(&post.content);
            let html_body = markdown::markdown_to_html(&body);

            let description = if post.description.is_empty() {
                markdown::post_snippet(&body, None, 160)
            } else {
                post.description.to_string()
            };

            let base_path = if articles_prefix.is_empty() { "" } else { "../" };
            let slug_with_prefix = if articles_prefix.is_empty() {
                &post.slug
            } else {
                &format!("{}/{}", articles_prefix, post.slug)
            };

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
            ]);

            let rendered = template::template_render(&article_tpl, &vars);
            fs::write(&html_out, &rendered).map_err(|e| e.to_string())?;
            rebuilt_articles.push(html_out.to_string_lossy().to_string());
        }

        let prefix_segment = if articles_prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", articles_prefix)
        };
        let article_url = format!("{}/{}{}.html", cfg.url, prefix_segment, post.slug);
        sitemap_entries.push_str(&seo::sitemap_entry(&article_url, &post.date));
        sitemap_entries.push('\n');
    }

    Ok((rebuilt_articles, sitemap_entries, skipped))
}

// --- CSS compilation ---

fn compile_css(blog_dir: &Path, theme_dir: &Path) -> Result<String, String> {
    let style_dir = if blog_dir.join("base.css").exists() {
        blog_dir.to_path_buf()
    } else {
        theme_dir.to_path_buf()
    };
    let mut css = String::new();
    for part in &["base", "index", "article", "syntax", "responsive"] {
        let css_file = style_dir.join(format!("{part}.css"));
        if css_file.exists() {
            css.push_str(&fs::read_to_string(&css_file).map_err(|e| e.to_string())?);
        }
    }
    Ok(css)
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

// --- Index page ---

fn build_index(
    cfg: &BlogConfig,
    dist_dir: &Path,
    posts: &[Post],
    articles_prefix: &str,
    blog_dir: &Path,
    theme_dir: &Path,
    engine_dir: &Path,
) -> Result<(), String> {
    let index_tpl_path = template::resolve_file(
        "index_header.html",
        &blog_dir.join("templates"),
        &theme_dir.join("templates"),
        &engine_dir.join("templates"),
    )
    .ok_or("index_header.html template not found")?;
    let index_tpl = fs::read_to_string(&index_tpl_path).map_err(|e| e.to_string())?;

    let subtitle = cfg.subtitle.as_deref().unwrap_or("");
    let index_vars = HashMap::from([
        ("title", cfg.title.as_str()),
        ("subtitle", subtitle),
        ("url", cfg.url.as_str()),
        ("author", cfg.author.as_str()),
        ("lang", cfg.lang.as_str()),
    ]);
    let mut index_html = template::template_render(&index_tpl, &index_vars);

    inject_nav(cfg, &mut index_html);
    inject_post_list(posts, articles_prefix, &mut index_html);
    inject_footer(cfg, &mut index_html);

    fs::write(dist_dir.join("index.html"), &index_html).map_err(|e| e.to_string())
}

fn inject_nav(cfg: &BlogConfig, index_html: &mut String) {
    let links_html = links::render_links(cfg);
    let guides_html = links::render_guides(cfg);
    let nav_html = match (links_html.is_empty(), guides_html.is_empty()) {
        (false, false) => format!("{links_html} {guides_html}"),
        (false, true) => links_html,
        (true, false) => guides_html.to_string(),
        _ => String::new(),
    };
    if !nav_html.is_empty() {
        *index_html = index_html.replace(r#"<div class="guides-slot"></div>"#, &nav_html);
    }

    let tags_html = links::render_tags(cfg);
    if !tags_html.is_empty() {
        *index_html = index_html.replace(
            r#"<div class="tag-filter"></div>"#,
            &format!(r#"<div class="tag-filter">{tags_html}</div>"#),
        );
        *index_html = index_html.replace(
            r#"<div class="mobile-pills mobile-tags"></div>"#,
            &format!(r#"<div class="mobile-pills mobile-tags">{tags_html}</div>"#),
        );
    }
    if !guides_html.is_empty() {
        *index_html = index_html.replace(
            r#"<div class="mobile-pills mobile-guides"></div>"#,
            &format!(r#"<div class="mobile-pills mobile-guides">{guides_html}</div>"#),
        );
    }
}

fn inject_post_list(posts: &[Post], articles_prefix: &str, index_html: &mut String) {
    let mut seen_titles = HashSet::new();
    for post in posts {
        if !seen_titles.insert(&post.title) {
            continue;
        }
        let snippet = markdown::post_snippet(
            &config::post_body(&post.content),
            config::frontmatter("description", &post.content).as_deref(),
            300,
        );
        let lang = config::frontmatter("language", &post.content)
            .map(|l| match l.as_str() {
                "pt-BR" | "pt-br" | "pt" => "pt".to_string(),
                other => other.to_string(),
            })
            .unwrap_or_default();
        let tags = extract_tags(&post.content);
        let slug_href = if articles_prefix.is_empty() {
            format!("{}.html", post.slug)
        } else {
            format!("{}/{}.html", articles_prefix, post.slug)
        };
        index_html.push_str(&format!(
            r#"<li data-lang="{lang}" data-tags="{tags}"><time datetime="{date}">{date}</time><a href="{slug_href}">{title}</a><p class="post-desc">{snippet}</p></li>"#,
            date = post.date,
            title = post.title,
        ));
        index_html.push('\n');
    }
}

fn inject_footer(cfg: &BlogConfig, index_html: &mut String) {
    let license = cfg.license.as_deref().unwrap_or("");
    let license_url = cfg.license_url.as_deref().unwrap_or("");
    let mut footer_parts = String::new();
    if !license.is_empty() && !license_url.is_empty() {
        footer_parts = format!(r#"<a href="{license_url}" target="_blank" rel="noopener">{license}</a>"#);
    } else if !license.is_empty() {
        footer_parts = license.to_string();
    }
    footer_parts.push_str(r#" · <a href="feed.xml">rss</a>"#);

    let analytics_id = cfg.analytics_id.as_deref().unwrap_or("");
    let ga_script = ga_script_tag(analytics_id);

    index_html.push_str(&format!(
        "</ul></main>\n{}\n{ga_script}\n<footer>{footer_parts}</footer>\n</body></html>",
        FILTER_SCRIPT,
    ));
}

// --- Feeds ---

fn generate_sitemap(cfg: &BlogConfig, dist_dir: &Path, sitemap_entries: &str) -> Result<(), String> {
    let sitemap = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n<url><loc>{}/</loc></url>\n{}</urlset>",
        cfg.url, sitemap_entries
    );
    fs::write(dist_dir.join("sitemap.xml"), &sitemap).map_err(|e| e.to_string())
}

fn generate_feed(cfg: &BlogConfig, dist_dir: &Path, posts: &[Post], articles_prefix: &str) -> Result<(), String> {
    let subtitle = cfg.subtitle.as_deref().unwrap_or("");
    let mut feed = seo::rss_header(&cfg.title, &cfg.url, subtitle);
    for post in posts.iter().take(20) {
        let body = config::post_body(&post.content);
        let html_body = markdown::markdown_to_html(&body);
        let prefix_segment = if articles_prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", articles_prefix)
        };
        let article_url = format!("{}/{}{}.html", cfg.url, prefix_segment, post.slug);
        feed.push_str(&seo::rss_item(&post.title, &article_url, &html_body, &post.date));
    }
    feed.push_str("</channel></rss>\n");
    fs::write(dist_dir.join("feed.xml"), &feed).map_err(|e| e.to_string())
}

// --- SEO files ---

fn generate_seo_files(
    cfg: &BlogConfig,
    dist_dir: &Path,
    blog_dir: &Path,
    theme_dir: &Path,
    engine_dir: &Path,
) -> Result<(), String> {
    fs::write(dist_dir.join("robots.txt"), seo::robots_txt(&cfg.url)).map_err(|e| e.to_string())?;

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

// --- Analytics injection ---

fn inject_analytics(cfg: &BlogConfig, rebuilt_articles: &[String]) -> Result<(), String> {
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

// --- Minification ---

fn minify_and_inline(dist_dir: &Path, css: &str, rebuilt_articles: &[String]) -> Result<(), String> {
    let minified_css = minify::minify_css(css);
    let mut process_files = vec![dist_dir.join("index.html")];
    for path in rebuilt_articles {
        process_files.push(PathBuf::from(path));
    }
    process_files.push(dist_dir.join("404.html"));

    for html_path in &process_files {
        if html_path.exists() {
            let content = fs::read_to_string(html_path).map_err(|e| e.to_string())?;
            let content = minify::inline_css(&content, &minified_css);
            let content = minify::minify_html(&content);
            fs::write(html_path, content).map_err(|e| e.to_string())?;
        }
    }

    let _ = fs::remove_file(dist_dir.join("style.css"));
    Ok(())
}

// --- Helpers ---

struct Post {
    slug: String,
    title: String,
    date: String,
    description: String,
    content: String,
    path: PathBuf,
}

fn collect_posts(posts_dir: &Path, date_field: &str) -> Result<Vec<Post>, String> {
    let mut posts = Vec::new();
    if !posts_dir.exists() {
        return Ok(posts);
    }

    let mut entries: Vec<_> = fs::read_dir(posts_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let slug = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let title = config::frontmatter("title", &content).unwrap_or_default();
        let date = config::frontmatter_date(date_field, &content).unwrap_or_default();
        let description = config::frontmatter("description", &content).unwrap_or_default();

        posts.push(Post { slug, title, date, description, content, path });
    }

    Ok(posts)
}

fn extract_tags(content: &str) -> String {
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("tags:") {
            return rest
                .trim()
                .trim_matches(|c| c == '[' || c == ']')
                .replace('"', "")
                .split(',')
                .map(|t| t.trim())
                .collect::<Vec<_>>()
                .join(" ");
        }
    }
    String::new()
}

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

fn ga_script_tag(analytics_id: &str) -> String {
    if analytics_id.is_empty() {
        return String::new();
    }
    format!("<script>window.addEventListener('load',function(){{setTimeout(function(){{var s=document.createElement('script');s.src='https://www.googletagmanager.com/gtag/js?id={analytics_id}';s.async=true;document.head.appendChild(s);s.onload=function(){{window.dataLayer=window.dataLayer||[];function g(){{dataLayer.push(arguments)}}g('js',new Date());g('config','{analytics_id}')}}}},2000)}})</script>")
}

/// JavaScript for tag/lang filtering and search on the index page.
/// Dropped during the shell-to-Rust port, restored here.
const FILTER_SCRIPT: &str = r#"<script>
var activeLang='all',activeTag='all';
function bindFilter(sel,cls,cb){
  document.querySelectorAll(sel).forEach(function(el){
    el.onclick=function(e){
      if(!e.target.classList.contains(cls))return;
      var prev=el.querySelector('.'+cls+'.active');
      if(prev)prev.classList.remove('active');
      e.target.classList.add('active');
      cb(e.target.dataset);
      filterPosts();
    };
  });
}
bindFilter('.lang-filter,.mobile-menu .mobile-pills','lang-btn',function(d){activeLang=d.lang});
bindFilter('.tag-filter,.mobile-tags','tag-btn',function(d){activeTag=d.tag});
function filterPosts(){
  var q=document.querySelector('.search-input').value.toLowerCase();
  document.querySelectorAll('.post-list li').forEach(function(li){
    var matchLang=activeLang==='all'||li.dataset.lang===activeLang;
    var matchTag=activeTag==='all'||(' '+li.dataset.tags+' ').indexOf(' '+activeTag+' ')!==-1;
    var matchSearch=!q||li.textContent.toLowerCase().indexOf(q)!==-1;
    li.style.display=matchLang&&matchTag&&matchSearch?'':'none';
  });
}
</script>"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("devtui-build-{}-{}", std::process::id(), id));
        fs::create_dir_all(&dir).expect("failed to create test temp dir");
        dir
    }

    fn engine_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("engine")
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
