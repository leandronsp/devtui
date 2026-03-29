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
THEME="$(cfg theme "$CONFIG")"
ANALYTICS_ID="$(cfg analytics_id "$CONFIG")"
LICENSE="$(cfg license "$CONFIG")"
LICENSE_URL="$(cfg license_url "$CONFIG")"

# Theme directory (defaults to paper)
THEME="${THEME:-paper}"
THEME_DIR="$ENGINE_DIR/themes/$THEME"

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
ARTICLE_TPL="$(resolve_file article.html "$BLOG_DIR/templates" "$THEME_DIR/templates" "$ENGINE_DIR/templates")"
SITEMAP_ENTRIES=""

REBUILT_ARTICLES=""
SKIPPED=0

for md in "$POSTS_DIR"/*.md; do
  [ -f "$md" ] || continue
  slug="$(basename "$md" .md)"
  post_title="$(frontmatter title "$md")"
  post_date="$(frontmatter_date "$DATE_FIELD" "$md")"
  post_desc="$(post_snippet description "$md" 160)"

  # Incremental: skip if html exists and is newer than both md and template
  html_out="$ARTICLES_DIR/$slug.html"
  if [ -f "$html_out" ] && [ "$html_out" -nt "$md" ] && [ "$html_out" -nt "$ARTICLE_TPL" ]; then
    SKIPPED=$((SKIPPED + 1))
  else
    # Pipe body only (skip frontmatter) to avoid pandoc parsing --- as YAML
    post_body "$md" | pandoc --from markdown-yaml_metadata_block-tex_math_dollars-simple_tables-multiline_tables+autolink_bare_uris -o "$html_out" \
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
    REBUILT_ARTICLES="$REBUILT_ARTICLES $html_out"
    echo "  built $slug.html"
  fi

  SITEMAP_ENTRIES="$SITEMAP_ENTRIES$(sitemap_entry "$SITE_URL/${ARTICLES_PREFIX}$slug.html" "$post_date")
"
done
[ "$SKIPPED" -gt 0 ] && echo "  skipped $SKIPPED unchanged articles"

# Build style: concatenate modular CSS from theme (blog override > theme)
STYLE_DIR=""
for dir in "$BLOG_DIR" "$THEME_DIR"; do
  [ -f "$dir/base.css" ] && STYLE_DIR="$dir" && break
done

> "$DIST_DIR/style.css"
for part in base index article syntax responsive; do
  [ -f "$STYLE_DIR/$part.css" ] && cat "$STYLE_DIR/$part.css" >> "$DIST_DIR/style.css"
done

# Copy static assets (uploads, images, etc.)
for dir in uploads images assets; do
  if [ -d "$BLOG_DIR/$dir" ]; then
    rsync -a "$BLOG_DIR/$dir/" "$DIST_DIR/$dir/"
    echo "  copied $dir/"
  fi
done

# Build index
# Build index from template with variable substitution
INDEX_TPL="$(resolve_file index_header.html "$BLOG_DIR/templates" "$THEME_DIR/templates" "$ENGINE_DIR/templates")"
template_sub "$INDEX_TPL" \
  title "$TITLE" \
  subtitle "$SUBTITLE" \
  url "$SITE_URL" \
  author "$AUTHOR" \
  lang "$LANG" \
  > "$DIST_DIR/index.html"

# Inject social links and guides into header nav
LINKS_HTML="$(render_links "$CONFIG")"
GUIDES_HTML="$(render_guides "$CONFIG")"
NAV_HTML="$LINKS_HTML"
if [ -n "$NAV_HTML" ] && [ -n "$GUIDES_HTML" ]; then
  NAV_HTML="$NAV_HTML $GUIDES_HTML"
elif [ -n "$GUIDES_HTML" ]; then
  NAV_HTML="$GUIDES_HTML"
fi
if [ -n "$NAV_HTML" ]; then
  python3 -c "
import sys
html = open(sys.argv[1]).read()
html = html.replace('<div class=\"guides-slot\"></div>', sys.argv[2], 1)
with open(sys.argv[1], 'w') as f: f.write(html)
" "$DIST_DIR/index.html" "$NAV_HTML"
fi

TAGS_HTML="$(render_tags "$CONFIG")"
if [ -n "$TAGS_HTML" ]; then
  python3 -c "
import sys
html = open(sys.argv[1]).read()
html = html.replace('<div class=\"tag-filter\"></div>', '<div class=\"tag-filter\">' + sys.argv[2] + '</div>', 1)
with open(sys.argv[1], 'w') as f: f.write(html)
" "$DIST_DIR/index.html" "$TAGS_HTML"
fi

# Inject guides and tags into mobile menu
if [ -n "$GUIDES_HTML" ]; then
  python3 -c "
import sys
html = open(sys.argv[1]).read()
html = html.replace('<div class=\"mobile-pills mobile-guides\"></div>', '<div class=\"mobile-pills mobile-guides\">' + sys.argv[2] + '</div>', 1)
with open(sys.argv[1], 'w') as f: f.write(html)
" "$DIST_DIR/index.html" "$GUIDES_HTML"
fi
if [ -n "$TAGS_HTML" ]; then
  python3 -c "
import sys
html = open(sys.argv[1]).read()
html = html.replace('<div class=\"mobile-pills mobile-tags\"></div>', '<div class=\"mobile-pills mobile-tags\">' + sys.argv[2] + '</div>', 1)
with open(sys.argv[1], 'w') as f: f.write(html)
" "$DIST_DIR/index.html" "$TAGS_HTML"
fi

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
  post_lang="$(frontmatter language "$md")"
  # Normalize pt-BR to pt
  case "$post_lang" in pt-BR|pt-br|pt) post_lang="pt" ;; esac
  post_tags="$(grep '^tags:' "$md" | sed 's/^tags: *//;s/\[//;s/\]//;s/"//g;s/, */ /g' || true)"
  echo "<li data-lang=\"$post_lang\" data-tags=\"$post_tags\"><time datetime=\"$post_date\">$post_date</time><a href=\"${ARTICLES_PREFIX}$slug.html\">$post_title</a><p class=\"post-desc\">$snippet</p></li>"
done <<< "$(echo "$SORTED_POSTS" | sort -r)" >> "$DIST_DIR/index.html"

FOOTER_HTML=""
if [ -n "$LICENSE" ] && [ -n "$LICENSE_URL" ];then
  FOOTER_HTML="<a href=\"$LICENSE_URL\" target=\"_blank\" rel=\"noopener\">$LICENSE</a>"
elif [ -n "$LICENSE" ]; then
  FOOTER_HTML="$LICENSE"
fi

GA_SCRIPT=""
if [ -n "$ANALYTICS_ID" ]; then
  GA_SCRIPT="<script>window.addEventListener('load',function(){setTimeout(function(){var s=document.createElement('script');s.src='https://www.googletagmanager.com/gtag/js?id=$ANALYTICS_ID';s.async=true;document.head.appendChild(s);s.onload=function(){window.dataLayer=window.dataLayer||[];function g(){dataLayer.push(arguments)}g('js',new Date());g('config','$ANALYTICS_ID')};},2000)})</script>"
fi

cat >> "$DIST_DIR/index.html" << INDEXFOOT
</ul></main>
<script>
var activeLang='all',activeTag='all';
function bindFilter(sel,cls,cb){
  document.querySelectorAll(sel).forEach(function(el){
    el.onclick=function(e){
      if(!e.target.classList.contains(cls))return;
      el.querySelector('.'+cls+'.active').classList.remove('active');
      e.target.classList.add('active');
      cb(e.target.dataset);
      filterPosts();
    };
  });
}
bindFilter('.lang-filter,.mobile-menu .mobile-pills','lang-btn',function(d){activeLang=d.lang});
bindFilter('.tag-filter,.mobile-tags','tag-btn',function(d){activeTag=d.tag});
function filterPosts(){
  var q=document.querySelector('.search-input').value.toLowerCase();
  document.querySelectorAll('.post-list li').forEach(function(li){
    var matchLang=activeLang==='all'||li.dataset.lang===activeLang;
    var matchTag=activeTag==='all'||(' '+li.dataset.tags+' ').indexOf(' '+activeTag+' ')!==-1;
    var matchSearch=!q||li.textContent.toLowerCase().indexOf(q)!==-1;
    li.style.display=matchLang&&matchTag&&matchSearch?'':'none';
  });
}
</script>
$GA_SCRIPT
<footer>$FOOTER_HTML</footer>
</body></html>
INDEXFOOT
echo "  built index.html"

# Generate sitemap.xml
cat > "$DIST_DIR/sitemap.xml" << SITEMAP
<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
<url><loc>$SITE_URL/</loc></url>
$SITEMAP_ENTRIES</urlset>
SITEMAP
echo "  built sitemap.xml"

# Generate feed.xml (RSS), sorted by date descending
rss_header "$TITLE" "$SITE_URL" "$SUBTITLE" > "$DIST_DIR/feed.xml"
RSS_ITEMS=""
for md in "$POSTS_DIR"/*.md; do
  [ -f "$md" ] || continue
  post_date="$(frontmatter_date "$DATE_FIELD" "$md")"
  RSS_ITEMS="$RSS_ITEMS$post_date	$md
"
done
echo "$RSS_ITEMS" | sort -rn | head -10 | while IFS='	' read -r post_date md; do
  [ -z "$md" ] && continue
  post_title="$(frontmatter title "$md")"
  post_desc="$(post_body "$md" | pandoc --from markdown-yaml_metadata_block-tex_math_dollars-simple_tables-multiline_tables+autolink_bare_uris --highlight-style=breezedark 2>/dev/null)"
  slug="$(basename "$md" .md)"
  rss_item "$post_title" "$SITE_URL/${ARTICLES_PREFIX}$slug.html" "$post_desc" "$post_date"
done >> "$DIST_DIR/feed.xml"
echo '</channel></rss>' >> "$DIST_DIR/feed.xml"
xmllint --noout "$DIST_DIR/feed.xml" 2>/dev/null || { echo "  ERROR: feed.xml is not valid XML"; exit 1; }
echo "  built feed.xml"

# Generate robots.txt
robots_txt "$SITE_URL" > "$DIST_DIR/robots.txt"
echo "  built robots.txt"

# Collect files that need post-processing (GA + minification)
PROCESS_FILES="$DIST_DIR/index.html"
for html in $REBUILT_ARTICLES; do
  PROCESS_FILES="$PROCESS_FILES $html"
done

# Inject Google Analytics into rebuilt article pages
if [ -n "$ANALYTICS_ID" ]; then
  for html in $REBUILT_ARTICLES; do
    python3 -c "
import sys
html = open(sys.argv[1]).read()
html = html.replace('</body>', sys.argv[2] + '</body>', 1)
with open(sys.argv[1], 'w') as f: f.write(html)
" "$html" "$GA_SCRIPT"
  done
fi

# Minify: inline CSS + compress HTML (only rebuilt files)
MINIFIED_CSS="$DIST_DIR/.min.css"
minify_css "$DIST_DIR/style.css" > "$MINIFIED_CSS"

MINIFIED=0
for html in $PROCESS_FILES; do
  [ -f "$html" ] || continue
  inline_css "$html" "$MINIFIED_CSS"
  minify_html "$html"
  MINIFIED=$((MINIFIED + 1))
done

rm -f "$MINIFIED_CSS" "$DIST_DIR/style.css"
echo "  minified $MINIFIED files"
