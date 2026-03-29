#!/bin/bash
# Config and frontmatter parsing
# Handles blog.toml config and markdown post metadata

# Read a value from a blog.toml file
# Usage: cfg "title" "/path/to/blog.toml"
cfg() {
  local key="$1" config="$2"
  grep "^$key" "$config" | sed 's/.*= *"*//;s/"*$//'
}

# Extract frontmatter field from a markdown file
# Usage: frontmatter "title" "/path/to/post.md"
# Extract frontmatter field from a markdown file
# Returns empty string if field is missing (grep || true)
frontmatter() {
  local field="$1" file="$2"
  grep "^${field}:" "$file" | sed "s/^${field}: *//;s/^\"//;s/\".*$//" || true
}

# Extract post body (everything after the closing --- of frontmatter)
# Usage: post_body "/path/to/post.md"
post_body() {
  local file="$1"
  awk 'BEGIN{n=0} /^---$/{n++; if(n==2){found=1; next}} found{print}' "$file"
}
