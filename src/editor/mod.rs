pub mod chrome;
pub mod db;
pub mod kitty;
pub mod list;
pub mod preview;
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
        let _ = db::import_from_filesystem(&conn, &posts_dir, &cfg.date_field);
    }

    // Prepare HTML preview config (template + CSS)
    let html_config = load_html_preview_config(&cfg, &blog_dir);

    // Query terminal for font size and graphics protocol BEFORE entering raw mode.
    let picker = ratatui_image::picker::Picker::from_query_stdio()
        .unwrap_or_else(|_| ratatui_image::picker::Picker::halfblocks());

    // Chrome viewport width matches the preview pane pixel width.
    // Height doesn't matter: we capture full-page and crop client-side.
    let term_size = crossterm::terminal::size().unwrap_or((120, 40));
    let font_size = picker.font_size();
    let pane_cols = term_size.0 / 2;
    let viewport_width = pane_cols as u32 * font_size.0 as u32;
    let chrome = ChromeHandle::try_spawn(viewport_width);

    let mut terminal = ratatui::init();
    let result = cms_loop(&mut terminal, &conn, &cfg, &blog_dir, html_config.as_ref(), chrome.as_ref(), &picker);
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
    )?;
    let article_tpl = std::fs::read_to_string(&article_tpl_path).ok()?;
    let mut css = minify::compile_css(blog_dir, &theme_dir).ok()?;
    if css.is_empty() {
        let style_path = theme_dir.join("style.css");
        if style_path.exists() {
            css = std::fs::read_to_string(&style_path).ok()?;
        }
    }

    Some(HtmlPreviewConfig {
        css,
        article_tpl,
        blog_config: BlogConfig::from_file(&blog_dir.join("blog.toml")).ok()?,
    })
}

fn cms_loop(
    terminal: &mut ratatui::DefaultTerminal,
    conn: &Connection,
    cfg: &BlogConfig,
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
                edit_article(terminal, conn, cfg, blog_dir, id, html_config, chrome, picker)?;
            }
            ListAction::New => {
                new_article(terminal, conn, blog_dir, html_config, chrome, picker)?;
            }
        }
    }
}

fn edit_article(
    terminal: &mut ratatui::DefaultTerminal,
    conn: &Connection,
    cfg: &BlogConfig,
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

    let _ = db::update_content(conn, id, &final_content);

    if let Some(title) = crate::engine::config::frontmatter("title", &final_content) {
        if title != article.title {
            let _ = db::update_title(conn, id, &title);
        }
    }

    let updated = db::get_article(conn, id)
        .map_err(|e| io::Error::other(e.to_string()))?;
    if updated.status == Status::Published {
        write_published_md(&updated, cfg, blog_dir)?;
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

    let _ = db::update_content(conn, article.id, &final_content);

    if let Some(title) = crate::engine::config::frontmatter("title", &final_content) {
        let _ = db::update_title(conn, article.id, &title);
    }

    let _ = std::fs::remove_file(&tmp_file);
    Ok(())
}

fn write_published_md(article: &db::Article, cfg: &BlogConfig, blog_dir: &Path) -> io::Result<()> {
    let posts_dir = blog_dir.join("posts");
    std::fs::create_dir_all(&posts_dir)?;
    let md = db::build_markdown(article, &cfg.date_field);
    let path = posts_dir.join(format!("{}.md", article.slug));
    std::fs::write(path, md)
}
