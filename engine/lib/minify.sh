#!/bin/bash
# Minification functions for CSS and HTML

# Minify CSS: strip comments, collapse whitespace
# Usage: minify_css "file.css" (outputs to stdout)
minify_css() {
  python3 -c "
import sys, re
css = open(sys.argv[1], encoding='utf-8', errors='replace').read()
css = re.sub(r'/\*.*?\*/', '', css, flags=re.S)
css = re.sub(r'\s+', ' ', css)
css = re.sub(r'\s*([{}:;,>~+])\s*', r'\1', css)
css = re.sub(r';\s*}', '}', css)
print(css.strip())
" "$1"
}

# Inline CSS into HTML: replaces <link rel=stylesheet> with <style>content</style>
# Usage: inline_css "page.html" "minified.css"
inline_css() {
  python3 -c "
import re, sys
html = open(sys.argv[1], encoding='utf-8', errors='replace').read()
css = open(sys.argv[2], encoding='utf-8', errors='replace').read()
html = re.sub(r'<link[^>]*stylesheet[^>]*/?\s*>', '<style>' + css + '</style>', html)
with open(sys.argv[1], 'w', encoding='utf-8') as f:
    f.write(html)
" "$1" "$2"
}

# Minify HTML: strip comments, collapse whitespace (preserves pre/code/script)
# Usage: minify_html "page.html"
minify_html() {
  python3 -c "
import re, sys

with open(sys.argv[1], encoding='utf-8', errors='replace') as f:
    html = f.read()

preserved = []
def save(m):
    preserved.append(m.group(0))
    return f'__PRESERVE_{len(preserved)-1}__'

html = re.sub(r'<pre[^>]*>.*?</pre>', save, html, flags=re.S)
html = re.sub(r'<script[^>]*>.*?</script>', save, html, flags=re.S)
html = re.sub(r'<style[^>]*>.*?</style>', save, html, flags=re.S)

html = re.sub(r'<!--(?!\[).*?-->', '', html, flags=re.S)
html = re.sub(r'>\s+<', '><', html)
html = re.sub(r'\s+', ' ', html)

for i, block in enumerate(preserved):
    html = html.replace(f'__PRESERVE_{i}__', block)

with open(sys.argv[1], 'w', encoding='utf-8') as f:
    f.write(html)
" "$1"
}
