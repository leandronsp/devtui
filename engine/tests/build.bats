#!/usr/bin/env bats

setup() {
  FIXTURES="$BATS_TEST_DIRNAME/fixtures"
  DIST="$BATS_TEST_DIRNAME/fixtures/dist"
  ENGINE="$BATS_TEST_DIRNAME/.."
  BLOG="$FIXTURES/test-blog"

  mkdir -p "$BLOG/posts"

  cat > "$BLOG/blog.toml" << 'EOF'
title = "Test Blog"
subtitle = "integration test blog"
url = "https://test-blog.com"
author = "Tester"
date_field = "date"
lang = "en"
EOF

  cat > "$BLOG/posts/2026-01-01-hello.md" << 'EOF'
---
title: Hello World
date: 2026-01-01
description: First post
---

Hello **world**.
EOF

  cat > "$BLOG/posts/2026-01-02-second.md" << 'EOF'
---
title: Second Post
date: 2026-01-02
description: Another post
---

Content here.
EOF

  run "$ENGINE/build.sh" "$BLOG" "$DIST"
  [ "$status" -eq 0 ]
}

teardown() {
  rm -rf "$FIXTURES"
}

# --- articles ---

@test "build: generates article html files" {
  [ -f "$DIST/2026-01-01-hello.html" ]
  [ -f "$DIST/2026-01-02-second.html" ]
}

@test "build: article has correct title tag" {
  grep -q '<title>Hello World - Test Blog</title>' "$DIST/2026-01-01-hello.html"
}

@test "build: article has canonical url" {
  grep -q 'rel="canonical" href="https://test-blog.com/2026-01-01-hello.html"' "$DIST/2026-01-01-hello.html"
}

@test "build: article has open graph tags" {
  grep -q 'og:title' "$DIST/2026-01-01-hello.html"
  grep -q 'og:description' "$DIST/2026-01-01-hello.html"
  grep -q 'og:url' "$DIST/2026-01-01-hello.html"
  grep -q 'og:type.*article' "$DIST/2026-01-01-hello.html"
}

@test "build: article has json-ld schema" {
  grep -q 'BlogPosting' "$DIST/2026-01-01-hello.html"
  grep -q '"author"' "$DIST/2026-01-01-hello.html"
}

@test "build: article has semantic time element" {
  grep -q 'datetime="2026-01-01"' "$DIST/2026-01-01-hello.html"
}

@test "build: article has nav link back to index" {
  grep -q 'href="index.html"' "$DIST/2026-01-01-hello.html"
}

@test "build: article has site title in nav" {
  grep -q '>Test Blog<' "$DIST/2026-01-01-hello.html"
}

# --- index ---

@test "build: generates index.html" {
  [ -f "$DIST/index.html" ]
}

@test "build: index has correct title" {
  grep -q '<title>Test Blog</title>' "$DIST/index.html"
}

@test "build: index has h1 with site title" {
  grep -q '<h1 class="site-title">Test Blog</h1>' "$DIST/index.html"
}

@test "build: index lists both posts" {
  grep -q 'Hello World' "$DIST/index.html"
  grep -q 'Second Post' "$DIST/index.html"
}

@test "build: index has canonical url" {
  grep -q 'rel="canonical" href="https://test-blog.com/"' "$DIST/index.html"
}

@test "build: index has json-ld blog schema" {
  grep -q '"Blog"' "$DIST/index.html"
}

# --- sitemap ---

@test "build: generates sitemap.xml" {
  [ -f "$DIST/sitemap.xml" ]
}

@test "build: sitemap has index url" {
  grep -q '<loc>https://test-blog.com/</loc>' "$DIST/sitemap.xml"
}

@test "build: sitemap has article urls" {
  grep -q 'test-blog.com/2026-01-01-hello.html' "$DIST/sitemap.xml"
  grep -q 'test-blog.com/2026-01-02-second.html' "$DIST/sitemap.xml"
}

@test "build: sitemap has lastmod dates" {
  grep -q '<lastmod>2026-01-01</lastmod>' "$DIST/sitemap.xml"
  grep -q '<lastmod>2026-01-02</lastmod>' "$DIST/sitemap.xml"
}

# --- robots ---

@test "build: generates robots.txt" {
  [ -f "$DIST/robots.txt" ]
}

@test "build: robots.txt references sitemap" {
  grep -q 'Sitemap: https://test-blog.com/sitemap.xml' "$DIST/robots.txt"
}

# --- rss ---

@test "build: generates feed.xml" {
  [ -f "$DIST/feed.xml" ]
}

@test "build: feed.xml has channel title" {
  grep -q '<title>Test Blog</title>' "$DIST/feed.xml"
}

@test "build: feed.xml has atom self link" {
  grep -q 'href="https://test-blog.com/feed.xml"' "$DIST/feed.xml"
}

@test "build: feed.xml has items for each post" {
  grep -q '<title>Hello World</title>' "$DIST/feed.xml"
  grep -q '<title>Second Post</title>' "$DIST/feed.xml"
}

@test "build: feed.xml items have correct links" {
  grep -q '<link>https://test-blog.com/2026-01-01-hello.html</link>' "$DIST/feed.xml"
}

# --- style ---

@test "build: copies style.css" {
  [ -f "$DIST/style.css" ]
}
