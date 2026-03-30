.DEFAULT_GOAL := help
.PHONY: help test lint

include mk/editor.mk
include mk/blog.mk

help: ## Show available targets
	@grep -hE '^[a-zA-Z._%-]+:.*##' Makefile mk/*.mk | awk -F ':.*## ' '{printf "  \033[36m%-28s\033[0m %s\n", $$1, $$2}'

lint: ## Run clippy linter
	@cargo clippy -- -D warnings

test: editor.test blog.test lint ## Run all tests + lint
