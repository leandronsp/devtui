# Editor targets (Rust TUI)

.PHONY: editor.build editor.run editor.test

editor.build: ## Build editor in release mode
	@cargo build --release --bin devtui

editor.run: ## Run editor (usage: make editor.run FILE=mypost.md)
	@cargo run --bin devtui -- $(FILE)

editor.test: ## Run editor tests
	@cargo test editor::
