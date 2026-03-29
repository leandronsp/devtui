#!/bin/bash
# Template resolution and variable substitution

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
