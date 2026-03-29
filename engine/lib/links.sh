#!/bin/bash
# Social links and guides parsing
# Format: label|url (one per line, blank lines and # comments ignored)

# Generate HTML for social links (inline, separated by ·)
# Usage: render_links "/path/to/links.txt"
render_links() {
  local file="$1"
  [ -f "$file" ] || return 0
  local first=true
  echo -n '<nav class="social-links">'
  while IFS='|' read -r label url; do
    [ -z "$label" ] && continue
    [[ "$label" == \#* ]] && continue
    $first || echo -n ' · '
    echo -n "<a href=\"$url\" target=\"_blank\" rel=\"noopener\">$label</a>"
    first=false
  done < "$file"
  echo '</nav>'
}

# Generate HTML for guides (badge list)
# Usage: render_guides "/path/to/guides.txt"
render_guides() {
  local file="$1"
  [ -f "$file" ] || return 0
  echo '<div class="guides">'
  while IFS='|' read -r label url; do
    [ -z "$label" ] && continue
    [[ "$label" == \#* ]] && continue
    echo "<a href=\"$url\" target=\"_blank\" rel=\"noopener\" class=\"guide-badge\">$label</a>"
  done < "$file"
  echo '</div>'
}
