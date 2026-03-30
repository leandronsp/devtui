# Blog Engine

Multi-site static blog generator. Each blog has its own config, posts, theme, and optional template overrides. Incremental builds skip unchanged articles.

```bash
make blog.list                 # list available blogs
make blog.build.<name>         # build one blog
make blog.build                # build all blogs
make blog.serve.<name>         # build and serve on localhost:8000
make blog.clean                # remove all generated files
make deploy.cp.<name>         # build and rsync to blog repo
```

## Getting started

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
make deploy.cp.my-site       # rsync dist to blog repo
cd ../my-site && git push     # auto-deploys via hosting provider
```

## Choosing a theme

Set `theme` in `blog.toml`:

| Theme | Style | Default mode | Best for |
|-------|-------|-------------|----------|
| **paper** | serif body, warm earthy tones, aged paper feel | light | long-form articles, readability |
| **terminal** | monospace, CRT scanlines, `$` prompt, cursor blink | dark | technical blogs, dev audience |

Both themes include dark/light toggle, mobile responsive popover, and identical SEO output.

Themes are modular CSS in `src/engine/themes/<name>/`:

```
base.css        # variables, reset, layout
index.css       # index page (search, filters, post list)
article.css     # article page (headings, code, blockquotes)
syntax.css      # code syntax highlighting colors
responsive.css  # mobile breakpoints
```

To create a custom theme, copy an existing one and edit the CSS files.

## Using external repos

For blogs that live in their own git repo, symlink into `blogs/`:

```
blogs/leandronsp.com/
  blog.toml
  posts -> ../../../leandronsp.com/articles
  images -> ../../../leandronsp.com/images
  uploads -> ../../../leandronsp.com/uploads
```

The engine reads markdown through the symlink and copies images/uploads to dist during build. On deploy, `rsync` copies everything back to the repo.

## Output per blog

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

## Features

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

## Example: leandronsp.com

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

# deploy: copies built files to blog repo
make deploy.cp.leandronsp.com

# review, commit and push manually
cd ../leandronsp.com
git diff --stat
git add -A && git commit -m "deploy: update site"
git push                          # triggers Cloudflare Pages auto-deploy
```
