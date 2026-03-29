#!/bin/bash
# Template resolution and variable substitution

# Resolve a file with fallback chain:
#   blog override > theme > engine default
# Usage: resolve_file "style.css" "/blog/dir" "/engine/themes/newspaper" "/engine"
resolve_file() {
  local file="$1" blog_dir="$2" theme_dir="$3" engine_dir="$4"
  if [ -f "$blog_dir/$file" ]; then
    echo "$blog_dir/$file"
  elif [ -n "$theme_dir" ] && [ -f "$theme_dir/$file" ]; then
    echo "$theme_dir/$file"
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
