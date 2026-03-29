# The Art of Writing Clean Code

Writing clean code is not about following rules blindly. It is about **communication**. Code is read far more often than it is written.

## Principles That Matter

Here are the principles I follow daily:

- Keep functions small and focused
- Name things descriptively
- Avoid premature abstraction
  - Wait for the third repetition
  - Even then, question it
    - Sometimes three lines are better
    - Than one clever abstraction

## Code Examples

Inline code like `def hello` or `mix phx.server` should render nicely.

Here is a block of code:

```elixir
defmodule Blog do
  def list_articles do
    Repo.all(Article)
  end
end
```

## Formatting

This paragraph has **bold text**, *italic text*, and ***bold italic*** together. Also ~~strikethrough~~ for deleted content.

> This is a blockquote. It should be indented and styled differently.
> Multiple lines in the same quote.

## Lists Galore

Simple list:

- First item
- Second item
- Third item

Nested list:

- Languages I use
  - Elixir
  - Rust
  - Go
- Tools I love
  - Neovim
  - tmux
  - Git

Deep nesting:

- Level 1
  - Level 2
    - Level 3
      - Level 4

Ordered-ish content (markdown unordered):

- Step 1: Install dependencies
- Step 2: Configure the database
- Step 3: Run migrations

## Links and References

Check out [my blog](https://leandronsp.com) for more articles.

## Horizontal Rules

Above the line.

---

Below the line.

## Final Thoughts

The best code is the code you **don't write**. Every line must justify its existence. *Less is more*.

- Remove before adding
- Simplify before optimizing
- Understand before changing
