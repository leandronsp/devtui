# Editor targets (Rust TUI)

.PHONY: editor.build editor.run editor.test

editor.build: ## Build editor in release mode
	@cargo build --release --bin devtui

editor.run: ## Run editor (usage: make editor.run FILE=mypost.md)
	@cargo run --bin devtui -- $(FILE)

editor.cms.%: ## Run CMS list view for a blog (usage: make editor.cms.<name>)
	@cargo run --bin devtui -- --cms blogs/$*

editor.test: ## Run editor tests
	@cargo test editor::
