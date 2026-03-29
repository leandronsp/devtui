# Blog engine targets (shell-based static site generator)

BLOGS := $(notdir $(wildcard blogs/*))

.PHONY: blog.list blog.build blog.clean blog.test

blog.test: ## Run engine tests (bats)
	@bats engine/tests/

blog.list: ## List available blogs
	@for b in $(BLOGS); do echo "  $$b"; done

blog.build: $(addprefix blog.build.,$(BLOGS)) ## Build all blogs

blog.build.%: ## Build a specific blog (e.g. make blog.build.my-site)
	@echo "building $*..."
	@engine/build.sh blogs/$* dist/$*

blog.serve.%: blog.build.% ## Build and serve a blog on localhost:8000
	@echo "  serving $* at http://localhost:8000"
	@cd dist/$* && python3 -m http.server 8000

blog.clean: ## Remove all generated files
	@rm -rf dist

blog.clean.%: ## Clean a specific blog
	@rm -rf dist/$*

# ——— Deploy (git auto-deploy) ———

REPO_DIR = $(dir $(realpath blogs/$*/posts))

deploy.git.%: blog.build.% ## Build and copy to blog repo
	@echo "deploying $* to $(REPO_DIR)..."
	@rsync -a dist/$*/ $(REPO_DIR)
