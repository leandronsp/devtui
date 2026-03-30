# CLAUDE.md

## Project Overview

DevTUI is three things:

1. **Editor** — Terminal markdown editor with live preview. Embeds vim via PTY with real-time rendered markdown preview. Built in Rust with ratatui.
2. **Engine** — Multi-site static blog generator. Converts markdown to SEO-optimized HTML. Pure Rust, using pulldown-cmark.
3. **CLI** — The glue. Makefile orchestrates builds across multiple blogs.

## Dependencies

### Editor

- **Rust toolchain** (cargo, rustc)
- **vim** — embedded via PTY as child process
- Crates: ratatui, crossterm, pulldown-cmark, tui-term, vt100, portable-pty

### Engine

- Pure Rust. No external tools.
- Crates: pulldown-cmark, toml, serde, gh-emoji
- **rsync** — deploy only (copies dist to blog repo)

### Install (macOS)

```bash
brew install vim
# Rust toolchain via rustup
```

## Architecture

### Editor (src/editor/)

- **`src/editor/mod.rs`** — PTY setup, vim spawn, event loop, scroll sync, mode detection
- **`src/editor/preview.rs`** — Markdown to ratatui Lines rendering + 26 unit tests

### Engine (src/engine/)

DDD module structure. Each module is self-contained with unit tests.

- **`src/engine/build.rs`** — Pipeline orchestrator. 43 integration tests.
- **`src/engine/config.rs`** — `BlogConfig` (serde), `frontmatter()`, `post_body()`. 20 tests.
- **`src/engine/template.rs`** — `resolve_file()`, `template_render()` with `$var$` and `$if()$...$endif$`. 9 tests.
- **`src/engine/seo.rs`** — `xml_escape()`, `sitemap_entry()`, `robots_txt()`, `rss_header()`, `rss_item()`. 14 tests.
- **`src/engine/minify.rs`** — CSS/HTML minification, CSS inlining. 8 tests.
- **`src/engine/links.rs`** — Social links, tags, guides HTML from BlogConfig. 6 tests.
- **`src/engine/markdown.rs`** — Markdown-to-HTML (pulldown-cmark), `post_snippet()`, emoji shortcodes. 19 tests.
- **`engine/templates/`** — Default HTML templates.
- **`engine/themes/<name>/`** — Theme CSS, split into modular files.

### Themes (engine/themes/)

Themes provide modular CSS split into files concatenated in order:

```
engine/themes/<name>/
  base.css        # variables, reset, body defaults (line-height, font-size)
  index.css       # index page styles
  article.css     # article page styles
  syntax.css      # code syntax highlighting
  responsive.css  # mobile breakpoints
```

Blog selects theme via `theme = "paper"` in `blog.toml`. Default theme is `paper`. The build concatenates the CSS files, minifies, and inlines into each HTML.

**To change CSS, edit the theme files under `engine/themes/<name>/`. There is no fallback CSS file.**

### Blog Content (blogs/, gitignored)

- **`blogs/<name>/blog.toml`** — Site config (title, subtitle, url, author, date_field, lang, theme).
- **`blogs/<name>/posts/`** — Markdown posts with YAML frontmatter.
- **`blogs/<name>/templates/`** — Optional template overrides.

Blogs can be local directories or symlinks to external repos.

### Output (dist/, gitignored)

Generated per blog: article HTMLs, index.html, 404.html, sitemap.xml, robots.txt, feed.xml. CSS inlined into HTML, no external stylesheet.

## Commands

Makefile is split into `mk/editor.mk` and `mk/blog.mk`, included from the root Makefile.

```bash
# Editor (mk/editor.mk)
make editor.run FILE=file.md   # run editor
make editor.build              # release build
make editor.test               # editor tests
make editor.lint               # clippy

# Blog (mk/blog.mk)
make blog.list                 # list blogs
make blog.build                # build all blogs
make blog.build.<name>         # build one blog
make blog.serve.<name>         # build and serve on localhost:8000
make blog.clean                # remove dist/
make deploy.git.<name>         # build, copy to repo

# All
make help                      # all targets
make test                      # cargo + bats
```

## How the Editor Works

### Vim via PTY

vim runs inside a pseudo-terminal (`vim -u NONE -N`). All keystrokes pass through. Full vim, not a reimplementation.

### Position Sync (zero I/O)

vim's `titlestring=%{line('w0')}:%{mode()}` encodes the first visible line and current mode in the terminal title. The vt100 parser reads `screen.title()` every frame.

### Content Sync (debounced I/O)

vim writes buffer to `/tmp/devtui-content` on `CursorHold` (fires after 150ms idle) and `TextChanged`. A background thread polls every 100ms.

### Preview Rendering

`render_with_offsets()` parses the full document once, returns rendered lines + source-to-rendered offset map. Preview uses `Paragraph::scroll()` to scroll to the correct position. Re-renders only when content changes.

### Mode Detection

Parsed from titlestring. Vim's `mode()` returns: `n`, `i`, `v/V`, `R`, `c`. Displayed as colored badge.

## How the Engine Works

### Blog Config

Each blog has a `blog.toml` with:
- `title`, `subtitle` — displayed on index and in meta tags
- `url` — canonical base URL for SEO
- `author` — used in meta tags and JSON-LD
- `date_field` — which frontmatter field holds the date (e.g. `date` or `published_at`)
- `lang` — HTML lang attribute
- `theme` — CSS theme (default: `paper`). Available: `paper`, `terminal`
- `analytics_id` — Google Analytics measurement ID (optional, lazy-loaded)
- `license`, `license_url` — footer license link (optional)
- `tags` — curated tag list for index filter (optional)
- `[[links]]` — social links in header nav (label + url)
- `[[guides]]` — guide badges in header (title + url)

### Build Pipeline

1. Read `blog.toml` config
2. For each post: extract frontmatter, preprocess markdown (fix lists/blockquotes), run pandoc with article template. **Incremental**: skip if html is newer than md and template.
3. Concatenate theme CSS modules (base, index, article, syntax, responsive)
4. Generate index.html with post list, filters, search, social links, guides
5. Generate sitemap.xml, robots.txt, feed.xml, 404.html
6. Inject Google Analytics (if configured)
7. Minify CSS, inline into HTML, minify HTML (preserves pre/script/style blocks)

### SEO Output

Every page gets: `<title>`, `<meta description>`, `<link rel="canonical">`, Open Graph tags, Twitter Card, JSON-LD schema (BlogPosting/Blog), `<time datetime>`. Plus sitemap.xml, robots.txt, feed.xml (RSS 2.0 with Atom self-link).

### Template Override

`resolve_file()` checks `blogs/<name>/templates/` first, falls back to `engine/templates/`. Same for `style.css`. Blogs inherit defaults unless they explicitly override.

### Blog-to-Repo Symlink Pattern

External blogs use symlinks to connect devtui to their git repos. Example for `leandronsp.com`:

```
blogs/leandronsp.com/
  blog.toml              # config (title, url, etc.)
  posts -> ../../../leandronsp.com/articles   # symlink to repo
  images -> ../../../leandronsp.com/images    # symlink to repo
  uploads -> ../../../leandronsp.com/uploads  # symlink to repo
```

The symlinks mean the engine reads directly from the repo without copying source files into devtui.

### Build and Deploy Flow

`make deploy.git.<name>` builds and prepares a commit in the blog's repo. Push is manual.

```
repo/articles/*.md ─── symlink ──> engine reads markdown
repo/images/*      ─── symlink ──> engine copies to dist (round-trip)
repo/uploads/*     ─── symlink ──> engine copies to dist (round-trip)
                                        │
                                   dist/<name>/
                                   ├── articles/*.html  (generated)
                                   ├── index.html       (generated)
                                   ├── feed.xml         (generated)
                                   ├── sitemap.xml      (generated)
                                   ├── robots.txt       (generated)
                                   ├── images/*         (copied from repo)
                                   └── uploads/*        (copied from repo)
                                        │
                               rsync ───┘
                                        │
                                   repo/ (committed)
                                        │
                                   git push (manual) -> auto-deploy
```

Static assets (images, uploads) do a round-trip: they live in the repo, get copied to dist via symlink during build, and get copied back to the repo during deploy. This is redundant but harmless. It ensures new assets added to `blogs/<name>/images/` also reach the repo.

The repo path is resolved automatically by following the `posts` symlink (`REPO_DIR` in Makefile). The `rsync -a` uses trailing slashes on both source and destination to copy contents without creating nested directories.

## Key Gotchas

- **`-c` with `|` inside autocmd**: The pipe is part of the autocmd in vim, not a command separator. Use separate `-c` flags.
- **`vim -u NONE`** needs explicit config (tabstop, expandtab, noswapfile). Without it, tab is 8 spaces.
- **`writefile()` is synchronous with fsync**. Use CursorHold (debounced) instead of TextChanged for I/O.
- **Code blocks from pulldown-cmark**: `Event::Text` inside `Tag::CodeBlock` delivers entire block as one string with `\n`. Split on `\n` and push each line separately.
- **Rendered lines != source lines**: Headings add blank lines, lists add spacing. Offset map drifts over long documents.
- **`screen.title()`**: Real-time position from vim's titlestring via OSC escape sequences. Zero file I/O.
- **`shortmess=aFIoOstTWcCS`** via `--cmd` (before file load) to suppress vim messages.
- **Blog frontmatter quoting**: Some blogs quote values (`title: "My Title"`), others don't. The `frontmatter()` function strips both.
- **Lists without blank lines**: dev.to markdown has lists/blockquotes without preceding blank lines. `post_body()` preprocesses to insert blank lines before `* `, `- `, `> ` markers.
- **Emoji shortcodes**: pandoc `+emoji` extension converts `:wave:` etc. to unicode.
- **CSS order matters**: `@media` queries must come AFTER base rules in the CSS. The minifier preserves `<style>` blocks during HTML minification.
- **Incremental builds**: compare mtime of .md vs .html. If html is newer than both md and template, skip pandoc. Index/sitemap/feed always rebuild.

## Controls

All vim keybindings work:

- `i/a/A/I/o/O` — Enter insert mode
- `Esc` — Normal mode
- `hjkl` — Navigation
- `G/gg` — Bottom/top
- `Ctrl+D/Ctrl+U` — Half-page scroll
- `:w` — Save (message auto-cleared)
- `:q` / `:wq` — Quit
