# Blog engine targets (Rust static site generator)

BLOGS := $(notdir $(wildcard blogs/*))

.PHONY: blog.list blog.build blog.clean blog.test

blog.test: ## Run engine tests
	@cargo test engine::

blog.list: ## List available blogs
	@for b in $(BLOGS); do echo "  $$b"; done

blog.build: $(addprefix blog.build.,$(BLOGS)) ## Build all blogs

blog.build.%: ## Build a specific blog (e.g. make blog.build.my-site)
	@echo "building $*..."
	@cargo run --release --bin devtui-engine -- blogs/$* dist/$*

blog.serve.%: blog.build.% ## Build and serve a blog on localhost:8000
	@echo "  serving $* at http://localhost:8000"
	@cargo run --release --bin devtui-engine -- serve dist/$*

blog.theme.%: ## Set a blog theme (usage: make blog.theme.<name> THEME=terminal)
	@test -n "$(THEME)" || (echo "usage: make blog.theme.$* THEME=paper|newspaper|terminal" && exit 1)
	@test -d src/engine/themes/$(THEME) || (echo "unknown theme: $(THEME)" && exit 1)
	@sed -i.bak -E 's/^theme = ".*"/theme = "$(THEME)"/' blogs/$*/blog.toml && rm blogs/$*/blog.toml.bak
	@echo "  $* -> $(THEME)"

blog.clean: ## Remove all generated files
	@rm -rf dist

blog.clean.%: ## Clean a specific blog
	@rm -rf dist/$*

# ——— Deploy (copy to blog repo) ———

REPO_DIR = $(dir $(realpath blogs/$*/posts))

deploy.cp.%: blog.build.% ## Build and copy to blog repo
	@echo "deploying $* to $(REPO_DIR)..."
	@rsync -a dist/$*/ $(REPO_DIR)
