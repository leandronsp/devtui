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

### Setting up a blog

Create a directory in `blogs/` with a `blog.toml` and `posts/`:

```
blogs/my-site/
  blog.toml
  posts/
    2026-03-29-my-post.md
```

`blog.toml`:

```toml
title = "My Site"
subtitle = "a blog about things"
url = "https://my-site.com"
author = "Your Name"
date_field = "date"
lang = "en"
theme = "paper"
```

Optional fields: `articles_path`, `analytics_id`, `license`, `license_url`, `tags`, `[[links]]`, `[[guides]]`.

Posts use YAML frontmatter: `title`, the date field, `description`, `language`, `tags`.

### External repos

For blogs that live in their own git repo, symlink into `blogs/`:

```
blogs/leandronsp.com/
  blog.toml
  posts -> ../../../leandronsp.com/articles
  images -> ../../../leandronsp.com/images
  uploads -> ../../../leandronsp.com/uploads
```

### Themes

Two built-in themes:

- **paper** -- serif body, warm earthy palette, aged paper feel. Light default.
- **terminal** -- monospace everything, CRT aesthetic, `$` prompt, cursor blink, scanlines.

Themes are modular CSS split into `base.css`, `index.css`, `article.css`, `syntax.css`, `responsive.css`. Concatenated in order during build.

Set in `blog.toml`: `theme = "paper"`. Default is `paper`.

### Output per blog

```
dist/<name>/
  articles/*.html    # article pages with SEO meta tags
  index.html         # article listing with search, filters
  404.html           # not found page
  feed.xml           # RSS 2.0
  sitemap.xml
  robots.txt
  images/            # copied from blog
  uploads/           # copied from blog
```

Every page gets: `<title>`, `<meta description>`, canonical URL, Open Graph, Twitter Card, JSON-LD schema. CSS inlined and HTML minified.

### Features

- Incremental builds (skip unchanged articles)
- Language filter (all/en/pt from frontmatter)
- Tag filter (curated tags from blog.toml)
- Client-side search with clear button
- Mobile filter popover with pill buttons
- Social links and guide badges in header
- Google Analytics (lazy-loaded, optional)
- Footer with license and RSS link
- Emoji shortcode conversion
- List/blockquote preprocessing for dev.to imports
- Dark/light theme toggle with localStorage

### Example: leandronsp.com

Full pipeline from markdown to deployed blog:

```bash
# setup (one-time): symlink external repo into blogs/
mkdir -p blogs/leandronsp.com
cat > blogs/leandronsp.com/blog.toml << 'EOF'
title = "Leandro Proenca"
subtitle = "low-level curiosity, high-level pragmatism"
url = "https://leandronsp.com"
author = "Leandro Proenca"
date_field = "published_at"
lang = "en"
articles_path = "articles"
theme = "paper"
analytics_id = "G-0Y5RNLZMKN"
license = "CC BY-SA 4.0"
license_url = "https://creativecommons.org/licenses/by-sa/4.0/"
tags = ["ruby", "rust", "assembly", "bash", "postgres", "kubernetes", "docker"]

[[links]]
label = "github"
url = "https://github.com/leandronsp"

[[links]]
label = "linkedin"
url = "https://www.linkedin.com/in/leandronsp/"

[[guides]]
title = "Concorrencia 101"
url = "https://concorrencia101.leandronsp.com/"
EOF
ln -s ../../../leandronsp.com/articles blogs/leandronsp.com/posts
ln -s ../../../leandronsp.com/images blogs/leandronsp.com/images
ln -s ../../../leandronsp.com/uploads blogs/leandronsp.com/uploads

# build (incremental: skips unchanged articles)
make blog.build.leandronsp.com

# preview locally
make blog.serve.leandronsp.com

# deploy: rsync to repo, then push
make deploy.git.leandronsp.com
cd ../leandronsp.com && git push   # triggers Cloudflare Pages auto-deploy
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
