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

mkdir -p "$DIST_DIR"

# Build articles
ARTICLE_TPL="$(resolve_file article.html "$BLOG_DIR/templates" "$ENGINE_DIR/templates")"
SITEMAP_ENTRIES=""

for md in "$POSTS_DIR"/*.md; do
  [ -f "$md" ] || continue
  slug="$(basename "$md" .md)"
  post_date="$(frontmatter "$DATE_FIELD" "$md")"

  pandoc "$md" -o "$DIST_DIR/$slug.html" \
    --template="$ARTICLE_TPL" \
    --highlight-style=breezedark \
    --variable "site-title=$TITLE" \
    --variable "site-author=$AUTHOR" \
    --variable "site-url=$SITE_URL" \
    --variable "slug=$slug" \
    --variable "lang=$LANG"

  SITEMAP_ENTRIES="$SITEMAP_ENTRIES$(sitemap_entry "$SITE_URL/$slug.html" "$post_date")
"
  echo "  built $slug.html"
done

# Copy style
cp "$(resolve_file style.css "$BLOG_DIR" "$ENGINE_DIR")" "$DIST_DIR/style.css"

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

for md in $(ls -r "$POSTS_DIR"/*.md 2>/dev/null); do
  post_title="$(frontmatter title "$md")"
  post_date="$(frontmatter "$DATE_FIELD" "$md")"
  post_desc="$(frontmatter description "$md")"
  slug="$(basename "$md" .md)"
  echo "<li><time datetime=\"$post_date\">$post_date</time><a href=\"$slug.html\">$post_title</a><p class=\"post-desc\">$post_desc</p></li>" >> "$DIST_DIR/index.html"
done

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

# Generate robots.txt
robots_txt "$SITE_URL" > "$DIST_DIR/robots.txt"
echo "  built robots.txt"
