#!/bin/bash
set -euo pipefail

BLOG_DIR="$1"
DIST_DIR="$2"
ENGINE_DIR="$(dirname "$0")"

BLOG_NAME="$(basename "$BLOG_DIR")"
CONFIG="$BLOG_DIR/blog.toml"
POSTS_DIR="$BLOG_DIR/posts"

# Read config (simple toml parser for key = "value" or key = value)
cfg() { grep "^$1" "$CONFIG" | sed 's/.*= *"*//;s/"*$//' ; }

TITLE="$(cfg title)"
SUBTITLE="$(cfg subtitle)"
DATE_FIELD="$(cfg date_field)"

# Resolve templates: blog override > engine default
template() {
  if [ -f "$BLOG_DIR/templates/$1" ]; then
    echo "$BLOG_DIR/templates/$1"
  else
    echo "$ENGINE_DIR/templates/$1"
  fi
}

# Resolve style: blog override > engine default
style_src() {
  if [ -f "$BLOG_DIR/style.css" ]; then
    echo "$BLOG_DIR/style.css"
  else
    echo "$ENGINE_DIR/style.css"
  fi
}

mkdir -p "$DIST_DIR"

# Build articles
ARTICLE_TPL="$(template article.html)"
for md in "$POSTS_DIR"/*.md; do
  [ -f "$md" ] || continue
  slug="$(basename "$md" .md)"
  pandoc "$md" -o "$DIST_DIR/$slug.html" \
    --template="$ARTICLE_TPL" \
    --highlight-style=breezedark \
    --variable "site-title=$TITLE"
  echo "  built $slug.html"
done

# Copy style
cp "$(style_src)" "$DIST_DIR/style.css"

# Build index
INDEX_HEADER="$(template index_header.html)"

# Generate index_header with site-specific title/subtitle
sed -e "s/\\\$title\\\$/$TITLE/g" -e "s/\\\$subtitle\\\$/$SUBTITLE/g" "$INDEX_HEADER" > "$DIST_DIR/index.html"

# Append posts (reverse chronological by filename)
for md in $(ls -r "$POSTS_DIR"/*.md 2>/dev/null); do
  title="$(grep "^title:" "$md" | sed 's/^title: *//;s/^"//;s/"$//')"
  date="$(grep "^${DATE_FIELD}:" "$md" | sed "s/^${DATE_FIELD}: *//;s/^\"//;s/\".*$//")"
  desc="$(grep "^description:" "$md" | sed 's/^description: *//;s/^"//;s/"$//')"
  slug="$(basename "$md" .md)"
  echo "<li><time>$date</time><a href=\"$slug.html\">$title</a><p class=\"post-desc\">$desc</p></li>" >> "$DIST_DIR/index.html"
done

echo '</ul></main></body></html>' >> "$DIST_DIR/index.html"
echo "  built index.html"
