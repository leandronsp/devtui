# Blog Engine Conventions

## Structure

```
engine/              # generic, no blog-specific content
  build.sh           # orchestrator
  lib.sh             # module loader
  lib/config.sh      # cfg(), frontmatter()
  lib/template.sh    # resolve_file(), template_sub()
  lib/seo.sh         # sitemap, robots, rss functions
  templates/         # default templates
  style.css          # default CSS
  tests/             # bats tests

blogs/               # gitignored, user-managed content
  <name>/blog.toml   # config per blog
  <name>/posts/      # markdown posts
  <name>/templates/  # optional overrides
  <name>/style.css   # optional override

dist/                # gitignored, generated output
```

## Blog Config (blog.toml)

Required fields: `title`, `subtitle`, `url`, `author`, `date_field`, `lang`.

The `date_field` allows different blogs to use different frontmatter keys for dates (e.g. `date` vs `published_at`).

## Posts

- YAML frontmatter required: `title`, the date field, `description`
- Filename becomes the URL slug
- pandoc auto-detects code block language from fence markers (140+ languages)

## Template Override

`resolve_file()` checks blog directory first, falls back to engine default. This applies to both templates and style.css.

## SEO

Every build generates: canonical URLs, Open Graph, Twitter Card, JSON-LD (BlogPosting/Blog), semantic `<time datetime>`, sitemap.xml, robots.txt, feed.xml (RSS 2.0).

## Testing

- `make blog.test` runs all engine tests (119 tests, ~1s)
- Unit tests in each `src/engine/*.rs` module with `#[cfg(test)]`
- Integration tests in `src/engine/build.rs` (full pipeline with temp dirs)
- Every new engine function needs tests. Every new output artifact needs integration assertions.
- `make test` runs all tests (145 total)

## Adding a new engine module

1. Create `src/engine/<module>.rs` with functions and `#[cfg(test)]` tests
2. Add `pub mod <module>;` to `src/engine/mod.rs`
3. If the module produces output files, add integration tests in `src/engine/build.rs`
