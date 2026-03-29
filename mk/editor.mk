# Editor targets (Rust TUI)

.PHONY: editor.build editor.run editor.test editor.lint

editor.build: ## Build editor in release mode
	@cargo build --release

editor.run: ## Run editor (usage: make editor.run FILE=mypost.md)
	@cargo run -- $(FILE)

editor.test: ## Run editor tests
	@cargo test

editor.lint: ## Run clippy linter
	@cargo clippy -- -D warnings
