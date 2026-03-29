#!/bin/bash
# Config and frontmatter parsing
# Uses dasel for TOML, grep/sed for markdown frontmatter

# Read a value from a blog.toml file via dasel
# Returns empty string if key is missing
# Usage: cfg "title" "/path/to/blog.toml"
cfg() {
  local key="$1" config="$2"
  local val
  val="$(dasel -i toml "$key" < "$config" 2>/dev/null)" || true
  echo "$val" | sed "s/^'//;s/'$//"
}

# Extract frontmatter field from a markdown file
# Returns empty string if field is missing
# Usage: frontmatter "title" "/path/to/post.md"
frontmatter() {
  local field="$1" file="$2"
  grep "^${field}:" "$file" | sed "s/^${field}: *//;s/^\"//;s/\".*$//" || true
}

# Extract date from frontmatter, stripping time portion
# "2024-07-14 02:37:25Z" -> "2024-07-14"
frontmatter_date() {
  frontmatter "$1" "$2" | cut -d' ' -f1
}

# Extract post body (everything after the closing --- of frontmatter)
post_body() {
  local file="$1"
  awk 'BEGIN{n=0} /^---$/{n++; if(n==2){found=1; next}} found{print}' "$file"
}

# Extract a text snippet from the post body (first ~300 chars, no markdown)
# Falls back to frontmatter description if available
post_snippet() {
  local desc_field="$1" file="$2"
  local desc
  desc="$(frontmatter "$desc_field" "$file")"
  if [ -n "$desc" ]; then
    echo "$desc"
    return
  fi
  # Convert body to plain text via pandoc, strip artifacts, truncate
  post_body "$file" \
    | pandoc --from markdown-yaml_metadata_block-tex_math_dollars-simple_tables-multiline_tables -t plain --wrap=none 2>/dev/null \
    | sed 's/---*//g;s/~~//g;s/^#.*//g' \
    | tr '\n' ' ' \
    | sed 's/  */ /g;s/^ //' \
    | LC_ALL=en_US.UTF-8 awk '{print substr($0, 1, 300)}'
}
