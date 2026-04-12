pub mod chrome;
pub mod db;
pub mod kitty;
pub mod list;
pub mod preview;
pub mod tmux;
pub mod vim;

use std::io;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::engine::config::BlogConfig;
use crate::engine::{minify, template};
use chrome::ChromeHandle;
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

    // Prepare HTML preview config (template + CSS)
    let html_config = load_html_preview_config(&cfg, &blog_dir);
    if html_config.is_none() {
        log::warn!("HTML preview config failed to load — ^P will be disabled");
    }

    // Query terminal for font size and graphics protocol BEFORE entering raw mode.
    let picker = ratatui_image::picker::Picker::from_query_stdio()
        .unwrap_or_else(|e| {
            log::warn!("Picker query failed ({e}), falling back to halfblocks");
            ratatui_image::picker::Picker::halfblocks()
        });

    // Chrome viewport in CSS pixels, screenshot at device scale for crisp Retina rendering.
    // Retina displays report ~16-20 physical px/cell; non-Retina ~7-10. Threshold at 12.
    let term_size = crossterm::terminal::size().unwrap_or((120, 40));
    let font_size = picker.font_size();
    let pane_cols = term_size.0 / 2;
    let physical_width = pane_cols as u32 * font_size.0 as u32;
    let scale: f64 = if font_size.0 > 12 { 2.0 } else { 1.0 };
    let viewport_css = (physical_width as f64 / scale) as u32;
    log::info!(
        "Terminal: {:?}, font_size: {:?}, pane_cols: {}, physical_width: {}, scale: {}, viewport_css: {}",
        term_size, font_size, pane_cols, physical_width, scale, viewport_css
    );
    let chrome = ChromeHandle::try_spawn(viewport_css, scale);
    match &chrome {
        Some(_) => log::info!("Chrome spawned (viewport={}px, scale={})", viewport_css, scale),
        None => log::warn!("Chrome not available — HTML preview disabled"),
    }

    let mut terminal = ratatui::init();
    let result = cms_loop(&mut terminal, &conn, &blog_dir, html_config.as_ref(), chrome.as_ref(), &picker);
    ratatui::restore();
    result
}

/// Legacy entry point: run editor for a single file (no CMS).
pub fn run(file_path: PathBuf) -> io::Result<()> {
    let picker = ratatui_image::picker::Picker::from_query_stdio()
        .unwrap_or_else(|_| ratatui_image::picker::Picker::halfblocks());
    let mut terminal = ratatui::init();
    let result = vim::run(&mut terminal, file_path, None, None, &picker);
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
    })
}

fn cms_loop(
    terminal: &mut ratatui::DefaultTerminal,
    conn: &Connection,
    blog_dir: &Path,
    html_config: Option<&HtmlPreviewConfig>,
    chrome: Option<&ChromeHandle>,
    picker: &ratatui_image::picker::Picker,
) -> io::Result<()> {
    loop {
        let articles = db::list_articles(conn, None)
            .map_err(|e| io::Error::other(e.to_string()))?;

        let mut list_view = ListView::new(articles);
        let action = list_view.run(terminal, conn)?;

        match action {
            ListAction::Quit => return Ok(()),
            ListAction::Edit(id) => {
                edit_article(terminal, conn, blog_dir, id, html_config, chrome, picker)?;
            }
            ListAction::New => {
                new_article(terminal, conn, blog_dir, html_config, chrome, picker)?;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn edit_article(
    terminal: &mut ratatui::DefaultTerminal,
    conn: &Connection,
    blog_dir: &Path,
    id: i64,
    html_config: Option<&HtmlPreviewConfig>,
    chrome: Option<&ChromeHandle>,
    picker: &ratatui_image::picker::Picker,
) -> io::Result<()> {
    let article = db::get_article(conn, id)
        .map_err(|e| io::Error::other(e.to_string()))?;

    let tmp_dir = std::env::temp_dir().join("devtui-cms");
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp_file = tmp_dir.join(format!("{}.md", article.slug));
    std::fs::write(&tmp_file, &article.content)?;

    let (_result, final_content) = vim::run(terminal, tmp_file.clone(), html_config, chrome, picker)?;

    // Only update DB if content actually changed (protects against :q! or crash)
    if !final_content.is_empty() && final_content != article.content {
        let _ = db::update_content(conn, id, &final_content);

        if let Some(title) = crate::engine::config::frontmatter("title", &final_content) {
            if title != article.title {
                let _ = db::update_title(conn, id, &title);
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
    chrome: Option<&ChromeHandle>,
    picker: &ratatui_image::picker::Picker,
) -> io::Result<()> {
    let article = db::create_article(conn, "Untitled")
        .map_err(|e| io::Error::other(e.to_string()))?;

    let tmp_dir = std::env::temp_dir().join("devtui-cms");
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp_file = tmp_dir.join(format!("{}.md", article.slug));
    std::fs::write(&tmp_file, "")?;

    let (_result, final_content) = vim::run(terminal, tmp_file.clone(), html_config, chrome, picker)?;

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
        assert_eq!(contents, md);
    }
}
