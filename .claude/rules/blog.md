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

- `bats engine/tests/` runs all engine tests
- `engine/tests/lib.bats` — unit tests for each lib function
- `engine/tests/build.bats` — integration tests for full build output
- Every new lib function needs bats tests. Every new output artifact needs integration assertions.
- `make test` runs both cargo and bats (76 tests total)

## Adding a new lib module

1. Create `engine/lib/<module>.sh` with functions
2. Add `source "$LIB_DIR/<module>.sh"` to `engine/lib.sh`
3. Add unit tests in `engine/tests/lib.bats`
4. If the module produces output files, add integration tests in `engine/tests/build.bats`
