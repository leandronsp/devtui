#!/usr/bin/env bats

setup() {
  source "$BATS_TEST_DIRNAME/../lib.sh"
  FIXTURES="$BATS_TEST_DIRNAME/fixtures"
  mkdir -p "$FIXTURES"

  # Create a test blog.toml
  cat > "$FIXTURES/blog.toml" << 'EOF'
title = "Test Blog"
subtitle = "a test subtitle"
url = "https://test.com"
author = "Test Author"
date_field = "date"
lang = "en"
EOF

  # Create a test markdown post
  cat > "$FIXTURES/post.md" << 'EOF'
---
title: My Test Post
date: 2026-03-29
description: A test description
tags: ["rust", "elixir"]
---

Some content here.
EOF

  # Create a post with published_at (different date field)
  cat > "$FIXTURES/post-alt.md" << 'EOF'
---
title: "Alt Date Post"
published_at: "2024-01-15 03:32:44Z"
description: "Alt description"
---

Content.
EOF

  # Post without description (like leandronsp.com articles)
  cat > "$FIXTURES/post-no-desc.md" << 'EOF'
---
title: "No Desc Post"
published_at: "2024-01-15"
---

Content without description.
EOF

  # Post with horizontal rules in body
  cat > "$FIXTURES/post-with-hr.md" << 'EOF'
---
title: "HR Post"
date: 2026-01-01
---

Before rule

---

After rule
EOF
}

teardown() {
  rm -rf "$FIXTURES"
}

# --- cfg ---

@test "cfg: reads title from toml" {
  result="$(cfg title "$FIXTURES/blog.toml")"
  [[ "$result" == "Test Blog" ]]
}

@test "cfg: reads subtitle from toml" {
  result="$(cfg subtitle "$FIXTURES/blog.toml")"
  [[ "$result" == "a test subtitle" ]]
}

@test "cfg: reads url from toml" {
  result="$(cfg url "$FIXTURES/blog.toml")"
  [[ "$result" == "https://test.com" ]]
}

@test "cfg: reads author from toml" {
  result="$(cfg author "$FIXTURES/blog.toml")"
  [[ "$result" == "Test Author" ]]
}

@test "cfg: reads lang from toml" {
  result="$(cfg lang "$FIXTURES/blog.toml")"
  [[ "$result" == "en" ]]
}

# --- resolve_file ---

@test "resolve_file: uses blog override when it exists" {
  mkdir -p "$FIXTURES/blog-templates"
  touch "$FIXTURES/blog-templates/article.html"
  result="$(resolve_file article.html "$FIXTURES/blog-templates" "$FIXTURES/engine-templates")"
  [[ "$result" == "$FIXTURES/blog-templates/article.html" ]]
}

@test "resolve_file: falls back to engine default" {
  mkdir -p "$FIXTURES/engine-templates"
  touch "$FIXTURES/engine-templates/article.html"
  result="$(resolve_file article.html "$FIXTURES/nonexistent" "$FIXTURES/engine-templates")"
  [[ "$result" == "$FIXTURES/engine-templates/article.html" ]]
}

# --- frontmatter ---

@test "frontmatter: extracts title" {
  result="$(frontmatter title "$FIXTURES/post.md")"
  [[ "$result" == "My Test Post" ]]
}

@test "frontmatter: extracts date" {
  result="$(frontmatter date "$FIXTURES/post.md")"
  [[ "$result" == "2026-03-29" ]]
}

@test "frontmatter: extracts description" {
  result="$(frontmatter description "$FIXTURES/post.md")"
  [[ "$result" == "A test description" ]]
}

@test "frontmatter: extracts quoted title" {
  result="$(frontmatter title "$FIXTURES/post-alt.md")"
  [[ "$result" == "Alt Date Post" ]]
}

@test "frontmatter: extracts published_at date" {
  result="$(frontmatter published_at "$FIXTURES/post-alt.md")"
  [[ "$result" == "2024-01-15 03:32:44Z" ]]
}

@test "frontmatter_date: strips time from published_at" {
  result="$(frontmatter_date published_at "$FIXTURES/post-alt.md")"
  [[ "$result" == "2024-01-15" ]]
}

@test "frontmatter_date: keeps date-only values unchanged" {
  result="$(frontmatter_date date "$FIXTURES/post.md")"
  [[ "$result" == "2026-03-29" ]]
}

@test "frontmatter: extracts quoted description" {
  result="$(frontmatter description "$FIXTURES/post-alt.md")"
  [[ "$result" == "Alt description" ]]
}

@test "frontmatter: returns empty for missing field" {
  result="$(frontmatter description "$FIXTURES/post-no-desc.md")"
  [[ "$result" == "" ]]
}

# --- post_body ---

@test "post_body: extracts content after frontmatter" {
  result="$(post_body "$FIXTURES/post.md")"
  [[ "$result" == *"Some content here."* ]]
}

@test "post_body: does not include frontmatter" {
  result="$(post_body "$FIXTURES/post.md")"
  [[ "$result" != *"title:"* ]]
}

@test "post_body: handles horizontal rules in content" {
  result="$(post_body "$FIXTURES/post-with-hr.md")"
  [[ "$result" == *"Before rule"* ]]
  [[ "$result" == *"---"* ]]
  [[ "$result" == *"After rule"* ]]
}

# --- sitemap_entry ---

@test "sitemap_entry: generates valid xml" {
  result="$(sitemap_entry "https://test.com/post.html" "2026-03-29")"
  [[ "$result" == '<url><loc>https://test.com/post.html</loc><lastmod>2026-03-29</lastmod></url>' ]]
}

# --- robots_txt ---

@test "robots_txt: contains user-agent allow and sitemap" {
  result="$(robots_txt "https://test.com")"
  [[ "$result" == *"User-agent: *"* ]]
  [[ "$result" == *"Allow: /"* ]]
  [[ "$result" == *"Sitemap: https://test.com/sitemap.xml"* ]]
}

# --- rss_header ---

@test "rss_header: contains channel title" {
  result="$(rss_header "My Blog" "https://test.com" "desc")"
  [[ "$result" == *"<title>My Blog</title>"* ]]
}

@test "rss_header: contains channel link" {
  result="$(rss_header "My Blog" "https://test.com" "desc")"
  [[ "$result" == *"<link>https://test.com</link>"* ]]
}

@test "rss_header: contains atom self link" {
  result="$(rss_header "My Blog" "https://test.com" "desc")"
  [[ "$result" == *'href="https://test.com/feed.xml"'* ]]
}

# --- rss_item ---

@test "rss_item: contains title and link" {
  result="$(rss_item "Post" "https://test.com/post.html" "desc" "2026-03-29")"
  [[ "$result" == *"<title>Post</title>"* ]]
  [[ "$result" == *"<link>https://test.com/post.html</link>"* ]]
}

@test "rss_item: contains guid" {
  result="$(rss_item "Post" "https://test.com/post.html" "desc" "2026-03-29")"
  [[ "$result" == *"<guid>https://test.com/post.html</guid>"* ]]
}

@test "rss_item: contains pubDate" {
  result="$(rss_item "Post" "https://test.com/post.html" "desc" "2026-03-29")"
  [[ "$result" == *"<pubDate>2026-03-29</pubDate>"* ]]
}

# --- template_sub ---

@test "template_sub: replaces single variable" {
  echo 'Hello $name$' > "$FIXTURES/tpl.html"
  result="$(template_sub "$FIXTURES/tpl.html" name "World")"
  [[ "$result" == "Hello World" ]]
}

@test "template_sub: replaces multiple variables" {
  echo '<title>$title$</title><p>$subtitle$</p>' > "$FIXTURES/tpl.html"
  result="$(template_sub "$FIXTURES/tpl.html" title "My Blog" subtitle "hello world")"
  [[ "$result" == "<title>My Blog</title><p>hello world</p>" ]]
}

@test "template_sub: leaves unmatched variables" {
  echo '$title$ and $other$' > "$FIXTURES/tpl.html"
  result="$(template_sub "$FIXTURES/tpl.html" title "Replaced")"
  [[ "$result" == 'Replaced and $other$' ]]
}
