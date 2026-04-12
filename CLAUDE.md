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
- Crates: ratatui, crossterm, pulldown-cmark, tui-term, vt100, portable-pty, serde_json
- **overmind** — optional, enables the scribe writing companion (AI annotations)

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
- **`src/editor/scribe.rs`** — AI writing companion: `Annotation`, `Tier`, `ScribeState` state machine, `build_check_prompt()`, `extract_annotations()`, `render_lines_with_focus()`. 35 tests.
- **`src/editor/ops.rs`** — Overmind integration: `start_scribe_session()`, `send_to_scribe()`, `run_subscriber()`, `escape_for_overmind()`, `kill_scribe_session()`. 18 tests.

### Engine (src/engine/)

DDD module structure. Each module is self-contained with unit tests.

- **`src/engine/build.rs`** — Thin pipeline orchestrator. 47 integration tests.
- **`src/engine/config.rs`** — `BlogConfig`, `Post`, `frontmatter()`, `post_body()`, `collect_posts()`, `resolve_og_image()`, `twitter_card()`. 20 tests.
- **`src/engine/template.rs`** — `resolve_file()`, `template_render()` with `$var$` and `$if()$...$endif$`. 9 tests.
- **`src/engine/index.rs`** — Index page assembly: nav, post list, footer, filter script.
- **`src/engine/feed.rs`** — RSS feed generation (`rss_header()`, `rss_item()`, `generate()`). 7 tests.
- **`src/engine/seo.rs`** — `xml_escape()`, `sitemap_entry()`, `robots_txt()`, `sitemap()`, `generate_files()`. 6 tests.
- **`src/engine/analytics.rs`** — Google Analytics injection. 2 tests.
- **`src/engine/minify.rs`** — CSS compilation, CSS/HTML minification, CSS inlining. 8 tests.
- **`src/engine/links.rs`** — Social links, tags, guides HTML from BlogConfig. 6 tests.
- **`src/engine/markdown.rs`** — Markdown-to-HTML (pulldown-cmark), `post_snippet()`, emoji shortcodes. 19 tests.
- **`src/engine/templates/`** — Default HTML templates.
- **`src/engine/themes/<name>/`** — Theme CSS, split into modular files.

### Themes (src/engine/themes/)

Themes provide modular CSS split into files concatenated in order:

```
src/engine/themes/<name>/
  base.css        # variables, reset, body defaults (line-height, font-size)
  index.css       # index page styles
  article.css     # article page styles
  syntax.css      # code syntax highlighting
  responsive.css  # mobile breakpoints
```

Blog selects theme via `theme = "paper"` in `blog.toml`. Default theme is `paper`. The build concatenates the CSS files, minifies, and inlines into each HTML.

**To change CSS, edit the theme files under `src/engine/themes/<name>/`. There is no fallback CSS file.**

### Blog Content (blogs/, gitignored)

- **`blogs/<name>/blog.toml`** — Site config (title, subtitle, url, author, lang, theme).
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

# Blog (mk/blog.mk)
make blog.list                 # list blogs
make blog.build                # build all blogs
make blog.build.<name>         # build one blog
make blog.serve.<name>         # build and serve on localhost:8000
make blog.clean                # remove dist/
make deploy.cp.<name>         # build, copy to repo

# All
make help                      # all targets
make test                      # all tests + lint
make lint                      # clippy
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

### Scribe (AI Writing Companion)

The right pane can swap between preview and scribe (Ctrl+T). Scribe sends the visible portion of the document to an AI model via overmind and displays grammar/spelling/factual annotations.

**Architecture:**

```
DevTUI process
  ├── Subscriber thread (long-lived)
  │     └── reads `overmind subscribe <session>` NDJSON events
  │     └── pushes assistant text into shared result slot
  │
  └── Main loop (per check)
        └── Sender thread: `overmind send <session> <prompt>` (fire-and-forget)
        └── poll_result() picks up response from subscriber
        └── extract_annotations() parses JSON into Annotation structs
```

**Key design decisions:**

- **Persistent session.** One `overmind run --type session` per DevTUI process. Survives across articles. Killed only on process exit.
- **Subscriber, not polling.** A long-running thread reads `overmind subscribe` events. No polling loop, no log parsing. Response arrives the instant the AI finishes.
- **Visible portion only.** Only lines currently visible on screen are sent (not the full document). Keeps prompts small (~2KB) and responses fast.
- **Line-numbered content.** Each line is prefixed with its absolute line number so the AI returns correct line references.
- **Newline escaping.** `overmind send` silently drops multiline arguments. `escape_for_overmind()` replaces `\n` with literal `\n`. The AI interprets them correctly.
- **Haiku model.** Uses claude-haiku for speed (~8s response with warm session vs ~15s with sonnet).
- **Concise prompt.** Max 15 words per annotation message. Two tiers: `error` (typo/grammar, show fix) and `hint` (phrasing/factual).

**State machine (`ScribeState`):**

- `Idle` — waiting for content to change
- `Checking` — prompt sent, waiting for subscriber to deliver response
- `CheckingSlow` — checking for >15 seconds
- `Error` — last check failed

**Gotchas:**

- `overmind send` silently drops multiline CLI arguments. Always escape with `escape_for_overmind()`.
- `overmind subscribe` streams events for tasks (`claude run`) but NOT for `send` messages to sessions. The subscriber thread must be started before the first send.
- `session_started` is only set after successful `start_scribe_session`, not in `begin_check`. This allows recovery if session start fails.
- `poll_result` guards on `pending` to prevent ghost results from delayed subscriber events after `clear_display`.
- `content_invalidated()` clears annotations and resets the idle timer. Called on every content change.

## How the Engine Works

### Blog Config

Each blog has a `blog.toml` with:
- `title`, `subtitle` — displayed on index and in meta tags
- `url` — canonical base URL for SEO
- `author` — used in meta tags and JSON-LD
- `lang` — HTML lang attribute
- `theme` — CSS theme (default: `paper`). Available: `paper`, `terminal`
- `og_image` — Open Graph image URL for social media previews (optional, site-wide default)
- `analytics_id` — Google Analytics measurement ID (optional, lazy-loaded)
- `license`, `license_url` — footer license link (optional)
- `tags` — curated tag list for index filter (optional)
- `[[links]]` — social links in header nav (label + url)
- `[[guides]]` — guide badges in header (title + url)

### Build Pipeline

1. Read `blog.toml` config
2. For each post: extract frontmatter, preprocess markdown (fix lists/blockquotes), render HTML via pulldown-cmark with article template. **Incremental**: skip if html is newer than md and template.
3. Concatenate theme CSS modules (base, index, article, syntax, responsive)
4. Generate index.html with post list, filters, search, social links, guides
5. Generate sitemap.xml, robots.txt, feed.xml, 404.html
6. Inject Google Analytics (if configured)
7. Minify CSS, inline into HTML, minify HTML (preserves pre/script/style blocks)

### SEO Output

Every page gets: `<title>`, `<meta description>`, `<link rel="canonical">`, Open Graph tags (including `og:image`), Twitter Card (`summary_large_image` when image present), JSON-LD schema (BlogPosting/Blog with optional `image`), `<time datetime>`. Plus sitemap.xml, robots.txt, feed.xml (RSS 2.0 with Atom self-link). Post frontmatter `image` overrides site-level `og_image`.

### Template Override

`resolve_file()` checks `blogs/<name>/templates/` first, falls back to `src/engine/templates/`. Same for `style.css`. Blogs inherit defaults unless they explicitly override.

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

`make deploy.cp.<name>` builds and copies dist to the blog's repo via rsync. Commit and push are manual.

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
- **Emoji shortcodes**: `replace_emoji_shortcodes()` in markdown.rs converts `:wave:` etc. to unicode spans via gh-emoji crate.
- **CSS order matters**: `@media` queries must come AFTER base rules in the CSS. The minifier preserves `<style>` blocks during HTML minification.
- **Incremental builds**: compare mtime of .md vs .html. If html is newer than both md and template, skip rebuild. Index/sitemap/feed always rebuild.

## Controls

All vim keybindings work:

- `i/a/A/I/o/O` — Enter insert mode
- `Esc` — Normal mode
- `hjkl` — Navigation
- `G/gg` — Bottom/top
- `Ctrl+D/Ctrl+U` — Half-page scroll
- `:w` — Save (message auto-cleared)
- `:q` / `:wq` — Quit
- `Ctrl+T` — Toggle scribe panel (AI writing companion)
- `Ctrl+G` — Cycle layouts: Vertical preview -> Scribe -> Editor only
