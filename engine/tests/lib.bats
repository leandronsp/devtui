#!/usr/bin/env bats

setup() {
  source "$BATS_TEST_DIRNAME/../lib.sh"
  FIXTURES="$(mktemp -d)"

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
  mkdir -p "$FIXTURES/blog-templates" "$FIXTURES/theme-templates" "$FIXTURES/engine-templates"
  touch "$FIXTURES/blog-templates/article.html"
  result="$(resolve_file article.html "$FIXTURES/blog-templates" "$FIXTURES/theme-templates" "$FIXTURES/engine-templates")"
  [[ "$result" == "$FIXTURES/blog-templates/article.html" ]]
}

@test "resolve_file: falls back to theme when no blog override" {
  mkdir -p "$FIXTURES/theme-templates" "$FIXTURES/engine-templates"
  touch "$FIXTURES/theme-templates/style.css"
  result="$(resolve_file style.css "$FIXTURES/nonexistent" "$FIXTURES/theme-templates" "$FIXTURES/engine-templates")"
  [[ "$result" == "$FIXTURES/theme-templates/style.css" ]]
}

@test "resolve_file: falls back to engine when no blog or theme" {
  mkdir -p "$FIXTURES/engine-templates"
  touch "$FIXTURES/engine-templates/article.html"
  result="$(resolve_file article.html "$FIXTURES/nonexistent" "" "$FIXTURES/engine-templates")"
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

# --- post_snippet ---

@test "post_snippet: uses description when available" {
  result="$(post_snippet description "$FIXTURES/post.md")"
  [[ "$result" == "A test description" ]]
}

@test "post_snippet: falls back to body when no description" {
  result="$(post_snippet description "$FIXTURES/post-no-desc.md")"
  [[ "$result" == *"Content without description"* ]]
}

@test "post_snippet: strips markdown formatting from body" {
  cat > "$FIXTURES/post-bold.md" << 'EOF'
---
title: Bold Post
date: 2026-01-01
---

This has **bold** and *italic* and `code` text.
EOF
  result="$(post_snippet description "$FIXTURES/post-bold.md")"
  [[ "$result" != *"**"* ]]
  [[ "$result" != *'`'* ]]
  [[ "$result" == *"bold"* ]]
}

@test "post_snippet: strips markdown links from body" {
  cat > "$FIXTURES/post-link.md" << 'EOF'
---
title: Link Post
date: 2026-01-01
---

Check [my site](https://example.com) for _more_ info.
EOF
  result="$(post_snippet description "$FIXTURES/post-link.md")"
  [[ "$result" != *"](http"* ]]
  [[ "$result" != *"_more_"* ]]
  [[ "$result" == *"my site"* ]]
  [[ "$result" == *"more"* ]]
}

@test "post_snippet: strips horizontal rules from body" {
  cat > "$FIXTURES/post-hr.md" << 'EOF'
---
title: HR Post
date: 2026-01-01
---

Intro text.

---

More text after rule.
EOF
  result="$(post_snippet description "$FIXTURES/post-hr.md")"
  [[ "$result" != *"---"* ]]
  [[ "$result" == *"Intro text"* ]]
  [[ "$result" == *"More text"* ]]
}

@test "post_snippet: strips strikethrough markers from body" {
  cat > "$FIXTURES/post-strike.md" << 'EOF'
---
title: Strike Post
date: 2026-01-01
---

I created ~~shame~~ courage and decided to write.
EOF
  result="$(post_snippet description "$FIXTURES/post-strike.md")"
  [[ "$result" != *"~~"* ]]
  [[ "$result" == *"shame"* ]]
  [[ "$result" == *"courage"* ]]
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

@test "rss_item: escapes ampersand in title" {
  result="$(rss_item "AI & Ruby" "https://test.com/post.html" "desc" "2026-03-29")"
  [[ "$result" == *"<title>AI &amp; Ruby</title>"* ]]
}

@test "rss_item: wraps description in CDATA" {
  result="$(rss_item "Post" "https://test.com/post.html" "A <b>bold</b> & \"quoted\"" "2026-03-29")"
  [[ "$result" == *'<description><![CDATA[A <b>bold</b> & "quoted"]]></description>'* ]]
}

# --- excerpt ---

@test "post_snippet: custom limit truncates at given length" {
  cat > "$FIXTURES/post-long.md" << 'EOF'
---
title: "Long Post"
---

This is a paragraph that goes on and on and on and on and on and on and on and on forever and ever and keeps going.
EOF
  result="$(post_snippet description "$FIXTURES/post-long.md" 50)"
  [[ ${#result} -le 50 ]]
}

@test "post_snippet: preserves UTF-8 accented chars" {
  cat > "$FIXTURES/post-utf8.md" << 'EOF'
---
title: "Test"
---

Sentado no sofá e assistindo Frozen, então bora lá!
EOF
  result="$(post_snippet description "$FIXTURES/post-utf8.md" 500)"
  [[ "$result" == *"sofá"* ]]
  [[ "$result" == *"então"* ]]
  [[ "$result" == *"lá"* ]]
}

@test "post_snippet: does not break multi-byte chars at boundary" {
  cat > "$FIXTURES/post-boundary.md" << 'EOF'
---
title: "Test"
---

Sentado no sofá e assistindo Frozen, tive a ideia de escrever sobre minha retrospectiva deste ano. Nunca fiz isso antes, então bora lá! E mais texto aqui para passar do limite de caracteres que definimos.
EOF
  result="$(post_snippet description "$FIXTURES/post-boundary.md" 160)"
  echo "$result" | python3 -c "import sys; sys.stdin.buffer.read().decode('utf-8')"
  [[ ${#result} -le 160 ]]
}

@test "post_snippet: xml_escape works with accented snippet" {
  cat > "$FIXTURES/post-special.md" << 'EOF'
---
title: "Test"
---

Café & résumé are "common" words.
EOF
  result="$(post_snippet description "$FIXTURES/post-special.md" 500)"
  escaped="$(xml_escape "$result")"
  [[ "$escaped" == *"&amp;"* ]]
  [[ "$escaped" == *"Café"* ]]
  [[ "$escaped" == *"résumé"* ]]
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

# --- minify_css ---

@test "minify_css: strips comments" {
  echo '/* comment */ body { color: red; }' > "$FIXTURES/test.css"
  result="$(minify_css "$FIXTURES/test.css")"
  [[ "$result" != *"comment"* ]]
  [[ "$result" == *"body{color:red}"* ]]
}

@test "minify_css: collapses whitespace" {
  printf 'body {\n  color:  red;\n  font-size: 16px;\n}\n' > "$FIXTURES/test.css"
  result="$(minify_css "$FIXTURES/test.css")"
  [[ "$result" != *$'\n'* ]]
  [[ "$result" == *"body{color:red;font-size:16px}"* ]]
}

# --- inline_css ---

@test "inline_css: replaces link tag with style" {
  echo '<html><head><link rel="stylesheet" href="style.css"></head><body>hi</body></html>' > "$FIXTURES/page.html"
  echo 'body{color:red}' > "$FIXTURES/min.css"
  inline_css "$FIXTURES/page.html" "$FIXTURES/min.css"
  result="$(cat "$FIXTURES/page.html")"
  [[ "$result" == *"<style>body{color:red}"* ]]
  [[ "$result" == *"</style>"* ]]
  [[ "$result" != *"<link"* ]]
}

# --- minify_html ---

@test "minify_html: strips html comments" {
  echo '<html><!-- comment --><body>hi</body></html>' > "$FIXTURES/page.html"
  minify_html "$FIXTURES/page.html"
  result="$(cat "$FIXTURES/page.html")"
  [[ "$result" != *"comment"* ]]
  [[ "$result" == *"hi"* ]]
}

@test "minify_html: collapses whitespace between tags" {
  printf '<div>  \n  <p>text</p>  \n  </div>' > "$FIXTURES/page.html"
  minify_html "$FIXTURES/page.html"
  result="$(cat "$FIXTURES/page.html")"
  [[ "$result" == *"<div><p>text</p></div>"* ]]
}

@test "minify_html: preserves pre content" {
  echo '<html><pre>  keep   spaces  </pre><p>  collapse  </p></html>' > "$FIXTURES/page.html"
  minify_html "$FIXTURES/page.html"
  result="$(cat "$FIXTURES/page.html")"
  [[ "$result" == *"  keep   spaces  "* ]]
}

# --- render_links ---

@test "render_links: generates nav with links from toml" {
  cat > "$FIXTURES/blog-links.toml" << 'EOF'
title = "Test"
[[links]]
label = "github"
url = "https://github.com/test"
[[links]]
label = "linkedin"
url = "https://linkedin.com/in/test"
EOF
  result="$(render_links "$FIXTURES/blog-links.toml")"
  [[ "$result" == *'<nav class="social-links">'* ]]
  [[ "$result" == *'github'* ]]
  [[ "$result" == *'linkedin'* ]]
  [[ "$result" == *' · '* ]]
}

@test "render_links: returns nothing when no links in toml" {
  cat > "$FIXTURES/blog-nolinks.toml" << 'EOF'
title = "Test"
EOF
  result="$(render_links "$FIXTURES/blog-nolinks.toml")"
  [[ -z "$result" ]]
}

# --- render_guides ---

@test "render_guides: generates badge list from toml" {
  cat > "$FIXTURES/blog-guides.toml" << 'EOF'
title = "Test"
[[guides]]
title = "Web 101"
url = "https://web101.example.com/"
[[guides]]
title = "AWS 101"
url = "https://aws101.example.com/"
EOF
  result="$(render_guides "$FIXTURES/blog-guides.toml")"
  [[ "$result" == *'class="guide-badge"'* ]]
  [[ "$result" == *'Web 101'* ]]
  [[ "$result" == *'AWS 101'* ]]
}

@test "render_guides: returns nothing when no guides in toml" {
  cat > "$FIXTURES/blog-noguides.toml" << 'EOF'
title = "Test"
EOF
  result="$(render_guides "$FIXTURES/blog-noguides.toml")"
  [[ -z "$result" ]]
}
