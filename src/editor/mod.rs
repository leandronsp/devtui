pub mod chat;
pub mod db;
pub mod list;
pub mod ops;
pub mod preview;
pub mod upload;
pub mod vim;

use std::io;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::engine::config::BlogConfig;
use crate::engine::{minify, template};
use db::Status;
use list::{ListAction, ListView};
use vim::HtmlPreviewConfig;

/// Run the CMS with article list and editor views.
pub fn run_cms(blog_dir: PathBuf) -> io::Result<()> {
    let config_path = blog_dir.join("blog.toml");
    let cfg = BlogConfig::from_file(&config_path)
        .map_err(io::Error::other)?;

    let db_path = blog_dir.join("devtui.db");
    let conn = db::init_db(&db_path)
        .map_err(|e| io::Error::other(e.to_string()))?;

    let posts_dir = blog_dir.join("posts");
    if posts_dir.exists() {
        let _ = db::import_if_empty(&conn, &posts_dir);
    }

    db::migrate_content_to_full_markdown(&conn)
        .map_err(|e| io::Error::other(e.to_string()))?;

    let html_config = load_html_preview_config(&cfg, &blog_dir);

    let mut terminal = ratatui::init();
    let result = cms_loop(&mut terminal, &conn, &blog_dir, &cfg, html_config.as_ref());
    ratatui::restore();
    result
}

/// Legacy entry point: run editor for a single file (no CMS).
pub fn run(file_path: PathBuf) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = vim::run(&mut terminal, file_path, None);
    ratatui::restore();
    result.map(|_| ())
}

fn load_html_preview_config(cfg: &BlogConfig, blog_dir: &Path) -> Option<HtmlPreviewConfig> {
    let engine_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/engine");
    let theme = cfg.theme.as_deref().unwrap_or("paper");
    let theme_dir = engine_dir.join("themes").join(theme);

    let article_tpl_path = template::resolve_file(
        "article.html",
        &blog_dir.join("templates"),
        &theme_dir.join("templates"),
        &engine_dir.join("templates"),
    );
    let Some(article_tpl_path) = article_tpl_path else {
        log::warn!("Article template not found in blog, theme, or engine dirs");
        return None;
    };

    let article_tpl = match std::fs::read_to_string(&article_tpl_path) {
        Ok(tpl) => tpl,
        Err(e) => {
            log::warn!("Failed to read article template {:?}: {}", article_tpl_path, e);
            return None;
        }
    };

    let mut css = match minify::compile_css(blog_dir, &theme_dir) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("CSS compile failed: {}", e);
            return None;
        }
    };
    if css.is_empty() {
        let style_path = theme_dir.join("style.css");
        if style_path.exists() {
            css = match std::fs::read_to_string(&style_path) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("Failed to read style.css: {}", e);
                    return None;
                }
            };
        }
    }

    log::info!("HTML preview config loaded (theme={}, css={}B, template={}B)", theme, css.len(), article_tpl.len());
    Some(HtmlPreviewConfig {
        css,
        article_tpl,
        blog_config: cfg.clone(),
        blog_dir: blog_dir.to_path_buf(),
    })
}

fn cms_loop(
    terminal: &mut ratatui::DefaultTerminal,
    conn: &Connection,
    blog_dir: &Path,
    config: &BlogConfig,
    html_config: Option<&HtmlPreviewConfig>,
) -> io::Result<()> {
    let articles = db::list_articles(conn, None)
        .map_err(|e| io::Error::other(e.to_string()))?;

    let mut list_view = ListView::new(blog_dir.to_path_buf(), articles, config);

    loop {
        let action = list_view.run(terminal, conn)?;

        match action {
            ListAction::Quit => {
                return Ok(());
            }
            ListAction::Edit(id) => {
                edit_article(terminal, conn, blog_dir, id, html_config)?;
                list_view.refresh(conn);
            }
            ListAction::New => {
                new_article(terminal, conn, blog_dir, html_config)?;
                list_view.refresh(conn);
            }
        }
    }
}

fn edit_article(
    terminal: &mut ratatui::DefaultTerminal,
    conn: &Connection,
    blog_dir: &Path,
    id: i64,
    html_config: Option<&HtmlPreviewConfig>,
) -> io::Result<()> {
    let article = db::get_article(conn, id)
        .map_err(|e| io::Error::other(e.to_string()))?;

    let tmp_dir = std::env::temp_dir().join("devtui-cms");
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp_file = tmp_dir.join(format!("{}.md", article.slug));
    std::fs::write(&tmp_file, &article.content)?;

    let (_result, final_content) = vim::run(terminal, tmp_file.clone(), html_config)?;

    // Only update DB if content actually changed (protects against :q! or crash)
    if !final_content.is_empty() && final_content != article.content {
        let _ = db::update_content(conn, id, &final_content);

        if let Some(title) = crate::engine::config::frontmatter("title", &final_content) {
            if title != article.title {
                let _ = db::update_title(conn, id, &title);
                rename_auto_slug_from_title(conn, &article, &title, blog_dir);
            }
        }

        let updated = db::get_article(conn, id)
            .map_err(|e| io::Error::other(e.to_string()))?;
        if updated.status == Status::Published {
            write_published_md(&updated, blog_dir)?;
        }
    }

    let _ = std::fs::remove_file(&tmp_file);
    Ok(())
}

fn new_article(
    terminal: &mut ratatui::DefaultTerminal,
    conn: &Connection,
    _blog_dir: &Path,
    html_config: Option<&HtmlPreviewConfig>,
) -> io::Result<()> {
    let article = db::create_article(conn, "Untitled")
        .map_err(|e| io::Error::other(e.to_string()))?;

    let tmp_dir = std::env::temp_dir().join("devtui-cms");
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp_file = tmp_dir.join(format!("{}.md", article.slug));
    let default_frontmatter = "---\ntitle: Untitled\nstatus: draft\n---\n\n";
    std::fs::write(&tmp_file, default_frontmatter)?;

    let (_result, final_content) = vim::run(terminal, tmp_file.clone(), html_config)?;

    if !final_content.is_empty() {
        let _ = db::update_content(conn, article.id, &final_content);

        if let Some(title) = crate::engine::config::frontmatter("title", &final_content) {
            let _ = db::update_title(conn, article.id, &title);
        }
    }

    let _ = std::fs::remove_file(&tmp_file);
    Ok(())
}

fn write_published_md(article: &db::Article, blog_dir: &Path) -> io::Result<()> {
    let posts_dir = blog_dir.join("posts");
    std::fs::create_dir_all(&posts_dir)?;
    let path = posts_dir.join(format!("{}.md", article.slug));
    std::fs::write(path, &article.content)
}

/// When the user gives the article a real title, replace the auto-generated
/// `untitled-N` slug with one derived from the title. Untitled / empty titles
/// leave the slug alone. User-set slugs are never rewritten so existing URLs
/// stay stable.
fn rename_auto_slug_from_title(
    conn: &Connection,
    article: &db::Article,
    new_title: &str,
    blog_dir: &Path,
) {
    if !db::is_auto_slug(&article.slug) {
        return;
    }
    let trimmed = new_title.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("untitled") {
        return;
    }

    let Ok(new_slug) = db::derive_unique_slug(conn, trimmed) else { return };
    if new_slug.is_empty() || db::is_auto_slug(&new_slug) || new_slug == article.slug {
        return;
    }

    if db::update_slug(conn, article.id, &new_slug).is_ok() {
        // Drop the orphan .md left over from a previous publish under the old
        // slug, otherwise the engine would render two posts.
        let old_md = blog_dir.join("posts").join(format!("{}.md", article.slug));
        let _ = std::fs::remove_file(&old_md);
    }
}

/// Force re-import `.md` files into the DB. Unlike `import_if_empty`, this
/// runs unconditionally; existing rows are updated in place. Intended for the
/// `make cms.import.<blog>` target, used when `.md` files are edited outside
/// the CMS and the user wants to pull changes in. Draft/pin state on
/// unaffected articles is preserved.
pub fn import_blog(blog_dir: &Path) -> io::Result<()> {
    BlogConfig::from_file(&blog_dir.join("blog.toml"))
        .map_err(io::Error::other)?;
    let conn = db::init_db(&blog_dir.join("devtui.db"))
        .map_err(|e| io::Error::other(e.to_string()))?;
    let posts_dir = blog_dir.join("posts");
    db::import_from_filesystem(&conn, &posts_dir)
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

/// Sync DB → `.md` for a DevTUI-managed blog. No-op when `devtui.db` is absent
/// (the blog is not managed by DevTUI and the engine already reads `.md`
/// directly). Intended to run once before `engine::build` at the binary entry
/// point, keeping the engine unaware of the DB.
pub fn sync_managed_blog(blog_dir: &Path) -> io::Result<()> {
    let db_path = blog_dir.join("devtui.db");
    if !db_path.exists() {
        return Ok(());
    }
    let conn = db::init_db(&db_path).map_err(|e| io::Error::other(e.to_string()))?;
    let posts_dir = blog_dir.join("posts");
    db::sync_to_filesystem(&conn, &posts_dir)
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::tempdir;

    fn write_blog_toml(blog_dir: &Path) {
        std::fs::write(
            blog_dir.join("blog.toml"),
            "title = \"Test\"\nurl = \"https://t.example\"\nauthor = \"A\"\nlang = \"en\"\n",
        )
        .unwrap();
    }

    #[test]
    fn sync_managed_blog_is_noop_when_db_missing() {
        // No devtui.db, no blog.toml. sync_managed_blog must early-return Ok
        // without touching the filesystem or reading config.
        let blog_dir = tempdir();
        sync_managed_blog(&blog_dir).unwrap();
        assert!(!blog_dir.join("posts").exists());
    }

    #[test]
    fn import_blog_populates_db_from_md_files() {
        let blog_dir = tempdir();
        write_blog_toml(&blog_dir);
        let posts_dir = blog_dir.join("posts");
        std::fs::create_dir_all(&posts_dir).unwrap();
        std::fs::write(
            posts_dir.join("hello.md"),
            "---\ntitle: Hello\ndate: 2026-04-01\n---\n\nBody.\n",
        )
        .unwrap();

        import_blog(&blog_dir).unwrap();

        let conn = db::init_db(&blog_dir.join("devtui.db")).unwrap();
        let articles = db::list_articles(&conn, None).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "Hello");
    }

    #[test]
    fn rename_auto_slug_replaces_untitled_with_derived_slug() {
        let blog_dir = tempdir();
        let conn = db::init_db(&blog_dir.join("devtui.db")).unwrap();
        let article = db::create_article(&conn, "Untitled").unwrap();
        // Sanity: starts with an auto slug.
        assert!(article.slug.starts_with("untitled"));

        rename_auto_slug_from_title(&conn, &article, "Hello World", &blog_dir);

        let after = db::get_article(&conn, article.id).unwrap();
        assert_eq!(after.slug, "hello-world");
    }

    #[test]
    fn rename_auto_slug_keeps_user_set_slug_intact() {
        let blog_dir = tempdir();
        let conn = db::init_db(&blog_dir.join("devtui.db")).unwrap();
        let article = db::create_article(&conn, "Untitled").unwrap();
        db::update_slug(&conn, article.id, "my-stable-url").unwrap();
        let stable = db::get_article(&conn, article.id).unwrap();

        rename_auto_slug_from_title(&conn, &stable, "Different Title", &blog_dir);

        let after = db::get_article(&conn, article.id).unwrap();
        assert_eq!(after.slug, "my-stable-url");
    }

    #[test]
    fn rename_auto_slug_skips_when_new_title_is_untitled() {
        let blog_dir = tempdir();
        let conn = db::init_db(&blog_dir.join("devtui.db")).unwrap();
        let article = db::create_article(&conn, "Untitled").unwrap();
        let original_slug = article.slug.clone();

        rename_auto_slug_from_title(&conn, &article, "  untitled  ", &blog_dir);
        rename_auto_slug_from_title(&conn, &article, "", &blog_dir);

        let after = db::get_article(&conn, article.id).unwrap();
        assert_eq!(after.slug, original_slug);
    }

    #[test]
    fn rename_auto_slug_removes_orphan_md_at_old_slug() {
        let blog_dir = tempdir();
        let conn = db::init_db(&blog_dir.join("devtui.db")).unwrap();
        let article = db::create_article(&conn, "Untitled").unwrap();
        let posts = blog_dir.join("posts");
        std::fs::create_dir_all(&posts).unwrap();
        let old_md = posts.join(format!("{}.md", article.slug));
        std::fs::write(&old_md, "stale content").unwrap();

        rename_auto_slug_from_title(&conn, &article, "Brand New", &blog_dir);

        assert!(!old_md.exists(), "old .md should be cleaned up");
    }

    #[test]
    fn sync_managed_blog_writes_published_article() {
        let blog_dir = tempdir();
        write_blog_toml(&blog_dir);
        let db_path = blog_dir.join("devtui.db");
        let conn = db::init_db(&db_path).unwrap();
        let article = db::create_article(&conn, "Sync Me").unwrap();
        let md = "---\ntitle: Sync Me\n---\n\nPublished body.";
        db::update_content(&conn, article.id, md).unwrap();
        db::publish(&conn, article.id).unwrap();
        drop(conn);

        sync_managed_blog(&blog_dir).unwrap();

        let md_path = blog_dir.join("posts/sync-me.md");
        assert!(md_path.exists());
        let contents = std::fs::read_to_string(&md_path).unwrap();
        assert!(contents.contains("title: Sync Me"));
        assert!(contents.contains("status: published"));
        assert!(contents.contains("Published body."));
    }
}
