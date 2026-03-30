pub mod db;
pub mod list;
pub mod preview;
pub mod vim;

use std::io;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::engine::config::BlogConfig;
use db::Status;
use list::{ListAction, ListView};

/// Run the CMS with article list and editor views.
/// blog_dir is the path to a blog directory (e.g. blogs/acme-alchemist).
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

    let mut terminal = ratatui::init();
    let result = cms_loop(&mut terminal, &conn, &cfg, &blog_dir);
    ratatui::restore();
    result
}

/// Legacy entry point: run editor for a single file (no CMS).
pub fn run(file_path: PathBuf) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = vim::run(&mut terminal, file_path);
    ratatui::restore();
    result.map(|_| ())
}

fn cms_loop(
    terminal: &mut ratatui::DefaultTerminal,
    conn: &Connection,
    cfg: &BlogConfig,
    blog_dir: &Path,
) -> io::Result<()> {
    loop {
        let articles = db::list_articles(conn, None)
            .map_err(|e| io::Error::other(e.to_string()))?;

        let mut list_view = ListView::new(articles);
        let action = list_view.run(terminal, conn)?;

        match action {
            ListAction::Quit => return Ok(()),
            ListAction::Edit(id) => {
                edit_article(terminal, conn, cfg, blog_dir, id)?;
            }
            ListAction::New => {
                new_article(terminal, conn, cfg, blog_dir)?;
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
) -> io::Result<()> {
    let article = db::get_article(conn, id)
        .map_err(|e| io::Error::other(e.to_string()))?;

    let tmp_dir = std::env::temp_dir().join("devtui-cms");
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp_file = tmp_dir.join(format!("{}.md", article.slug));
    std::fs::write(&tmp_file, &article.content)?;

    let (_result, final_content) = vim::run(terminal, tmp_file.clone())?;

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
    _cfg: &BlogConfig,
    _blog_dir: &Path,
) -> io::Result<()> {
    let article = db::create_article(conn, "Untitled")
        .map_err(|e| io::Error::other(e.to_string()))?;

    let tmp_dir = std::env::temp_dir().join("devtui-cms");
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp_file = tmp_dir.join(format!("{}.md", article.slug));
    std::fs::write(&tmp_file, "")?;

    let (_result, final_content) = vim::run(terminal, tmp_file.clone())?;

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
