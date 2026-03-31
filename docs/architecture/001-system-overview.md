# DevTUI Architecture

Technical reference for the DevTUI system. Updated 2026-03-31.

## System Context (C4 Level 1)

```
+-------------------+       +-------------------+
|     Developer     |       |   Blog Readers    |
|  (writes posts)   |       | (consume HTML)    |
+--------+----------+       +---------+---------+
         |                            |
    terminal (TUI)              static HTML
         |                            |
+--------v----------------------------v---------+
|                  DevTUI                        |
|  Editor (TUI) + Engine (SSG) + CLI (Make)      |
+--------+----------------------------+----------+
         |                            |
    vim (PTY)                    rsync / git
         |                            |
+--------v----------+       +---------v---------+
|   Filesystem      |       |   Blog Repo       |
| (SQLite, /tmp)    |       | (GitHub Pages)    |
+-------------------+       +-------------------+
```

**DevTUI** is a terminal markdown editor with live preview (editor) and a multi-site static blog generator (engine), glued together by a Makefile CLI.

## Container Diagram (C4 Level 2)

```
devtui (Rust binary, ~6200 LOC)
|
+-- editor/          TUI editor with embedded vim
|   +-- vim.rs       PTY lifecycle, event loop, scroll sync, mode detection
|   +-- preview.rs   Markdown-to-ratatui rendering with offset map
|   +-- db.rs        SQLite CMS (articles CRUD, import, export)
|   +-- list.rs      Article list view (search, filter, sort)
|   +-- chrome.rs    Headless Chrome HTML screenshots
|   +-- kitty.rs     Kitty graphics protocol image encoding
|   +-- tmux.rs      tmux detection, APC escape wrapping
|   +-- mod.rs       Orchestration, layout, event dispatch
|
+-- engine/          Static site generator
|   +-- build.rs     Pipeline orchestrator + 47 integration tests
|   +-- config.rs    BlogConfig, Post, frontmatter parsing
|   +-- template.rs  $var$ substitution, $if()$...$endif$ conditionals
|   +-- index.rs     Index page assembly (nav, posts, footer, filters)
|   +-- feed.rs      RSS 2.0 generation
|   +-- seo.rs       sitemap.xml, robots.txt, 404.html
|   +-- analytics.rs Google Analytics injection
|   +-- minify.rs    CSS compilation, minification, HTML inlining
|   +-- links.rs     Social links, tags, guides HTML
|   +-- markdown.rs  Markdown-to-HTML, snippets, emoji shortcodes
|   +-- templates/   Default HTML templates
|   +-- themes/      CSS themes (paper, terminal)
|
+-- bin/
    +-- editor.rs    devtui binary entry point (32 LOC)
    +-- engine.rs    devtui-engine binary entry point (92 LOC)
```

## Component Details (C4 Level 3)

### Editor: vim via PTY

The editor embeds a real vim process inside a pseudo-terminal. No vim reimplementation.

```
Keystrokes --> crossterm --> pty_writer --> vim (child process)
                                              |
vim output <-- vt100::Parser <-- pty_reader <-+
      |
      +-- screen.title() --> position + mode (zero I/O)
      +-- /tmp/devtui-content --> content (debounced I/O)
```

**Position sync**: vim's `titlestring=%{line('w0')}:%{mode()}` encodes the first visible line and current mode via OSC escape sequences. The vt100 parser reads `screen.title()` every frame. No file I/O.

**Content sync**: vim writes the buffer to `/tmp/devtui-content` on `CursorHold` (150ms idle) and `TextChanged`. A background thread polls every 100ms via `Arc<Mutex<Option<String>>>`.

**Mode detection**: Parsed from titlestring. vim's `mode()` returns: `n`, `i`, `v/V`, `R`, `c`. Displayed as a colored badge.

**Preview rendering**: `render_with_offsets()` parses full document, returns rendered lines + source-to-rendered offset map. Preview uses `Paragraph::scroll()`. Re-renders only on content change.

**HTML preview (optional)**: Headless Chrome renders the article HTML, takes a screenshot, encodes it as a Kitty graphics image, and displays it in the preview pane. Falls back to text preview if Chrome is unavailable.

### Engine: Build Pipeline

```
blog.toml --> BlogConfig
posts/*.md --> collect_posts() --> Vec<Post>
                                      |
     +--------------------------------+
     |
     v
For each post:
  1. Check mtime: skip if html newer than md + template
  2. Extract frontmatter (title, date, description, image, tags)
  3. Preprocess markdown (fix list/blockquote spacing)
  4. Render HTML via pulldown-cmark with article template
  5. Inject JSON-LD, OG tags, canonical URL

Then:
  6. Concatenate theme CSS (base, index, article, syntax, responsive)
  7. Build index.html (nav, post list, filters, search, footer)
  8. Generate sitemap.xml, robots.txt, feed.xml, 404.html
  9. Inject Google Analytics (if configured)
  10. Minify CSS, inline into HTML, minify HTML
```

**Incremental builds**: Compare mtime of .md vs .html. Skip if HTML is newer than both source and template. Index, sitemap, feed always rebuild.

**Template resolution**: `resolve_file()` checks blog templates > theme templates > engine defaults. Three-level cascade.

**CSS inlining**: Theme CSS is concatenated, minified, then injected via `<style>` tag replacing the `<link rel="stylesheet">`. Zero external CSS requests.

### Database: SQLite CMS

```sql
articles (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  title       TEXT NOT NULL,
  slug        TEXT NOT NULL UNIQUE,
  content     TEXT NOT NULL DEFAULT '',
  status      TEXT NOT NULL DEFAULT 'draft',    -- 'draft' | 'published'
  language    TEXT NOT NULL DEFAULT 'en',
  pinned      INTEGER NOT NULL DEFAULT 0,       -- 0 or 1
  tags        TEXT NOT NULL DEFAULT '',          -- comma-separated
  published_at TEXT,
  created_at  TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
)
```

**Import**: `import_from_filesystem()` reads existing .md files, extracts frontmatter, upserts by slug (idempotent).

**Export**: `build_markdown()` reconstructs frontmatter + body for writing .md files.

**Pinning**: Only one article pinned at a time. `pin()` uses a transaction to unpin all, then pin target.

## Dependencies

### Rust Crates (Cargo.toml)

| Crate | Purpose |
|-------|---------|
| ratatui 0.29 | Terminal UI framework |
| crossterm 0.28 | Terminal I/O (keyboard, mouse, resize) |
| pulldown-cmark 0.12 | Markdown-to-HTML parser |
| tui-term 0.2.0 | VT100 terminal widget for ratatui |
| vt100 0.15 | Terminal escape sequence parser |
| portable-pty 0.9 | Cross-platform pseudo-terminal |
| toml 0.8 | blog.toml config parsing |
| serde 1 | Serialization framework |
| gh-emoji 1.0 | GitHub emoji shortcode-to-unicode |
| tiny_http 0.12 | Dev server for blog preview |
| rusqlite 0.31 (bundled) | SQLite database |
| headless_chrome 1.0 | HTML preview screenshots |
| ratatui-image 9.0 | Kitty graphics protocol rendering |
| image 0.25 | Image decoding |
| base64 0.22.1 | Base64 encoding for Kitty protocol |
| log 0.4 | Logging facade |
| simplelog 0.12 | File-based logging |
| nvim-rs 0.9 | **UNUSED** (legacy, candidate for removal) |
| tokio 1 | Async runtime (required by nvim-rs) |

### System Dependencies

| Tool | Required | Purpose |
|------|----------|---------|
| vim | Yes | Embedded editor (spawned via PTY with `-u NONE -N`) |
| Rust toolchain | Yes | Build (cargo, rustc) |
| Google Chrome | Optional | HTML preview screenshots (graceful fallback) |
| rsync | Deploy only | Copy dist/ to blog repo |

## Data Flow

### Editor Session

```
User launches: make editor.run FILE=post.md
  |
  +-- init SQLite db (~/.devtui/articles.db)
  +-- spawn vim via PTY (vim -u NONE -N ...)
  +-- enter event loop:
        |
        +-- crossterm polls keyboard/mouse/resize
        +-- keystrokes forwarded to vim via pty_writer
        +-- vim renders to vt100 parser
        +-- every frame: read screen.title() for position/mode
        +-- every 100ms: check /tmp/devtui-content for changes
        +-- on content change: re-render preview
        +-- on Ctrl+R: refresh Chrome HTML preview
        +-- on Ctrl+O: open HTML in browser
        +-- on Ctrl+L: cycle layout (Split/Preview/Editor)
        +-- on :q/:wq: exit loop, save to db if content changed
```

### Blog Build

```
User runs: make blog.build.leandronsp.com
  |
  +-- read blogs/leandronsp.com/blog.toml
  +-- collect posts from blogs/leandronsp.com/posts/ (via symlink)
  +-- sort posts by date descending
  +-- for each post: render HTML (incremental)
  +-- build index.html, sitemap, robots, feed, 404
  +-- inject analytics
  +-- minify CSS, inline, minify HTML
  +-- output to dist/leandronsp.com/
```

### Deploy

```
User runs: make deploy.cp.leandronsp.com
  |
  +-- build (same as above)
  +-- resolve REPO_DIR by following posts symlink
  +-- rsync -a dist/leandronsp.com/ $REPO_DIR/
  +-- user commits and pushes manually
```

## Test Architecture

245 tests total. All inline with `#[cfg(test)]`.

| Module | Tests | Type |
|--------|-------|------|
| engine/build.rs | 47 | Integration (full pipeline, temp dirs) |
| editor/preview.rs | 18 | Unit (rendering, offset mapping) |
| editor/db.rs | 34 | Unit (SQLite in-memory) |
| engine/config.rs | 32 | Unit (frontmatter, config parsing) |
| engine/markdown.rs | 25 | Unit (HTML rendering, snippets) |
| engine/minify.rs | 16 | Unit (CSS/HTML minification) |
| engine/template.rs | 9 | Unit (variable substitution) |
| engine/index.rs | 8 | Unit (post list, lang normalization) |
| engine/feed.rs | 10 | Unit + integration (RSS generation) |
| engine/links.rs | 6 | Unit (social links, tags, guides) |
| engine/seo.rs | 8 | Unit + integration (sitemap, robots) |
| engine/analytics.rs | 5 | Unit + integration (GA injection) |

**Test patterns**: Temp directories for file I/O tests. In-memory SQLite for db tests. No mocks. No external dependencies. All tests run in ~1s.

## Build Targets (Makefile)

```
mk/editor.mk         mk/blog.mk
  editor.run            blog.list
  editor.build          blog.build / blog.build.<name>
  editor.test           blog.serve.<name>
                        blog.clean
                        blog.test
                        deploy.cp.<name>
```

Root Makefile: `make test` (all tests + clippy), `make lint`, `make help`.

## Key Design Decisions

1. **Vim via PTY, not reimplemented**. Deleted ~1000 lines of custom vim/buffer code. Full vim with zero compatibility surface.

2. **Zero-I/O position sync**. vim's `titlestring` via OSC sequences. The vt100 parser reads `screen.title()` every frame without touching the filesystem.

3. **CSS inlining, no external stylesheets**. One CSS change touches every HTML file. Trade-off: zero HTTP requests for styles, at the cost of larger diffs on CSS changes.

4. **Incremental builds by mtime**. Simple, fast, no dependency graph. Clean build needed for CSS-only changes.

5. **SQLite for CMS state**. Single file, zero config, ACID. Articles table stores everything. Import/export via markdown frontmatter.

6. **Three-level template cascade**. Blog overrides > theme defaults > engine defaults. Same pattern as WordPress child themes.

7. **Preprocessed markdown spacing**. `post_body()` inserts blank lines before lists/blockquotes that lack them. Fixes dev.to-style markdown without modifying the parser.
