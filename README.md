# DevTUI

Terminal markdown editor with live preview, AI writing companion, and multi-site static blog engine. Built in Rust with ratatui.

## Editor

Embeds vim via PTY with real-time markdown preview. Left pane is full vim. Right pane renders markdown as you type.

```bash
make editor.cms.<name>                                   # CMS list view for a blog (k9s-style)
make editor.cms.acme-alchemist                           # example
make editor.cms.leandronsp.com                           # example
make editor.run FILE=mypost.md                           # edit a single markdown file
make editor.build                                        # release build only
make editor.test                                         # editor tests
```

Run `make blog.list` to see available blog names.

Position sync uses vim's `titlestring` (zero file I/O). Content sync debounced via `CursorHold`. Mode detected from the terminal title.

### Controls

- All vim keybindings work
- `Ctrl+D`/`Ctrl+U` -- half-page scroll
- `G`/`gg` -- top/bottom
- `Ctrl+P` -- toggle preview pane
- `Ctrl+Y` -- toggle chat pane (AI writing companion)
- `Ctrl+J`/`Ctrl+K` -- scroll right pane
- `:w` -- save, `:q` -- quit

### Chat (AI Writing Companion)

Toggle with `Ctrl+Y`. An interactive chat pane for grammar checking, writing advice, and vault-powered context search.

- Grammar and spelling on demand. Ask "check the grammar" and the AI reviews the visible section.
- Vault search. Questions automatically search your Obsidian vault (learning/, lives/, blog/ folders) via qmd for related notes.
- Full article context. The AI sees frontmatter + the visible section with margin.
- Bilingual. Works in English and Portuguese, answers in the same language as the question.
- Configurable provider. Defaults to Groq (llama-3.1-8b-instant). Set `DEVTUI_LLM_PROVIDER=claude` for Anthropic.

Requires `GROQ_API_KEY` or `ANTHROPIC_API_KEY` in environment. Optional: [qmd](https://github.com/leandronsp/qmd) for vault search.

## Blog Engine

Multi-site static blog generator. See [docs/BLOG_ENGINE.md](docs/BLOG_ENGINE.md) for full documentation.

```bash
make blog.list            # list available blogs
make blog.build           # build all blogs
make blog.build.<name>    # build one blog
make blog.serve.<name>    # build and serve on localhost:8000
make blog.clean           # remove dist/
make blog.theme.<name> THEME=paper|newspaper|terminal   # switch theme
make deploy.cp.<name>     # build and rsync to blog repo
```

### Showcase

[![leandronsp.com](assets/showcase-leandronsp.png)](https://leandronsp.com)

[leandronsp.com](https://leandronsp.com) -- built with DevTUI's blog engine using the `paper` theme.

## Development

```bash
make help                     # list all targets
make test                     # all tests + lint
make editor.test              # editor tests only
make blog.test                # engine tests only
make lint                     # clippy
```

## Dependencies

### Editor

- [ratatui](https://ratatui.rs/) + [crossterm](https://github.com/crossterm-rs/crossterm) -- TUI framework
- [portable-pty](https://docs.rs/portable-pty) + [vt100](https://docs.rs/vt100) + [tui-term](https://docs.rs/tui-term) -- vim PTY embedding
- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) -- markdown parsing
- [ureq](https://docs.rs/ureq) -- HTTP client for LLM API calls (chat companion)
- [qmd](https://github.com/leandronsp/qmd) -- Obsidian vault search (optional)

### Engine

- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) -- markdown to HTML
- [toml](https://docs.rs/toml) + [serde](https://serde.rs/) -- config parsing
- [gh-emoji](https://docs.rs/gh-emoji) -- emoji shortcode conversion
- [tiny_http](https://docs.rs/tiny_http) -- local dev server
- **rsync** -- deploy only

### Install (macOS)

```bash
brew install vim
# Rust toolchain via rustup
```

## License

AGPL-3.0
