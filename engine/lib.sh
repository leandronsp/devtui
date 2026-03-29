#!/bin/bash
# Engine library loader. Sources all modules.

LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/lib" && pwd)"

source "$LIB_DIR/config.sh"
source "$LIB_DIR/template.sh"
source "$LIB_DIR/seo.sh"
source "$LIB_DIR/minify.sh"
