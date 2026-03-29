#!/bin/bash
# Engine library functions. Sourced by build.sh and tested by bats.

# Read a value from a blog.toml file
# Usage: cfg "title" "/path/to/blog.toml"
cfg() {
  local key="$1" config="$2"
  grep "^$key" "$config" | sed 's/.*= *"*//;s/"*$//'
}

# Resolve a file: blog override > engine default
# Usage: resolve_file "article.html" "/path/to/blog/templates" "/path/to/engine/templates"
resolve_file() {
  local file="$1" blog_dir="$2" engine_dir="$3"
  if [ -f "$blog_dir/$file" ]; then
    echo "$blog_dir/$file"
  else
    echo "$engine_dir/$file"
  fi
}

# Extract frontmatter field from a markdown file
# Usage: frontmatter "title" "/path/to/post.md"
frontmatter() {
  local field="$1" file="$2"
  grep "^${field}:" "$file" | sed "s/^${field}: *//;s/^\"//;s/\".*$//"
}

# Generate a sitemap entry
# Usage: sitemap_entry "https://example.com/post.html" "2026-03-29"
sitemap_entry() {
  local loc="$1" lastmod="$2"
  echo "<url><loc>$loc</loc><lastmod>$lastmod</lastmod></url>"
}

# Generate robots.txt content
# Usage: robots_txt "https://example.com"
robots_txt() {
  local url="$1"
  printf 'User-agent: *\nAllow: /\nSitemap: %s/sitemap.xml\n' "$url"
}

# Substitute template variables in a file
# Usage: template_sub "file" "key1" "val1" "key2" "val2" ...
template_sub() {
  local file="$1"; shift
  local content
  content="$(cat "$file")"
  while [ $# -ge 2 ]; do
    local key="$1" val="$2"; shift 2
    content="${content//\$${key}\$/$val}"
  done
  echo "$content"
}
