# Blog Engine Conventions

## Structure

```
src/engine/          # Rust modules + static assets
  build.rs           # thin pipeline orchestrator + integration tests
  config.rs          # BlogConfig, Post, frontmatter(), collect_posts()
  template.rs        # resolve_file(), template_render()
  index.rs           # index page assembly (nav, posts, footer, filter script)
  feed.rs            # RSS feed generation
  seo.rs             # sitemap, robots.txt, 404, xml_escape
  analytics.rs       # Google Analytics injection
  minify.rs          # CSS compilation, minification, inlining
  links.rs           # social links, tags, guides HTML
  markdown.rs        # markdown-to-HTML, post_snippet(), emoji
  mod.rs             # module declarations
  templates/         # default HTML templates
  themes/<name>/     # theme CSS (base, index, article, syntax, responsive)

blogs/               # gitignored, user-managed content
  <name>/blog.toml   # config per blog
  <name>/posts/      # markdown posts
  <name>/templates/  # optional overrides

dist/                # gitignored, generated output
```

## Blog Config (blog.toml)

Required fields: `title`, `subtitle`, `url`, `author`, `date_field`, `lang`.

Optional: `og_image` (site-wide Open Graph image URL for social previews).

The `date_field` allows different blogs to use different frontmatter keys for dates (e.g. `date` vs `published_at`).

## Posts

- YAML frontmatter required: `title`, the date field
- `description` is optional. If missing, auto-generated from post body (160 chars, word boundary)
- `image` is optional. Per-post Open Graph image URL (overrides site `og_image`)
- Filename becomes the URL slug
- pulldown-cmark uses language hints from fenced code blocks

## Template Override

`resolve_file()` checks blog directory first, falls back to theme, then engine default. This applies to templates.

## SEO

Every build generates: canonical URLs, Open Graph, Twitter Card, JSON-LD (BlogPosting/Blog), semantic `<time datetime>`, sitemap.xml, robots.txt, feed.xml (RSS 2.0).

## Testing

- `make blog.test` runs all engine tests (123 tests, ~1s)
- Unit tests in each `src/engine/*.rs` module with `#[cfg(test)]`
- Integration tests in `src/engine/build.rs` (full pipeline with temp dirs)
- Every new engine function needs tests. Every new output artifact needs integration assertions.
- `make test` runs all tests (149 total)

## Adding a new engine module

1. Create `src/engine/<module>.rs` with functions and `#[cfg(test)]` tests
2. Add `pub mod <module>;` to `src/engine/mod.rs`
3. If the module produces output files, add integration tests in `src/engine/build.rs`
