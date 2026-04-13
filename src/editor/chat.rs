//! Chat pane for interactive writing companion.
//! Supports conversation with draft context and optional vault search.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// All mutable chat state.
pub struct ChatState {
    pub messages: Vec<Message>,
    pub input: String,
    pub pending: bool,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            pending: false,
        }
    }

    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::User,
            content: content.to_string(),
        });
    }

    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::Assistant,
            content: content.to_string(),
        });
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.input.clear();
        self.pending = false;
    }

    /// Render chat messages as ratatui lines.
    pub fn render_messages(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for msg in &self.messages {
            let (prefix, color) = match msg.role {
                Role::User => ("You: ", Color::Cyan),
                Role::Assistant => ("AI: ", Color::Green),
            };
            // Split multi-line responses. First line gets the prefix.
            let content_lines: Vec<&str> = msg.content.lines().collect();
            if content_lines.is_empty() {
                lines.push(Line::from(Span::styled(prefix, Style::default().fg(color))));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(color)),
                    Span::styled(content_lines[0].to_string(), Style::default().fg(color)),
                ]));
                for content_line in &content_lines[1..] {
                    lines.push(Line::from(Span::styled(
                        format!("  {content_line}"),
                        Style::default().fg(color),
                    )));
                }
            }
            lines.push(Line::from(""));
        }
        if self.pending {
            lines.push(Line::from(Span::styled(
                "thinking...",
                Style::default().fg(Color::Yellow),
            )));
        }
        lines
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract YAML frontmatter from markdown content.
pub fn extract_frontmatter(content: &str) -> String {
    if !content.starts_with("---") {
        return String::new();
    }
    if let Some(end) = content[3..].find("\n---") {
        content[..end + 3 + 4].to_string()
    } else {
        String::new()
    }
}

/// Build the LLM prompt for a chat turn. Sends frontmatter + visible window
/// (with margin) instead of the full article to stay within token limits.
pub fn build_chat_prompt(
    frontmatter: &str,
    visible_window: &str,
    vault_context: Option<&str>,
    messages: &[Message],
    question: &str,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(
        "You are a writing companion helping draft a blog article. \
         The article content is shown below. Read it carefully.\n\n\
         Rules:\n\
         - Answer in the same language as the question.\n\
         - Engage with the CONTENT. Don't summarize what's already written. Push the writing forward.\n\
         - When asked about structure, angles, or phrasing: suggest concrete text, not generic advice.\n\
         - When asked to review: be direct about what's weak and how to fix it.\n\
         - Keep responses short. No fluff.\n\
         - Only use vault_search when the user explicitly asks about their vault, notes, or knowledge base. \
         Do NOT search the vault for every question.\n\
         - vault_search uses BM25. Pass short English keywords, not sentences.\n\n",
    );

    if !frontmatter.is_empty() {
        prompt.push_str("## Article Metadata\n\n");
        prompt.push_str(frontmatter);
        prompt.push_str("\n\n");
    }

    prompt.push_str("## Visible Section (current editing area + margin)\n\n");
    prompt.push_str(visible_window);
    prompt.push_str("\n\n");

    if let Some(vault) = vault_context {
        prompt.push_str("## Related Notes from Vault\n\n");
        prompt.push_str(vault);
        prompt.push_str("\n\n");
    }

    if !messages.is_empty() {
        prompt.push_str("## Conversation History\n\n");
        // Keep last 6 messages (3 exchanges) to stay within token budget
        let recent = if messages.len() > 6 {
            &messages[messages.len() - 6..]
        } else {
            messages
        };
        for msg in recent {
            let role = match msg.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
            };
            prompt.push_str(&format!("{role}: {}\n", msg.content));
        }
        prompt.push('\n');
    }

    prompt.push_str(&format!("User: {question}"));
    prompt
}

/// Safe vault paths: only learnings, lives, and blog drafts.
const VAULT_SAFE_PREFIXES: &[&str] = &[
    "qmd://vault/learning/",
    "qmd://vault/lives/",
    "qmd://vault/blog/",
];

/// Query the vault via qmd search (BM25, fast, no LLM).
/// Filters results to safe paths only (learning, lives, blog).
/// Returns None if qmd is not installed or no results.
pub fn query_vault(question: &str) -> Option<String> {
    let output = std::process::Command::new("qmd")
        .args(["search", "-n", "10", "--json", question])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).ok()?;

    let mut results = Vec::new();
    for item in &json {
        let file = item["file"].as_str().unwrap_or_default();
        if !VAULT_SAFE_PREFIXES.iter().any(|p| file.starts_with(p)) {
            continue;
        }
        let title = item["title"].as_str().unwrap_or("untitled");
        let snippet = item["snippet"].as_str().unwrap_or_default();
        results.push(format!("**{title}** ({file})\n{snippet}"));
        if results.len() >= 3 {
            break;
        }
    }

    if results.is_empty() {
        return None;
    }

    let text = results.join("\n\n");
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_chat_has_no_messages() {
        let chat = ChatState::new();
        assert!(chat.messages.is_empty());
        assert!(chat.input.is_empty());
        assert!(!chat.pending);
    }

    #[test]
    fn add_user_message_appends_to_history() {
        let mut chat = ChatState::new();
        chat.add_user_message("What is this about?");
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].role, Role::User);
        assert_eq!(chat.messages[0].content, "What is this about?");
    }

    #[test]
    fn add_assistant_message_appends_to_history() {
        let mut chat = ChatState::new();
        chat.add_assistant_message("This is a blog post about Rust.");
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].role, Role::Assistant);
    }

    #[test]
    fn clear_resets_all_state() {
        let mut chat = ChatState::new();
        chat.add_user_message("hello");
        chat.add_assistant_message("hi");
        chat.input = "draft question".to_string();
        chat.pending = true;

        chat.clear();

        assert!(chat.messages.is_empty());
        assert!(chat.input.is_empty());
        assert!(!chat.pending);
    }

    #[test]
    fn render_messages_shows_role_prefixes() {
        let mut chat = ChatState::new();
        chat.add_user_message("question");
        chat.add_assistant_message("answer");

        let lines = chat.render_messages();

        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.iter().any(|l| l.contains("You: question")));
        assert!(text.iter().any(|l| l.contains("AI: answer")));
    }

    #[test]
    fn render_messages_shows_thinking_when_pending() {
        let mut chat = ChatState::new();
        chat.pending = true;

        let lines = chat.render_messages();

        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.iter().any(|l| l.contains("thinking...")));
    }

    #[test]
    fn render_multiline_assistant_uses_consistent_color() {
        // Bug: first line of AI response is white (Span::raw), continuation
        // lines are green. All content lines should use the same color.
        let mut chat = ChatState::new();
        chat.add_assistant_message("First paragraph.\n\nSecond paragraph.");
        let lines = chat.render_messages();
        // Line 0: "AI: First paragraph." (prefix + content)
        // Line 1: "  " (blank from LLM)
        // Line 2: "  Second paragraph."
        let first_content_span = &lines[0].spans[1]; // content span after prefix
        let continuation_span = &lines[2].spans[0];
        assert_eq!(
            first_content_span.style.fg, continuation_span.style.fg,
            "first line fg {:?} != continuation fg {:?}",
            first_content_span.style.fg, continuation_span.style.fg
        );
    }

    #[test]
    fn build_chat_prompt_includes_frontmatter_and_visible_window() {
        let fm = "---\ntitle: My Post\n---";
        let window = "Some visible content.";
        let prompt = build_chat_prompt(fm, window, None, &[], "Is this clear?");

        assert!(prompt.contains("title: My Post"));
        assert!(prompt.contains("Some visible content."));
        assert!(prompt.contains("User: Is this clear?"));
    }

    #[test]
    fn build_chat_prompt_includes_vault_context_when_present() {
        let prompt = build_chat_prompt(
            "", "draft",
            Some("TIL: Rust ownership rules"),
            &[], "question",
        );

        assert!(prompt.contains("Related Notes from Vault"));
        assert!(prompt.contains("TIL: Rust ownership rules"));
    }

    #[test]
    fn build_chat_prompt_includes_conversation_history() {
        let messages = vec![
            Message { role: Role::User, content: "first question".to_string() },
            Message { role: Role::Assistant, content: "first answer".to_string() },
        ];

        let prompt = build_chat_prompt("", "draft", None, &messages, "follow up");

        assert!(prompt.contains("User: first question"));
        assert!(prompt.contains("Assistant: first answer"));
        assert!(prompt.contains("User: follow up"));
    }

    #[test]
    fn build_chat_prompt_omits_vault_results_when_none() {
        let prompt = build_chat_prompt("", "draft", None, &[], "question");

        assert!(!prompt.contains("Related Notes from Vault"));
    }

    #[test]
    fn extract_frontmatter_parses_yaml_block() {
        let content = "---\ntitle: Hello\ntags: [a]\n---\n\nBody here.";
        let fm = extract_frontmatter(content);
        assert!(fm.contains("title: Hello"));
        assert!(!fm.contains("Body here"));
    }

    #[test]
    fn extract_frontmatter_returns_empty_when_missing() {
        let fm = extract_frontmatter("No frontmatter here.");
        assert!(fm.is_empty());
    }
}
