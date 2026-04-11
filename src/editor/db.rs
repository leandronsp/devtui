use std::fmt;
use std::path::Path;

use rusqlite::{params, Connection};

use crate::engine::config;

// --- Types ---

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Draft,
    Published,
}

impl Status {
    fn as_str(&self) -> &'static str {
        match self {
            Status::Draft => "draft",
            Status::Published => "published",
        }
    }

    fn from_str(s: &str) -> Result<Self, CmsError> {
        match s {
            "draft" => Ok(Status::Draft),
            "published" => Ok(Status::Published),
            other => Err(CmsError::InvalidStatus(other.to_string())),
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Article {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub status: Status,
    pub language: String,
    pub pinned: bool,
    pub tags: Vec<String>,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug)]
pub enum CmsError {
    Database(rusqlite::Error),
    Filesystem(std::io::Error),
    ArticleNotFound(i64),
    SlugConflict(String),
    InvalidStatus(String),
}

impl fmt::Display for CmsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CmsError::Database(e) => write!(f, "database error: {e}"),
            CmsError::Filesystem(e) => write!(f, "filesystem error: {e}"),
            CmsError::ArticleNotFound(id) => write!(f, "article not found: {id}"),
            CmsError::SlugConflict(slug) => write!(f, "slug already exists: {slug}"),
            CmsError::InvalidStatus(s) => write!(f, "invalid status: {s}"),
        }
    }
}

impl std::error::Error for CmsError {}

impl From<rusqlite::Error> for CmsError {
    fn from(e: rusqlite::Error) -> Self {
        CmsError::Database(e)
    }
}

impl From<std::io::Error> for CmsError {
    fn from(e: std::io::Error) -> Self {
        CmsError::Filesystem(e)
    }
}

// --- Database operations ---

fn create_schema(conn: &Connection) -> Result<(), CmsError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS articles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            content TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'draft',
            language TEXT NOT NULL DEFAULT 'en',
            pinned INTEGER NOT NULL DEFAULT 0,
            tags TEXT NOT NULL DEFAULT '',
            published_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;
    Ok(())
}

pub fn init_db(path: &Path) -> Result<Connection, CmsError> {
    let conn = Connection::open(path)?;
    create_schema(&conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn init_db_memory() -> Result<Connection, CmsError> {
    let conn = Connection::open_in_memory()?;
    create_schema(&conn)?;
    Ok(conn)
}

pub fn create_article(conn: &Connection, title: &str) -> Result<Article, CmsError> {
    let base_slug = slugify(title);
    let slug = unique_slug(conn, &base_slug)?;

    conn.execute(
        "INSERT INTO articles (title, slug) VALUES (?1, ?2)",
        params![title, slug],
    )?;
    let id = conn.last_insert_rowid();
    get_article(conn, id)
}

pub fn get_article(conn: &Connection, id: i64) -> Result<Article, CmsError> {
    conn.query_row(
        "SELECT id, title, slug, content, status, language, pinned, tags, published_at, created_at, updated_at
         FROM articles WHERE id = ?1",
        params![id],
        row_to_article,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CmsError::ArticleNotFound(id),
        other => CmsError::Database(other),
    })
}

pub fn delete_article(conn: &Connection, id: i64) -> Result<(), CmsError> {
    let changed = conn.execute("DELETE FROM articles WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(CmsError::ArticleNotFound(id));
    }
    Ok(())
}

pub fn update_content(conn: &Connection, id: i64, content: &str) -> Result<(), CmsError> {
    let changed = conn.execute(
        "UPDATE articles SET content = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![content, id],
    )?;
    if changed == 0 {
        return Err(CmsError::ArticleNotFound(id));
    }
    Ok(())
}

pub fn update_title(conn: &Connection, id: i64, title: &str) -> Result<(), CmsError> {
    let changed = conn.execute(
        "UPDATE articles SET title = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![title, id],
    )?;
    if changed == 0 {
        return Err(CmsError::ArticleNotFound(id));
    }
    Ok(())
}

pub fn publish(conn: &Connection, id: i64) -> Result<(), CmsError> {
    let changed = conn.execute(
        "UPDATE articles
         SET status = 'published',
             published_at = COALESCE(published_at, datetime('now')),
             updated_at = datetime('now')
         WHERE id = ?1",
        params![id],
    )?;
    if changed == 0 {
        return Err(CmsError::ArticleNotFound(id));
    }
    Ok(())
}

pub fn unpublish(conn: &Connection, id: i64) -> Result<(), CmsError> {
    let changed = conn.execute(
        "UPDATE articles SET status = 'draft', updated_at = datetime('now') WHERE id = ?1",
        params![id],
    )?;
    if changed == 0 {
        return Err(CmsError::ArticleNotFound(id));
    }
    Ok(())
}

pub fn pin(conn: &Connection, id: i64) -> Result<(), CmsError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("UPDATE articles SET pinned = 0", [])?;
    let changed = tx.execute(
        "UPDATE articles SET pinned = 1, updated_at = datetime('now') WHERE id = ?1",
        params![id],
    )?;
    if changed == 0 {
        tx.rollback().ok();
        return Err(CmsError::ArticleNotFound(id));
    }
    tx.commit()?;
    Ok(())
}

pub fn unpin(conn: &Connection, id: i64) -> Result<(), CmsError> {
    let changed = conn.execute(
        "UPDATE articles SET pinned = 0, updated_at = datetime('now') WHERE id = ?1",
        params![id],
    )?;
    if changed == 0 {
        return Err(CmsError::ArticleNotFound(id));
    }
    Ok(())
}

pub fn update_tags(conn: &Connection, id: i64, tags: &[String]) -> Result<(), CmsError> {
    let mut deduped: Vec<String> = Vec::new();
    for tag in tags {
        let normalized = tag.trim().to_lowercase();
        if !normalized.is_empty() && !deduped.contains(&normalized) {
            deduped.push(normalized);
        }
    }
    let tags_str = deduped.join(",");
    let changed = conn.execute(
        "UPDATE articles SET tags = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![tags_str, id],
    )?;
    if changed == 0 {
        return Err(CmsError::ArticleNotFound(id));
    }
    Ok(())
}

pub fn update_language(conn: &Connection, id: i64, lang: &str) -> Result<(), CmsError> {
    let changed = conn.execute(
        "UPDATE articles SET language = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![lang, id],
    )?;
    if changed == 0 {
        return Err(CmsError::ArticleNotFound(id));
    }
    Ok(())
}

/// List articles with optional search filter.
/// Order: drafts first (newest first), then pinned, then published (newest first).
pub fn list_articles(conn: &Connection, search: Option<&str>) -> Result<Vec<Article>, CmsError> {
    let query = match search {
        Some(_) => {
            "SELECT id, title, slug, content, status, language, pinned, tags, published_at, created_at, updated_at
             FROM articles
             WHERE title LIKE '%' || ?1 || '%'
             ORDER BY CASE WHEN status = 'draft' THEN 0 WHEN pinned = 1 THEN 1 ELSE 2 END,
                      COALESCE(published_at, created_at) DESC"
        }
        None => {
            "SELECT id, title, slug, content, status, language, pinned, tags, published_at, created_at, updated_at
             FROM articles
             ORDER BY CASE WHEN status = 'draft' THEN 0 WHEN pinned = 1 THEN 1 ELSE 2 END,
                      COALESCE(published_at, created_at) DESC"
        }
    };

    let mut stmt = conn.prepare(query)?;
    let rows = match search {
        Some(q) => stmt.query_map(params![q], row_to_article)?,
        None => stmt.query_map([], row_to_article)?,
    };

    let mut articles = Vec::new();
    for row in rows {
        articles.push(row?);
    }
    Ok(articles)
}

/// First-run import: populates the DB from `posts_dir` only when the `articles`
/// table is empty. On subsequent runs the DB is the source of truth and this
/// function is a no-op, preserving any draft/pin state from prior sessions.
pub fn import_if_empty(
    conn: &Connection,
    posts_dir: &Path,
    date_field: &str,
) -> Result<usize, CmsError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM articles", [], |row| row.get(0))?;
    if count > 0 {
        return Ok(0);
    }
    import_from_filesystem(conn, posts_dir, date_field)
}

/// Import existing .md files from posts directory into the database.
/// Idempotent: matches by slug, updates existing entries instead of duplicating.
pub fn import_from_filesystem(
    conn: &Connection,
    posts_dir: &Path,
    date_field: &str,
) -> Result<usize, CmsError> {
    let entries = std::fs::read_dir(posts_dir)?;
    let mut count = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "md") {
            let content = std::fs::read_to_string(&path)?;
            let slug = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let title = config::frontmatter("title", &content).unwrap_or_else(|| slug.clone());
            let date = config::frontmatter_date(date_field, &content);
            let language = config::frontmatter("language", &content).unwrap_or_else(|| "en".to_string());
            let tags_str = config::extract_tags(&content);

            let existing: Option<i64> = conn
                .query_row(
                    "SELECT id FROM articles WHERE slug = ?1",
                    params![slug],
                    |row| row.get(0),
                )
                .ok();

            match existing {
                Some(id) => {
                    conn.execute(
                        "UPDATE articles SET title = ?1, content = ?2, language = ?3, tags = ?4,
                         status = 'published', published_at = COALESCE(published_at, ?5),
                         updated_at = datetime('now')
                         WHERE id = ?6",
                        params![title, content, language, tags_str.replace(' ', ","), date, id],
                    )?;
                }
                None => {
                    conn.execute(
                        "INSERT INTO articles (title, slug, content, status, language, tags, published_at)
                         VALUES (?1, ?2, ?3, 'published', ?4, ?5, ?6)",
                        params![
                            title,
                            slug,
                            content,
                            language,
                            tags_str.replace(' ', ","),
                            date,
                        ],
                    )?;
                }
            }
            count += 1;
        }
    }
    Ok(count)
}

/// Sync DB state to the posts directory: write published articles as `.md`
/// files, remove `.md` files for articles now marked draft. This is the only
/// bridge between CMS state and what the blog engine sees; the engine stays
/// unaware of the DB. Orphan files (no DB row) are left untouched so manual
/// copies into `posts/` still reach the engine. Every call rewrites published
/// files unconditionally, so callers should gate sync on DB state change to
/// avoid forcing full rebuilds through mtime churn.
pub fn sync_to_filesystem(
    conn: &Connection,
    posts_dir: &Path,
    _date_field: &str,
) -> Result<(), CmsError> {
    std::fs::create_dir_all(posts_dir)?;
    for article in list_articles(conn, None)? {
        let path = posts_dir.join(format!("{}.md", article.slug));
        match article.status {
            Status::Published => {
                std::fs::write(&path, &article.content)?;
            }
            Status::Draft => {
                if path.exists() {
                    std::fs::remove_file(&path)?;
                }
            }
        }
    }
    Ok(())
}

/// One-shot, idempotent migration: rewrite any body-only `content` rows to
/// full markdown (frontmatter + body) so `content` is the single source of
/// truth for each article's on-disk form.
pub fn migrate_content_to_full_markdown(
    conn: &Connection,
    date_field: &str,
) -> Result<(), CmsError> {
    let mut stmt = conn.prepare(
        "SELECT id, title, slug, content, status, language, pinned, tags, published_at, created_at, updated_at
         FROM articles",
    )?;
    let articles: Vec<Article> = stmt
        .query_map([], row_to_article)?
        .collect::<Result<_, _>>()?;
    drop(stmt);
    for article in articles {
        if article.content.starts_with("---\n") {
            continue;
        }
        let rewritten = build_markdown(&article, date_field);
        conn.execute(
            "UPDATE articles SET content = ?1 WHERE id = ?2",
            params![rewritten, article.id],
        )?;
    }
    Ok(())
}

/// Build frontmatter + content for writing a published .md file.
pub fn build_markdown(article: &Article, date_field: &str) -> String {
    use std::fmt::Write;
    let mut md = String::from("---\n");
    let _ = writeln!(md, "title: {}", article.title);
    if let Some(ref date) = article.published_at {
        let _ = writeln!(md, "{date_field}: {date}");
    }
    if !article.tags.is_empty() {
        let tags_yaml: Vec<String> = article.tags.iter().map(|t| format!("\"{t}\"")).collect();
        let _ = writeln!(md, "tags: [{}]", tags_yaml.join(", "));
    }
    if article.language != "en" {
        let _ = writeln!(md, "language: {}", article.language);
    }
    md.push_str("---\n\n");
    md.push_str(&article.content);
    md
}

// --- Helpers ---

fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn unique_slug(conn: &Connection, base: &str) -> Result<String, CmsError> {
    if !slug_exists(conn, base)? {
        return Ok(base.to_string());
    }
    for i in 2..100 {
        let candidate = format!("{base}-{i}");
        if !slug_exists(conn, &candidate)? {
            return Ok(candidate);
        }
    }
    Err(CmsError::SlugConflict(base.to_string()))
}

fn slug_exists(conn: &Connection, slug: &str) -> Result<bool, CmsError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM articles WHERE slug = ?1",
        params![slug],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn row_to_article(row: &rusqlite::Row) -> rusqlite::Result<Article> {
    let tags_str: String = row.get(7)?;
    let tags: Vec<String> = if tags_str.is_empty() {
        Vec::new()
    } else {
        tags_str.split(',').map(|s| s.trim().to_string()).collect()
    };
    let status_str: String = row.get(4)?;
    let status = Status::from_str(&status_str).unwrap_or(Status::Draft);

    Ok(Article {
        id: row.get(0)?,
        title: row.get(1)?,
        slug: row.get(2)?,
        content: row.get(3)?,
        status,
        language: row.get(5)?,
        pinned: row.get::<_, i64>(6)? != 0,
        tags,
        published_at: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::tempdir;
    use std::fs;

    fn test_db() -> Connection {
        init_db_memory().expect("failed to init test db")
    }

    // --- migrate_content_to_full_markdown ---

    #[test]
    fn migrate_rewrites_body_only_content_to_full_markdown() {
        let conn = test_db();
        let article = create_article(&conn, "Test Post").unwrap();
        update_content(&conn, article.id, "Just a body.").unwrap();
        assert_eq!(get_article(&conn, article.id).unwrap().content, "Just a body.");

        migrate_content_to_full_markdown(&conn, "published_at").unwrap();

        let after = get_article(&conn, article.id).unwrap();
        assert!(after.content.starts_with("---\n"), "no frontmatter: {:?}", after.content);
        assert!(after.content.contains("title: Test Post"));
        assert!(after.content.contains("Just a body."));
    }

    #[test]
    fn migrate_is_idempotent() {
        let conn = test_db();
        let article = create_article(&conn, "Test").unwrap();
        update_content(&conn, article.id, "Body.").unwrap();
        migrate_content_to_full_markdown(&conn, "published_at").unwrap();
        let first = get_article(&conn, article.id).unwrap().content;
        migrate_content_to_full_markdown(&conn, "published_at").unwrap();
        let second = get_article(&conn, article.id).unwrap().content;
        assert_eq!(first, second);
    }

    // --- init_db ---

    #[test]
    fn init_db_creates_articles_table() {
        let conn = test_db();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='articles'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn init_db_is_idempotent() {
        let conn = test_db();
        create_schema(&conn).unwrap();
    }

    // --- create_article ---

    #[test]
    fn create_article_returns_draft_with_slug() {
        let conn = test_db();
        let article = create_article(&conn, "My First Post").unwrap();
        assert_eq!(article.title, "My First Post");
        assert_eq!(article.slug, "my-first-post");
        assert_eq!(article.status, Status::Draft);
    }

    #[test]
    fn create_article_deduplicates_slugs() {
        let conn = test_db();
        let a1 = create_article(&conn, "Hello World").unwrap();
        let a2 = create_article(&conn, "Hello World").unwrap();
        assert_eq!(a1.slug, "hello-world");
        assert_eq!(a2.slug, "hello-world-2");
    }

    // --- get_article ---

    #[test]
    fn get_article_retrieves_by_id() {
        let conn = test_db();
        let created = create_article(&conn, "Test").unwrap();
        let fetched = get_article(&conn, created.id).unwrap();
        assert_eq!(fetched.title, "Test");
    }

    #[test]
    fn get_article_returns_not_found() {
        let conn = test_db();
        let result = get_article(&conn, 999);
        assert!(matches!(result, Err(CmsError::ArticleNotFound(999))));
    }

    // --- delete_article ---

    #[test]
    fn delete_article_removes_it() {
        let conn = test_db();
        let article = create_article(&conn, "To Delete").unwrap();
        delete_article(&conn, article.id).unwrap();
        assert!(matches!(
            get_article(&conn, article.id),
            Err(CmsError::ArticleNotFound(_))
        ));
    }

    #[test]
    fn delete_nonexistent_returns_not_found() {
        let conn = test_db();
        assert!(matches!(
            delete_article(&conn, 999),
            Err(CmsError::ArticleNotFound(999))
        ));
    }

    // --- update_content ---

    #[test]
    fn update_content_persists() {
        let conn = test_db();
        let article = create_article(&conn, "Content Test").unwrap();
        update_content(&conn, article.id, "Hello world").unwrap();
        let fetched = get_article(&conn, article.id).unwrap();
        assert_eq!(fetched.content, "Hello world");
    }

    // --- publish / unpublish ---

    #[test]
    fn publish_sets_status_and_date() {
        let conn = test_db();
        let article = create_article(&conn, "To Publish").unwrap();
        assert_eq!(article.status, Status::Draft);
        assert!(article.published_at.is_none());

        publish(&conn, article.id).unwrap();
        let fetched = get_article(&conn, article.id).unwrap();
        assert_eq!(fetched.status, Status::Published);
        assert!(fetched.published_at.is_some());
    }

    #[test]
    fn publish_preserves_existing_published_at() {
        let conn = test_db();
        let article = create_article(&conn, "Imported").unwrap();
        // Simulate an imported article with an original frontmatter date.
        conn.execute(
            "UPDATE articles SET published_at = '2024-01-15' WHERE id = ?1",
            params![article.id],
        )
        .unwrap();

        publish(&conn, article.id).unwrap();

        let fetched = get_article(&conn, article.id).unwrap();
        assert_eq!(fetched.status, Status::Published);
        assert_eq!(fetched.published_at.as_deref(), Some("2024-01-15"));
    }

    #[test]
    fn unpublish_preserves_published_at() {
        let conn = test_db();
        let article = create_article(&conn, "Round Trip").unwrap();
        conn.execute(
            "UPDATE articles SET published_at = '2024-01-15' WHERE id = ?1",
            params![article.id],
        )
        .unwrap();
        publish(&conn, article.id).unwrap();
        unpublish(&conn, article.id).unwrap();

        let fetched = get_article(&conn, article.id).unwrap();
        assert_eq!(fetched.status, Status::Draft);
        assert_eq!(fetched.published_at.as_deref(), Some("2024-01-15"));
    }

    // --- pin / unpin ---

    #[test]
    fn pin_sets_pinned_and_unpins_others() {
        let conn = test_db();
        let a = create_article(&conn, "Article A").unwrap();
        let b = create_article(&conn, "Article B").unwrap();

        pin(&conn, a.id).unwrap();
        assert!(get_article(&conn, a.id).unwrap().pinned);

        pin(&conn, b.id).unwrap();
        assert!(!get_article(&conn, a.id).unwrap().pinned);
        assert!(get_article(&conn, b.id).unwrap().pinned);
    }

    #[test]
    fn unpin_clears_pinned() {
        let conn = test_db();
        let article = create_article(&conn, "Pinned").unwrap();
        pin(&conn, article.id).unwrap();
        unpin(&conn, article.id).unwrap();
        assert!(!get_article(&conn, article.id).unwrap().pinned);
    }

    // --- list_articles ---

    #[test]
    fn list_articles_orders_drafts_pinned_published() {
        let conn = test_db();
        let _draft = create_article(&conn, "Draft Article").unwrap();
        let pub1 = create_article(&conn, "Published One").unwrap();
        let pinned = create_article(&conn, "Pinned Article").unwrap();

        publish(&conn, pub1.id).unwrap();
        publish(&conn, pinned.id).unwrap();
        pin(&conn, pinned.id).unwrap();

        let articles = list_articles(&conn, None).unwrap();
        assert_eq!(articles.len(), 3);
        assert_eq!(articles[0].title, "Draft Article");
        assert_eq!(articles[1].title, "Pinned Article");
        assert_eq!(articles[2].title, "Published One");
    }

    #[test]
    fn list_articles_filters_by_search() {
        let conn = test_db();
        create_article(&conn, "Rust Guide").unwrap();
        create_article(&conn, "Elixir Guide").unwrap();
        create_article(&conn, "Go Tutorial").unwrap();

        let results = list_articles(&conn, Some("Guide")).unwrap();
        assert_eq!(results.len(), 2);
    }

    // --- update_tags ---

    #[test]
    fn update_tags_deduplicates_and_normalizes() {
        let conn = test_db();
        let article = create_article(&conn, "Tagged").unwrap();
        update_tags(
            &conn,
            article.id,
            &["Rust".to_string(), "rust".to_string(), "TDD".to_string()],
        )
        .unwrap();
        let fetched = get_article(&conn, article.id).unwrap();
        assert_eq!(fetched.tags, vec!["rust", "tdd"]);
    }

    // --- update_language ---

    #[test]
    fn update_language_persists() {
        let conn = test_db();
        let article = create_article(&conn, "Language Test").unwrap();
        assert_eq!(article.language, "en");
        update_language(&conn, article.id, "pt").unwrap();
        let fetched = get_article(&conn, article.id).unwrap();
        assert_eq!(fetched.language, "pt");
    }

    // --- import_from_filesystem ---

    #[test]
    fn import_creates_articles_from_md_files() {
        let conn = test_db();
        let dir = tempdir();
        fs::write(
            dir.join("2026-03-29-test-post.md"),
            "---\ntitle: Test Post\ndate: 2026-03-29\n---\n\nHello world.\n",
        )
        .unwrap();
        fs::write(
            dir.join("2026-03-28-another.md"),
            "---\ntitle: Another Post\ndate: 2026-03-28\n---\n\nContent.\n",
        )
        .unwrap();

        let count = import_from_filesystem(&conn, &dir, "date").unwrap();
        assert_eq!(count, 2);

        let articles = list_articles(&conn, None).unwrap();
        assert_eq!(articles.len(), 2);
        // All imported as published
        assert!(articles.iter().all(|a| a.status == Status::Published));
    }

    #[test]
    fn sync_writes_published_article_to_md_file() {
        let conn = test_db();
        let dir = tempdir();
        let article = create_article(&conn, "Hello World").unwrap();
        let md = "---\ntitle: Hello World\n---\n\nBody here.";
        update_content(&conn, article.id, md).unwrap();
        publish(&conn, article.id).unwrap();

        sync_to_filesystem(&conn, &dir, "date").unwrap();

        let path = dir.join("hello-world.md");
        assert!(path.exists(), "expected {path:?} to exist");
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, md, "content should be written verbatim");
    }

    #[test]
    fn sync_leaves_orphan_md_files_untouched() {
        let conn = test_db();
        let dir = tempdir();
        let orphan = dir.join("manual-post.md");
        fs::write(&orphan, "# Manual\n").unwrap();

        sync_to_filesystem(&conn, &dir, "date").unwrap();

        assert!(orphan.exists(), "orphan .md should survive sync");
        assert_eq!(fs::read_to_string(&orphan).unwrap(), "# Manual\n");
    }

    #[test]
    fn sync_removes_md_file_for_draft_article() {
        let conn = test_db();
        let dir = tempdir();
        let article = create_article(&conn, "To Be Unpublished").unwrap();
        update_content(&conn, article.id, "Content.").unwrap();
        publish(&conn, article.id).unwrap();

        sync_to_filesystem(&conn, &dir, "date").unwrap();
        let path = dir.join("to-be-unpublished.md");
        assert!(path.exists());

        unpublish(&conn, article.id).unwrap();
        sync_to_filesystem(&conn, &dir, "date").unwrap();

        assert!(!path.exists(), "draft .md should be removed after sync");
    }

    #[test]
    fn import_stores_full_markdown_in_content() {
        let conn = test_db();
        let dir = tempdir();
        fs::write(
            dir.join("hello.md"),
            "---\ntitle: Hello\ndate: 2026-04-11\n---\n\nBody text.\n",
        )
        .unwrap();
        import_from_filesystem(&conn, &dir, "date").unwrap();
        let articles = list_articles(&conn, None).unwrap();
        assert!(articles[0].content.starts_with("---\n"), "{}", articles[0].content);
        assert!(articles[0].content.contains("title: Hello"));
        assert!(articles[0].content.contains("Body text."));
    }

    #[test]
    fn import_if_empty_skips_when_db_has_articles() {
        let conn = test_db();
        let dir = tempdir();
        fs::write(
            dir.join("2026-03-29-post.md"),
            "---\ntitle: Post\ndate: 2026-03-29\n---\n\nBody.\n",
        )
        .unwrap();

        // First-run import populates the DB.
        import_if_empty(&conn, &dir, "date").unwrap();
        let articles = list_articles(&conn, None).unwrap();
        assert_eq!(articles.len(), 1);

        // User toggles the article to draft.
        unpublish(&conn, articles[0].id).unwrap();

        // Second-run must NOT re-import and must NOT overwrite the draft state.
        import_if_empty(&conn, &dir, "date").unwrap();
        let after = list_articles(&conn, None).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].status, Status::Draft);
    }

    #[test]
    fn import_does_not_override_existing_published_at() {
        let conn = test_db();
        let dir = tempdir();
        // First import: DB has no row, file date wins.
        fs::write(
            dir.join("post.md"),
            "---\ntitle: Post\ndate: 2024-01-15\n---\n\nv1.\n",
        )
        .unwrap();
        import_from_filesystem(&conn, &dir, "date").unwrap();

        // User edits the file with a different (wrong) date.
        fs::write(
            dir.join("post.md"),
            "---\ntitle: Post\ndate: 2099-12-31\n---\n\nv2.\n",
        )
        .unwrap();
        import_from_filesystem(&conn, &dir, "date").unwrap();

        let articles = list_articles(&conn, None).unwrap();
        assert_eq!(articles.len(), 1);
        // Immutable: the DB date from the first import must win.
        assert_eq!(articles[0].published_at.as_deref(), Some("2024-01-15"));
        // Content still updates, stored verbatim including frontmatter.
        assert!(articles[0].content.contains("v2.\n"));
        assert!(articles[0].content.starts_with("---\n"));
    }

    #[test]
    fn import_is_idempotent() {
        let conn = test_db();
        let dir = tempdir();
        fs::write(
            dir.join("test.md"),
            "---\ntitle: Test\ndate: 2026-03-29\n---\n\nBody.\n",
        )
        .unwrap();

        import_from_filesystem(&conn, &dir, "date").unwrap();
        import_from_filesystem(&conn, &dir, "date").unwrap();

        let articles = list_articles(&conn, None).unwrap();
        assert_eq!(articles.len(), 1);
    }

    // --- content preservation ---

    #[test]
    fn update_content_with_empty_string_clears_content() {
        let conn = test_db();
        let article = create_article(&conn, "Will Be Cleared").unwrap();
        update_content(&conn, article.id, "Original content here").unwrap();
        update_content(&conn, article.id, "").unwrap();
        let fetched = get_article(&conn, article.id).unwrap();
        assert_eq!(fetched.content, "");
    }

    #[test]
    fn content_survives_when_caller_guards_empty() {
        // Simulates the fix in mod.rs: caller checks !final_content.is_empty()
        // before calling update_content. This test verifies the DB behavior.
        let conn = test_db();
        let article = create_article(&conn, "Precious Content").unwrap();
        update_content(&conn, article.id, "Do not lose this").unwrap();

        // Simulate editor returning empty (user did :q! without saving)
        let final_content = "";
        if !final_content.is_empty() {
            update_content(&conn, article.id, final_content).unwrap();
        }

        let fetched = get_article(&conn, article.id).unwrap();
        assert_eq!(fetched.content, "Do not lose this");
    }

    #[test]
    fn content_updates_when_changed() {
        let conn = test_db();
        let article = create_article(&conn, "Will Update").unwrap();
        update_content(&conn, article.id, "Version 1").unwrap();

        let final_content = "Version 2";
        if !final_content.is_empty() && final_content != "Version 1" {
            update_content(&conn, article.id, final_content).unwrap();
        }

        let fetched = get_article(&conn, article.id).unwrap();
        assert_eq!(fetched.content, "Version 2");
    }

    #[test]
    fn content_not_updated_when_unchanged() {
        let conn = test_db();
        let article = create_article(&conn, "Same Content").unwrap();
        update_content(&conn, article.id, "Original").unwrap();
        let before = get_article(&conn, article.id).unwrap().updated_at;

        // Small delay to ensure different timestamp if updated
        std::thread::sleep(std::time::Duration::from_millis(1100));

        let final_content = "Original";
        if !final_content.is_empty() && final_content != "Original" {
            update_content(&conn, article.id, final_content).unwrap();
        }

        let after = get_article(&conn, article.id).unwrap().updated_at;
        assert_eq!(before, after);
    }

    // --- update_title ---

    #[test]
    fn update_title_persists() {
        let conn = test_db();
        let article = create_article(&conn, "Old Title").unwrap();
        update_title(&conn, article.id, "New Title").unwrap();
        let fetched = get_article(&conn, article.id).unwrap();
        assert_eq!(fetched.title, "New Title");
    }

    #[test]
    fn update_title_nonexistent_returns_not_found() {
        let conn = test_db();
        assert!(matches!(
            update_title(&conn, 999, "Title"),
            Err(CmsError::ArticleNotFound(999))
        ));
    }

    // --- update_tags edge cases ---

    #[test]
    fn update_tags_with_empty_array_clears_tags() {
        let conn = test_db();
        let article = create_article(&conn, "Tagged").unwrap();
        update_tags(&conn, article.id, &["rust".to_string()]).unwrap();
        update_tags(&conn, article.id, &[]).unwrap();
        let fetched = get_article(&conn, article.id).unwrap();
        assert!(fetched.tags.is_empty());
    }

    #[test]
    fn update_tags_strips_whitespace() {
        let conn = test_db();
        let article = create_article(&conn, "Tagged").unwrap();
        update_tags(&conn, article.id, &["  rust  ".to_string(), " tdd ".to_string()]).unwrap();
        let fetched = get_article(&conn, article.id).unwrap();
        assert_eq!(fetched.tags, vec!["rust", "tdd"]);
    }

    // --- build_markdown edge cases ---

    #[test]
    fn build_markdown_draft_without_date() {
        let article = Article {
            id: 1,
            title: "Draft Post".to_string(),
            slug: "draft-post".to_string(),
            content: "Work in progress.".to_string(),
            status: Status::Draft,
            language: "en".to_string(),
            pinned: false,
            tags: vec![],
            published_at: None,
            created_at: "2026-03-29".to_string(),
            updated_at: "2026-03-29".to_string(),
        };
        let md = build_markdown(&article, "date");
        assert!(md.starts_with("---\n"));
        assert!(md.contains("title: Draft Post"));
        assert!(!md.contains("date:"));
        assert!(!md.contains("tags:"));
        assert!(!md.contains("language:"));
        assert!(md.contains("Work in progress."));
    }

    #[test]
    fn build_markdown_non_english_includes_language() {
        let article = Article {
            id: 1,
            title: "Post".to_string(),
            slug: "post".to_string(),
            content: "Body.".to_string(),
            status: Status::Published,
            language: "pt".to_string(),
            pinned: false,
            tags: vec![],
            published_at: Some("2026-03-29".to_string()),
            created_at: "2026-03-29".to_string(),
            updated_at: "2026-03-29".to_string(),
        };
        let md = build_markdown(&article, "date");
        assert!(md.contains("language: pt"));
    }

    // --- create_article edge cases ---

    #[test]
    fn create_article_with_special_chars_slugifies() {
        let conn = test_db();
        let article = create_article(&conn, "C++ & Rust: A Comparison!").unwrap();
        assert_eq!(article.slug, "c-rust-a-comparison");
    }

    // --- list_articles edge cases ---

    #[test]
    fn list_articles_search_returns_empty_on_no_match() {
        let conn = test_db();
        create_article(&conn, "Rust Guide").unwrap();
        let results = list_articles(&conn, Some("Python")).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn list_articles_empty_db() {
        let conn = test_db();
        let results = list_articles(&conn, None).unwrap();
        assert!(results.is_empty());
    }

    // --- slugify ---

    #[test]
    fn slugify_converts_title() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("  My Post  "), "my-post");
        assert_eq!(slugify("Rust & TDD"), "rust-tdd");
    }

    // --- build_markdown ---

    #[test]
    fn build_markdown_produces_valid_frontmatter() {
        let article = Article {
            id: 1,
            title: "Test Post".to_string(),
            slug: "test-post".to_string(),
            content: "Hello world.".to_string(),
            status: Status::Published,
            language: "en".to_string(),
            pinned: false,
            tags: vec!["rust".to_string(), "tdd".to_string()],
            published_at: Some("2026-03-29".to_string()),
            created_at: "2026-03-29".to_string(),
            updated_at: "2026-03-29".to_string(),
        };
        let md = build_markdown(&article, "date");
        assert!(md.starts_with("---\n"));
        assert!(md.contains("title: Test Post"));
        assert!(md.contains("date: 2026-03-29"));
        assert!(md.contains("tags: [\"rust\", \"tdd\"]"));
        assert!(md.contains("Hello world."));
        // language "en" should NOT appear (it's the default)
        assert!(!md.contains("language:"));
    }

}
