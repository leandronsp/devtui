#!/bin/bash
# Config and frontmatter parsing

# Read a value from a blog.toml file
# Usage: cfg "title" "/path/to/blog.toml"
cfg() {
  local key="$1" config="$2"
  grep "^$key" "$config" | sed 's/.*= *"*//;s/"*$//'
}

# Extract frontmatter field from a markdown file
# Usage: frontmatter "title" "/path/to/post.md"
frontmatter() {
  local field="$1" file="$2"
  grep "^${field}:" "$file" | sed "s/^${field}: *//;s/^\"//;s/\".*$//"
}
