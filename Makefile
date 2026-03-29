POSTS    := $(sort $(wildcard posts/*.md))
BUILD    := blog
TEMPLATE := templates/article.html
STYLE    := style.css

ARTICLES := $(patsubst posts/%.md,$(BUILD)/%.html,$(POSTS))

.PHONY: help blog.build blog.serve blog.clean

help: ## Show available targets
	@grep -E '^[a-zA-Z._-]+:.*##' Makefile | awk -F ':.*## ' '{printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

blog.build: $(ARTICLES) blog.index $(BUILD)/style.css ## Build the blog

$(BUILD)/%.html: posts/%.md $(TEMPLATE) $(STYLE)
	@mkdir -p $(BUILD)
	@pandoc $< -o $@ --template=$(TEMPLATE) --highlight-style=breezedark
	@echo "  built $@"

$(BUILD)/style.css: $(STYLE)
	@mkdir -p $(BUILD)
	@cp $< $@

blog.index: $(ARTICLES)
	@mkdir -p $(BUILD)
	@{ \
	echo '<!DOCTYPE html>'; \
	echo '<html lang="en"><head>'; \
	echo '<meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">'; \
	echo '<title>leandronsp.com</title>'; \
	echo '<link rel="stylesheet" href="style.css">'; \
	echo '</head><body>'; \
	echo '<main>'; \
	echo '<p class="site-title">leandronsp.com</p>'; \
	echo '<p class="site-subtitle">software engineering, terminal life, and clean code</p>'; \
	echo '<ul class="post-list">'; \
	for f in $$(ls -r posts/*.md); do \
		title=$$(grep '^title:' $$f | sed 's/^title: *//'); \
		date=$$(grep '^date:' $$f | sed 's/^date: *//'); \
		desc=$$(grep '^description:' $$f | sed 's/^description: *//'); \
		slug=$$(basename $$f .md); \
		echo "<li><time>$$date</time><a href=\"$$slug.html\">$$title</a><p class=\"post-desc\">$$desc</p></li>"; \
	done; \
	echo '</ul></main></body></html>'; \
	} > $(BUILD)/index.html
	@echo "  built $(BUILD)/index.html"

blog.serve: blog.build ## Build and serve on localhost:8000
	@echo "  serving at http://localhost:8000"
	@cd $(BUILD) && python3 -m http.server 8000

blog.clean: ## Remove generated files
	@rm -rf $(BUILD)
