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
- `Ctrl+T` -- toggle scribe panel (AI writing companion)
- `Ctrl+G` -- cycle layouts: preview / scribe / editor only
- `:w` -- save, `:q` -- quit

### Scribe (AI Writing Companion)

Toggle with `Ctrl+T`. The scribe panel sends the visible portion of your document to an AI model and displays grammar, spelling, and factual annotations in real time.

- Annotations appear as the AI responds (no polling, event-streamed via overmind subscribe)
- Error tier: typos, grammar, wrong words. Shows the fix.
- Hint tier: awkward phrasing, factual issues. Brief suggestions.
- Visible lines only. Annotations reference absolute line numbers.
- Session persists across articles within a DevTUI instance.
- Clears automatically on content change and article switch.

Requires [overmind](https://github.com/leandronsp/overmind) installed. The scribe panel is hidden if overmind is not available.

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
- [serde_json](https://docs.rs/serde_json) -- annotation JSON parsing (scribe)
- [overmind](https://github.com/leandronsp/overmind) -- AI agent orchestrator (scribe, optional)

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
