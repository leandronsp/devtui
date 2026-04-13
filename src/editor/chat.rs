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
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(color)),
                Span::raw(msg.content.clone()),
            ]));
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

/// Build the LLM prompt for a chat turn. Includes the draft as context,
/// optional vault results, conversation history, and the user's question.
pub fn build_chat_prompt(
    draft: &str,
    vault_context: Option<&str>,
    messages: &[Message],
    question: &str,
) -> String {
    let mut prompt = String::new();

    prompt.push_str("You are a writing companion. The user is editing a blog post.\n\n");
    prompt.push_str("## Current Draft\n\n");
    // Truncate draft to ~3000 chars to stay within token limits
    let truncated = if draft.len() > 3000 {
        let cut = &draft[..draft[..3000].rfind('\n').unwrap_or(3000)];
        format!("{cut}\n\n[...draft truncated, {} total chars...]", draft.len())
    } else {
        draft.to_string()
    };
    prompt.push_str(&truncated);
    prompt.push_str("\n\n");

    if let Some(vault) = vault_context {
        prompt.push_str("## Related Notes from Vault\n\n");
        prompt.push_str(vault);
        prompt.push_str("\n\n");
    }

    if !messages.is_empty() {
        prompt.push_str("## Conversation History\n\n");
        for msg in messages {
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

/// Query the vault via qmd for related context. Returns None if qmd is not installed.
pub fn query_vault(question: &str) -> Option<String> {
    let output = std::process::Command::new("qmd")
        .args(["query", "-c", "vault", question])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
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
    fn build_chat_prompt_includes_draft_and_question() {
        let prompt = build_chat_prompt("# My Post\n\nSome content.", None, &[], "Is this clear?");

        assert!(prompt.contains("# My Post"));
        assert!(prompt.contains("Some content."));
        assert!(prompt.contains("User: Is this clear?"));
    }

    #[test]
    fn build_chat_prompt_includes_vault_context_when_present() {
        let prompt = build_chat_prompt(
            "draft",
            Some("TIL: Rust ownership rules"),
            &[],
            "question",
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

        let prompt = build_chat_prompt("draft", None, &messages, "follow up");

        assert!(prompt.contains("User: first question"));
        assert!(prompt.contains("Assistant: first answer"));
        assert!(prompt.contains("User: follow up"));
    }

    #[test]
    fn build_chat_prompt_omits_vault_section_when_none() {
        let prompt = build_chat_prompt("draft", None, &[], "question");

        assert!(!prompt.contains("Vault"));
    }

    #[test]
    fn build_chat_prompt_truncates_long_drafts() {
        let long_draft = "word ".repeat(1000); // ~5000 chars
        let prompt = build_chat_prompt(&long_draft, None, &[], "question");

        assert!(prompt.contains("truncated"));
        assert!(prompt.len() < long_draft.len());
    }
}
