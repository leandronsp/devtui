# DevTUI

Terminal markdown editor with live preview + multi-site static blog engine.

## Editor

Embeds vim via PTY with real-time markdown preview. Left pane is full vim. Right pane renders the markdown as you type.

```bash
cargo build --release
./target/release/devtui mypost.md
```

Position sync uses vim's `titlestring` (zero file I/O). Content sync debounced via `CursorHold`. Mode detected from the terminal title.

All vim keybindings work. `Ctrl+D`/`Ctrl+U` for half-page scroll, `G`/`gg` for top/bottom, `:w` to save, `:q` to quit.

## Blog Engine

Multi-site static blog generator. Each blog has its own config, posts, theme, and optional template overrides. Incremental builds skip unchanged articles.

```bash
make blog.list                 # list available blogs
make blog.build.<name>         # build one blog
make blog.build                # build all blogs
make blog.serve.<name>         # build and serve on localhost:8000
make blog.clean                # remove all generated files
make deploy.git.<name>         # build and rsync to blog repo
```

### Getting started

**1. Create your blog directory**

```bash
mkdir -p blogs/my-site/posts
```

**2. Write your config**

```toml
# blogs/my-site/blog.toml

# required
title = "My Site"
subtitle = "a blog about things"
url = "https://my-site.com"
author = "Your Name"
date_field = "date"
lang = "en"

# optional
theme = "paper"                    # "paper" (default) or "terminal"
articles_path = "articles"         # URL prefix for articles (default: root)
analytics_id = "G-XXXXXXXXXX"     # Google Analytics measurement ID
license = "CC BY-SA 4.0"
license_url = "https://creativecommons.org/licenses/by-sa/4.0/"
tags = ["ruby", "rust", "docker"] # curated tags for index filter

[[links]]                          # social links in header
label = "github"
url = "https://github.com/you"

[[guides]]                         # guide badges in header
title = "My Guide"
url = "https://guide.my-site.com"
```

**3. Write a post**

```markdown
# blogs/my-site/posts/2026-03-29-hello-world.md

---
title: Hello World
date: 2026-03-29
description: My first post
language: en
tags: ["intro"]
---

Welcome to my blog.
```

**4. Build and preview**

```bash
make blog.build.my-site       # generates dist/my-site/
make blog.serve.my-site       # serves on localhost:8000
```

**5. Deploy**

For git-based deploys (Cloudflare Pages, GitHub Pages, Netlify):

```bash
make deploy.git.my-site       # rsync dist to blog repo
cd ../my-site && git push     # auto-deploys via hosting provider
```

### Choosing a theme

Set `theme` in `blog.toml`:

| Theme | Style | Default mode | Best for |
|-------|-------|-------------|----------|
| **paper** | serif body, warm earthy tones, aged paper feel | light | long-form articles, readability |
| **terminal** | monospace, CRT scanlines, `$` prompt, cursor blink | dark | technical blogs, dev audience |

Both themes include dark/light toggle, mobile responsive popover, and identical SEO output.

Themes are modular CSS in `engine/themes/<name>/`:

```
base.css        # variables, reset, layout
index.css       # index page (search, filters, post list)
article.css     # article page (headings, code, blockquotes)
syntax.css      # code syntax highlighting colors
responsive.css  # mobile breakpoints
```

To create a custom theme, copy an existing one and edit the CSS files.

### Using external repos

For blogs that live in their own git repo, symlink into `blogs/`:

```
blogs/leandronsp.com/
  blog.toml
  posts -> ../../../leandronsp.com/articles
  images -> ../../../leandronsp.com/images
  uploads -> ../../../leandronsp.com/uploads
```

The engine reads markdown through the symlink and copies images/uploads to dist during build. On deploy, `rsync` copies everything back to the repo.

### Output per blog

```
dist/<name>/
  articles/*.html    # article pages with full SEO meta tags
  index.html         # article listing with search and filters
  404.html           # not found page
  feed.xml           # RSS 2.0 with Atom self-link
  sitemap.xml
  robots.txt
  images/            # copied from blog source
  uploads/           # copied from blog source
```

Every page gets: `<title>`, `<meta description>`, canonical URL, Open Graph, Twitter Card, JSON-LD schema. CSS is inlined and HTML is minified. No external stylesheets, no JavaScript frameworks.

### Features

- Incremental builds (skip unchanged articles)
- Language filter (all/en/pt from post frontmatter)
- Tag filter (curated tags from blog.toml)
- Client-side search with clear button
- Mobile filter popover with pill buttons
- Social links and guide badges in header
- Google Analytics (lazy-loaded 2s after page load, optional)
- Footer with license and RSS link
- Emoji shortcode conversion (`:wave:` becomes unicode)
- Markdown preprocessing for dev.to imports (lists, blockquotes)
- Dark/light theme toggle persisted in localStorage
- 404 page per blog

### Example: leandronsp.com

Full pipeline from markdown to production:

```bash
# one-time setup
mkdir -p blogs/leandronsp.com
ln -s ../../../leandronsp.com/articles blogs/leandronsp.com/posts
ln -s ../../../leandronsp.com/images blogs/leandronsp.com/images
ln -s ../../../leandronsp.com/uploads blogs/leandronsp.com/uploads
# create blog.toml with config...

# build (incremental: skips unchanged articles)
make blog.build.leandronsp.com

# preview
make blog.serve.leandronsp.com    # localhost:8000

# deploy to Cloudflare Pages
make deploy.git.leandronsp.com
cd ../leandronsp.com && git push  # triggers auto-deploy
```

## Development

```bash
make test                     # cargo tests + bats engine tests
cargo test                    # 26 preview rendering tests
bats engine/tests/            # 49 engine tests (unit + integration)
cargo clippy -- -D warnings   # lint
```

## Dependencies

### Editor

- [ratatui](https://ratatui.rs/) + [crossterm](https://github.com/crossterm-rs/crossterm) -- TUI framework
- [portable-pty](https://docs.rs/portable-pty) + [vt100](https://docs.rs/vt100) + [tui-term](https://docs.rs/tui-term) -- vim PTY embedding
- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) -- markdown parsing

### Engine

- [pandoc](https://pandoc.org/) -- markdown to HTML
- [dasel](https://github.com/TomWright/dasel) -- TOML config parsing
- [jq](https://jqlang.github.io/jq/) -- JSON processing
- **python3** -- minification, text processing
- **xmllint** -- feed.xml validation
- **rsync** -- static asset copying and deploy

### Testing

- [bats](https://github.com/bats-core/bats-core) -- engine shell tests

### Install (macOS)

```bash
brew install vim pandoc dasel jq bats-core
# python3 and xmllint are pre-installed on macOS
```

## License

AGPL-3.0
