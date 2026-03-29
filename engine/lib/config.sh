#!/bin/bash
# Config and frontmatter parsing
# Handles blog.toml config and markdown post metadata

# Read a value from a blog.toml file
# Usage: cfg "title" "/path/to/blog.toml"
# Read a value from a blog.toml file
# Returns empty string if key is missing
cfg() {
  local key="$1" config="$2"
  grep "^$key" "$config" 2>/dev/null | sed 's/.*= *"*//;s/"*$//' || true
}

# Extract frontmatter field from a markdown file
# Usage: frontmatter "title" "/path/to/post.md"
# Extract frontmatter field from a markdown file
# Returns empty string if field is missing (grep || true)
frontmatter() {
  local field="$1" file="$2"
  grep "^${field}:" "$file" | sed "s/^${field}: *//;s/^\"//;s/\".*$//" || true
}

# Extract date from frontmatter, stripping time portion
# "2024-07-14 02:37:25Z" → "2024-07-14"
# Usage: frontmatter_date "published_at" "/path/to/post.md"
frontmatter_date() {
  frontmatter "$1" "$2" | cut -d' ' -f1
}

# Extract post body (everything after the closing --- of frontmatter)
# Usage: post_body "/path/to/post.md"
post_body() {
  local file="$1"
  awk 'BEGIN{n=0} /^---$/{n++; if(n==2){found=1; next}} found{print}' "$file"
}

# Extract a text snippet from the post body (first ~160 chars, no markdown)
# Falls back to frontmatter description if available
# Usage: post_snippet "description" "/path/to/post.md"
post_snippet() {
  local desc_field="$1" file="$2"
  local desc
  desc="$(frontmatter "$desc_field" "$file")"
  if [ -n "$desc" ]; then
    echo "$desc"
    return
  fi
  # Convert body to plain text via pandoc, strip artifacts, grab first 160 chars
  post_body "$file" | pandoc --from markdown-yaml_metadata_block-tex_math_dollars-simple_tables-multiline_tables -t plain --wrap=none 2>/dev/null | python3 -c "
import sys, re
s = sys.stdin.read()
s = re.sub(r'[-=]{3,}', '', s)         # horizontal rules
s = re.sub(r'~~', '', s)              # strikethrough markers
s = re.sub(r'^\s*#.*$', '', s, flags=re.M)  # headings
s = ' '.join(s.split())               # collapse whitespace
print(s[:300])
"
}
