# SEO

Applies to generated blog HTML (engine templates, index pages, article pages).

## Meta tags

- `<title>` 50-60 chars. Articles: "Post Title - Site Name". Index: site name only.
- `<meta name="description">` 150-160 chars. Use post `description` from frontmatter.
- `<link rel="canonical">` on every page
- Open Graph: `og:title`, `og:description`, `og:type` (article), `og:url`, `og:site_name`
- Twitter: `twitter:card` (summary). Falls back to OG tags.

## Semantic HTML

- One `<h1>` per page. Article title on post pages, site title on index.
- `<h2>` for sections, `<h3>` for subsections. pulldown-cmark preserves heading levels from markdown.
- Use landmarks: `<header>`, `<nav>`, `<main>`, `<article>`, `<footer>`
- `<time>` for dates with `datetime` attribute

## Performance

- No external fonts in default template. System font stack only.
- Single CSS file, no JS frameworks. Inline theme script is minimal.
- `async` or `defer` on any scripts
- Static HTML. No client-side rendering.

## Files

- `robots.txt` at root with sitemap reference
- `sitemap.xml` generated during build
- Favicon via `<link rel="icon">`

## Structured data

- JSON-LD `BlogPosting` schema on article pages
- JSON-LD `Blog` schema on index page

## What NOT to do

- Don't stuff keywords. Write for humans.
- Don't duplicate title/description across pages. Each post has unique metadata.
- Don't use JS for content rendering. All content is static HTML.
