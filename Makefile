.DEFAULT_GOAL := help
.PHONY: help test

include mk/editor.mk
include mk/blog.mk

help: ## Show available targets
	@grep -hE '^[a-zA-Z._%-]+:.*##' Makefile mk/*.mk | awk -F ':.*## ' '{printf "  \033[36m%-28s\033[0m %s\n", $$1, $$2}'

test: editor.test blog.test editor.lint ## Run all tests + lint
