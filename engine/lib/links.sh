#!/bin/bash
# Social links and guides rendering from blog.toml via dasel + jq

# Generate HTML for social links (inline, separated by ·)
# Usage: render_links "/path/to/blog.toml"
render_links() {
  local config="$1"
  local json
  json="$(dasel -i toml -o json 'links' < "$config" 2>/dev/null)" || return 0
  [ "$json" = "null" ] || [ "$json" = "[]" ] && return 0

  local html
  html="$(echo "$json" | jq -r '[.[] | "<a href=\"\(.url)\" target=\"_blank\" rel=\"noopener\">\(.label)</a>"] | join(" · ")')"
  echo "<nav class=\"social-links\">$html</nav>"
}

# Generate HTML for tag filter buttons
# Usage: render_tags "/path/to/blog.toml"
render_tags() {
  local config="$1"
  local json
  json="$(dasel -i toml -o json 'tags' < "$config" 2>/dev/null)" || return 0
  [ "$json" = "null" ] || [ "$json" = "[]" ] && return 0

  local buttons
  local buttons
  buttons="$(echo "$json" | jq -r '[.[] | "<button class=\"tag-btn\" data-tag=\"\(.)\">\(.)</button>"] | join("<span class=\"sep\"> · </span>")')"
  echo "<button class=\"tag-btn active\" data-tag=\"all\">all</button><span class=\"sep\"> · </span>$buttons"
}

# Generate HTML for guides (badge list)
# Usage: render_guides "/path/to/blog.toml"
render_guides() {
  local config="$1"
  local json
  json="$(dasel -i toml -o json 'guides' < "$config" 2>/dev/null)" || return 0
  [ "$json" = "null" ] || [ "$json" = "[]" ] && return 0

  local html
  html="$(echo "$json" | jq -r '[.[] | "<a href=\"\(.url)\" target=\"_blank\" rel=\"noopener\" class=\"guide-badge\">\(.title)</a>"] | join("<span class=\"sep\"> · </span>")')"
  echo "<span class=\"guides\">$html</span>"
}
