# CLAUDE.md

## Project Overview

DevTUI is three things:

1. **Editor** — Terminal markdown editor with live preview. Embeds vim via PTY with real-time rendered markdown preview. Built in Rust with ratatui.
2. **Engine** — Multi-site static blog generator. Converts markdown to SEO-optimized HTML via pandoc. Shell-based, tested with bats.
3. **CLI** — The glue. Makefile orchestrates builds across multiple blogs.

## Architecture

### Editor (src/)

- **`src/main.rs`** — PTY setup, vim spawn, event loop, scroll sync, mode detection
- **`src/preview.rs`** — Markdown to ratatui Lines rendering + 26 unit tests

Crates: portable-pty, vt100, tui-term, pulldown-cmark, ratatui, crossterm.

### Engine (engine/)

- **`engine/build.sh`** — Orchestrator. Reads blog config, builds articles, index, sitemap, robots, RSS.
- **`engine/lib.sh`** — Module loader.
- **`engine/lib/config.sh`** — `cfg()`, `frontmatter()`. Config and frontmatter parsing.
- **`engine/lib/template.sh`** — `resolve_file()`, `template_sub()`. Template resolution with blog override > engine default fallback.
- **`engine/lib/seo.sh`** — `sitemap_entry()`, `robots_txt()`, `rss_header()`, `rss_item()`. SEO artifact generation.
- **`engine/templates/`** — Default HTML templates (pandoc format).
- **`engine/style.css`** — Default CSS with dark/light themes.
- **`engine/tests/`** — 50 bats tests (24 unit + 26 integration).

### Blog Content (blogs/, gitignored)

- **`blogs/<name>/blog.toml`** — Site config (title, subtitle, url, author, date_field, lang).
- **`blogs/<name>/posts/`** — Markdown posts with YAML frontmatter.
- **`blogs/<name>/templates/`** — Optional template overrides.
- **`blogs/<name>/style.css`** — Optional CSS override.

Blogs can be local directories or symlinks to external repos.

### Output (dist/, gitignored)

Generated per blog: article HTMLs, index.html, style.css, sitemap.xml, robots.txt, feed.xml.

## Commands

```bash
# Editor
cargo run -- file.md           # run editor
cargo build --release          # release build
cargo test                     # 26 preview tests
cargo clippy -- -D warnings    # lint

# Engine
make help                      # all targets
make test                      # cargo + bats (76 tests)
make blog.list                 # list blogs
make blog.build                # build all blogs
make blog.build.<name>         # build one blog
make blog.serve.<name>         # build and serve on localhost:8000
make blog.clean                # remove dist/
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

### Build Pipeline

1. Read `blog.toml` config
2. For each post: extract frontmatter, run pandoc with article template, pass site variables
3. Generate index.html from template with variable substitution
4. Generate sitemap.xml, robots.txt, feed.xml from post metadata
5. Copy CSS (blog override or engine default)

### SEO Output

Every page gets: `<title>`, `<meta description>`, `<link rel="canonical">`, Open Graph tags, Twitter Card, JSON-LD schema (BlogPosting/Blog), `<time datetime>`. Plus sitemap.xml, robots.txt, feed.xml (RSS 2.0 with Atom self-link).

### Template Override

`resolve_file()` checks `blogs/<name>/templates/` first, falls back to `engine/templates/`. Same for `style.css`. Blogs inherit defaults unless they explicitly override.

## Key Gotchas

- **`-c` with `|` inside autocmd**: The pipe is part of the autocmd in vim, not a command separator. Use separate `-c` flags.
- **`vim -u NONE`** needs explicit config (tabstop, expandtab, noswapfile). Without it, tab is 8 spaces.
- **`writefile()` is synchronous with fsync**. Use CursorHold (debounced) instead of TextChanged for I/O.
- **Code blocks from pulldown-cmark**: `Event::Text` inside `Tag::CodeBlock` delivers entire block as one string with `\n`. Split on `\n` and push each line separately.
- **Rendered lines != source lines**: Headings add blank lines, lists add spacing. Offset map drifts over long documents.
- **`screen.title()`**: Real-time position from vim's titlestring via OSC escape sequences. Zero file I/O.
- **`shortmess=aFIoOstTWcCS`** via `--cmd` (before file load) to suppress vim messages.
- **Blog frontmatter quoting**: Some blogs quote values (`title: "My Title"`), others don't. The `frontmatter()` function strips both.

## Controls

All vim keybindings work:

- `i/a/A/I/o/O` — Enter insert mode
- `Esc` — Normal mode
- `hjkl` — Navigation
- `G/gg` — Bottom/top
- `Ctrl+D/Ctrl+U` — Half-page scroll
- `:w` — Save (message auto-cleared)
- `:q` / `:wq` — Quit
