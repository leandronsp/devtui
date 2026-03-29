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

Generic static site generator. Supports multiple blogs, each with its own config, posts, and optional template/style overrides.

```bash
make blog.list                     # list available blogs
make blog.build                    # build all blogs
make blog.build.acme-alchemist     # build one blog
make blog.serve.acme-alchemist     # build and serve on localhost:8000
make blog.clean                    # remove all generated files
```

### Setting up a blog

Create a directory in `blogs/` (gitignored) with a `blog.toml` and `posts/`:

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
```

Posts use YAML frontmatter with `title`, `date` (or whatever `date_field` is), and `description`.

For external repos (like leandronsp.com), symlink into `blogs/`:

```bash
ln -s ../../leandronsp.com blogs/leandronsp.com
```

### Output per blog

Each blog generates into `dist/<blog-name>/`:

- Article HTML pages with SEO meta tags
- `index.html` with article listing
- `style.css` (dark/light theme toggle)
- `sitemap.xml`
- `robots.txt`
- `feed.xml` (RSS)

### Template overrides

Blogs can override default templates and CSS by placing files in their own directory:

- `blogs/my-site/templates/article.html` overrides `engine/templates/article.html`
- `blogs/my-site/style.css` overrides `engine/style.css`

## Development

```bash
make test                     # cargo tests + bats engine tests (76 total)
cargo test                    # 26 preview rendering tests
bats engine/tests/            # 50 engine tests (unit + integration)
cargo clippy -- -D warnings   # lint
```

## Dependencies

- [ratatui](https://ratatui.rs/) + [crossterm](https://github.com/crossterm-rs/crossterm) — TUI framework
- [portable-pty](https://docs.rs/portable-pty) + [vt100](https://docs.rs/vt100) + [tui-term](https://docs.rs/tui-term) — Vim PTY embedding
- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) — Markdown parsing
- [pandoc](https://pandoc.org/) — Blog HTML generation
- [bats](https://github.com/bats-core/bats-core) — Engine shell tests
