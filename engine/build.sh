#!/bin/bash
set -euo pipefail

BLOG_DIR="$1"
DIST_DIR="$2"
ENGINE_DIR="$(dirname "$0")"

source "$ENGINE_DIR/lib.sh"

CONFIG="$BLOG_DIR/blog.toml"
POSTS_DIR="$BLOG_DIR/posts"

TITLE="$(cfg title "$CONFIG")"
SUBTITLE="$(cfg subtitle "$CONFIG")"
SITE_URL="$(cfg url "$CONFIG")"
AUTHOR="$(cfg author "$CONFIG")"
DATE_FIELD="$(cfg date_field "$CONFIG")"
LANG="$(cfg lang "$CONFIG")"
ARTICLES_PATH="$(cfg articles_path "$CONFIG")"

mkdir -p "$DIST_DIR"

# Articles output dir (root or subdirectory like "articles")
if [ -n "$ARTICLES_PATH" ]; then
  ARTICLES_DIR="$DIST_DIR/$ARTICLES_PATH"
  ARTICLES_PREFIX="$ARTICLES_PATH/"
else
  ARTICLES_DIR="$DIST_DIR"
  ARTICLES_PREFIX=""
fi
mkdir -p "$ARTICLES_DIR"

# Build articles
ARTICLE_TPL="$(resolve_file article.html "$BLOG_DIR/templates" "$ENGINE_DIR/templates")"
SITEMAP_ENTRIES=""

for md in "$POSTS_DIR"/*.md; do
  [ -f "$md" ] || continue
  slug="$(basename "$md" .md)"
  post_title="$(frontmatter title "$md")"
  post_date="$(frontmatter_date "$DATE_FIELD" "$md")"
  post_desc="$(frontmatter description "$md")"

  # Pipe body only (skip frontmatter) to avoid pandoc parsing --- as YAML
  post_body "$md" | pandoc --from markdown-yaml_metadata_block-tex_math_dollars-simple_tables-multiline_tables+autolink_bare_uris -o "$ARTICLES_DIR/$slug.html" \
    --template="$ARTICLE_TPL" \
    --highlight-style=breezedark \
    --metadata "title=$post_title" \
    --metadata "date=$post_date" \
    --metadata "description=$post_desc" \
    --variable "site-title=$TITLE" \
    --variable "site-author=$AUTHOR" \
    --variable "site-url=$SITE_URL" \
    --variable "slug=${ARTICLES_PREFIX}$slug" \
    --variable "lang=$LANG" \
    --variable "base-path=$([ -n "$ARTICLES_PATH" ] && echo "../" || echo "")"

  SITEMAP_ENTRIES="$SITEMAP_ENTRIES$(sitemap_entry "$SITE_URL/${ARTICLES_PREFIX}$slug.html" "$post_date")
"
  echo "  built $slug.html"
done

# Copy style
cp "$(resolve_file style.css "$BLOG_DIR" "$ENGINE_DIR")" "$DIST_DIR/style.css"

# Copy static assets (uploads, images, etc.)
for dir in uploads images assets; do
  if [ -d "$BLOG_DIR/$dir" ]; then
    cp -r "$BLOG_DIR/$dir" "$DIST_DIR/$dir"
    echo "  copied $dir/"
  fi
done

# Build index
# Build index from template with variable substitution
INDEX_TPL="$(resolve_file index_header.html "$BLOG_DIR/templates" "$ENGINE_DIR/templates")"
template_sub "$INDEX_TPL" \
  title "$TITLE" \
  subtitle "$SUBTITLE" \
  url "$SITE_URL" \
  author "$AUTHOR" \
  lang "$LANG" \
  > "$DIST_DIR/index.html"

# Build sorted post list (newest first by date field)
SORTED_POSTS=""
for md in "$POSTS_DIR"/*.md; do
  [ -f "$md" ] || continue
  post_date="$(frontmatter_date "$DATE_FIELD" "$md")"
  SORTED_POSTS="$SORTED_POSTS$post_date	$md
"
done

# Deduplicate by title (some posts exist with and without dev.to suffix)
SEEN_TITLES=""
while IFS=$'\t' read -r post_date md; do
  [ -z "$md" ] && continue
  post_title="$(frontmatter title "$md")"
  # Skip if we've already listed this title
  case "$SEEN_TITLES" in *"|$post_title|"*) continue ;; esac
  SEEN_TITLES="$SEEN_TITLES|$post_title|"
  snippet="$(post_snippet description "$md")"
  slug="$(basename "$md" .md)"
  echo "<li><time datetime=\"$post_date\">$post_date</time><a href=\"${ARTICLES_PREFIX}$slug.html\">$post_title</a><p class=\"post-desc\">$snippet</p></li>"
done <<< "$(echo "$SORTED_POSTS" | sort -r)" >> "$DIST_DIR/index.html"

echo '</ul></main></body></html>' >> "$DIST_DIR/index.html"
echo "  built index.html"

# Generate sitemap.xml
cat > "$DIST_DIR/sitemap.xml" << SITEMAP
<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
<url><loc>$SITE_URL/</loc></url>
$SITEMAP_ENTRIES</urlset>
SITEMAP
echo "  built sitemap.xml"

# Generate feed.xml (RSS)
rss_header "$TITLE" "$SITE_URL" "$SUBTITLE" > "$DIST_DIR/feed.xml"
for md in $(ls -r "$POSTS_DIR"/*.md 2>/dev/null); do
  post_title="$(frontmatter title "$md")"
  post_date="$(frontmatter_date "$DATE_FIELD" "$md")"
  post_desc="$(frontmatter description "$md")"
  slug="$(basename "$md" .md)"
  rss_item "$post_title" "$SITE_URL/${ARTICLES_PREFIX}$slug.html" "$post_desc" "$post_date" >> "$DIST_DIR/feed.xml"
done
echo '</channel></rss>' >> "$DIST_DIR/feed.xml"
echo "  built feed.xml"

# Generate robots.txt
robots_txt "$SITE_URL" > "$DIST_DIR/robots.txt"
echo "  built robots.txt"
