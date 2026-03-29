#!/bin/bash
# SEO artifact generation: sitemap, robots, RSS

xml_escape() {
  echo "$1" | sed 's/&/\&amp;/g; s/</\&lt;/g; s/>/\&gt;/g; s/"/\&quot;/g'
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

# Generate RSS channel header
# Usage: rss_header "Site Title" "https://example.com" "Site description"
rss_header() {
  local title="$1" url="$2" desc="$3"
  cat << RSS
<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
<channel>
<title>$title</title>
<link>$url</link>
<description>$desc</description>
<atom:link href="$url/feed.xml" rel="self" type="application/rss+xml"/>
RSS
}

# Generate RSS item entry
# Usage: rss_item "Post Title" "https://example.com/post.html" "Description" "2026-03-29"
rss_item() {
  local title link date
  title="$(xml_escape "$1")"
  link="$2"
  date="$4"
  cat << RSS
<item>
<title>$title</title>
<link>$link</link>
<guid>$link</guid>
<description><![CDATA[$3]]></description>
<pubDate>$date</pubDate>
</item>
RSS
}
