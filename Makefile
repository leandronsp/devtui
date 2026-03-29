BLOGS := $(notdir $(wildcard blogs/*))

.PHONY: help blog.list blog.build blog.clean

help: ## Show available targets
	@grep -E '^[a-zA-Z._%-]+:.*##' Makefile | awk -F ':.*## ' '{printf "  \033[36m%-28s\033[0m %s\n", $$1, $$2}'

blog.list: ## List available blogs
	@for b in $(BLOGS); do echo "  $$b"; done

blog.build: $(addprefix blog.build.,$(BLOGS)) ## Build all blogs

blog.build.%: ## Build a specific blog (e.g. make blog.build.acme-alchemist)
	@echo "building $*..."
	@engine/build.sh blogs/$* dist/$*

blog.serve.%: blog.build.% ## Build and serve a blog on localhost:8000
	@echo "  serving $* at http://localhost:8000"
	@cd dist/$* && python3 -m http.server 8000

blog.clean: ## Remove all generated files
	@rm -rf dist

blog.clean.%: ## Clean a specific blog
	@rm -rf dist/$*
