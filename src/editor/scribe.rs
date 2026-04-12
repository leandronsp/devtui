use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use serde::Deserialize;

/// Annotation severity tier.
#[derive(Debug, Clone, PartialEq)]
pub enum Tier {
    Error,
    Hint,
    Research,
}

/// A single annotation from the AI writing companion.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub line: u16,
    pub tier: Tier,
    pub message: String,
}

#[derive(Deserialize)]
struct RawAnnotation {
    line: u16,
    tier: String,
    message: String,
}

/// Parse JSON annotations from the AI response.
///
/// Expected format:
/// ```json
/// [{"line": 5, "tier": "error", "message": "Misspelled: 'teh' → 'the'"}]
/// ```
pub fn parse_annotations(json: &str) -> Vec<Annotation> {
    let raw: Vec<RawAnnotation> = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(err) => {
            log::warn!("parse_annotations: invalid JSON: {err}");
            return Vec::new();
        }
    };

    let mut annotations: Vec<Annotation> = raw
        .into_iter()
        .filter_map(|r| {
            let tier = match r.tier.as_str() {
                "error" => Tier::Error,
                "hint" => Tier::Hint,
                "research" => Tier::Research,
                _ => return None,
            };
            Some(Annotation { line: r.line, tier, message: r.message })
        })
        .collect();

    annotations.sort_by_key(|a| {
        let tier_rank = match a.tier {
            Tier::Error => 0u8,
            Tier::Hint => 1,
            Tier::Research => 2,
        };
        (a.line, tier_rank)
    });
    annotations
}

/// Build the prompt sent to the AI writing companion.
/// `start_line` is the 1-based line number of the first line in `content`,
/// used so the AI returns absolute line numbers matching the full document.
pub fn build_check_prompt(content: &str, start_line: usize) -> String {
    let numbered: String = content
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{}: {line}", start_line + i))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"Proofread this markdown. Lines are numbered. Detect language automatically.
Skip fenced code blocks. Be concise. Max 15 words per message.

Return ONLY a JSON array, no other text:
[{{"line": N, "tier": "error|hint", "message": "..."}}]

error = typo, grammar, wrong word. Show the fix.
hint = awkward phrasing, factual issue. Suggest briefly.

If clean, return []

{numbered}"#
    )
}

/// Extract JSON annotations from a potentially noisy AI response.
/// Looks for the first `[` to last `]` substring and parses it.
pub fn extract_annotations(response: &str) -> Vec<Annotation> {
    let start = match response.find('[') {
        Some(pos) => pos,
        None => return Vec::new(),
    };
    let end = match response.rfind(']') {
        Some(pos) => pos,
        None => return Vec::new(),
    };
    if end <= start {
        return Vec::new();
    }
    parse_annotations(&response[start..=end])
}

/// Render annotations with visible-line focus. Annotations for lines within
/// `visible_start..visible_end` appear first and bright. Others are dimmed.
pub fn render_lines_with_focus(
    annotations: &[Annotation],
    visible_start: u16,
    visible_end: u16,
) -> Vec<Line<'static>> {
    let is_visible = |a: &Annotation| a.line >= visible_start && a.line <= visible_end;

    let mut visible: Vec<&Annotation> = annotations.iter().filter(|a| is_visible(a)).collect();
    let mut offscreen: Vec<&Annotation> = annotations.iter().filter(|a| !is_visible(a)).collect();
    visible.sort_by_key(|a| a.line);
    offscreen.sort_by_key(|a| a.line);

    let mut lines = Vec::with_capacity(annotations.len());

    for a in visible {
        let icon = tier_icon(&a.tier);
        lines.push(Line::from(format!("{icon} L{}: {}", a.line, a.message)));
    }

    let dim = Style::default().fg(Color::DarkGray);
    for a in offscreen {
        let icon = tier_icon(&a.tier);
        lines.push(Line::from(Span::styled(
            format!("{icon} L{}: {}", a.line, a.message),
            dim,
        )));
    }

    lines
}

fn tier_icon(tier: &Tier) -> &'static str {
    match tier {
        Tier::Error => "✗",
        Tier::Hint => "💡",
        Tier::Research => "📎",
    }
}

/// Render annotations as ratatui lines for the scribe panel.
pub fn render_lines(annotations: &[Annotation]) -> Vec<Line<'static>> {
    annotations
        .iter()
        .map(|a| {
            let icon = tier_icon(&a.tier);
            Line::from(format!("{icon} L{}: {}", a.line, a.message))
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ScribeStatus {
    Idle,
    Checking,
    CheckingSlow,
    Error,
}

/// Info needed by the background thread to run a scribe check.
pub struct CheckRequest {
    pub session_name: String,
    pub session_needs_start: bool,
    pub content: String,
    pub start_line: usize,
}

/// All mutable scribe state, extracted for testability.
pub struct ScribeState {
    pub annotations: Vec<Annotation>,
    pub status: ScribeStatus,
    pub session_name: String,
    pub session_started: bool,
    pub error: Option<String>,
    pub last_response: Option<Instant>,
    pub result: Arc<Mutex<Option<Result<String, String>>>>,
    pub status_log: Vec<String>,
    idle_since: Option<Instant>,
    pending: bool,
    last_sent: String,
    check_started: Option<Instant>,
}

const IDLE_THRESHOLD: Duration = Duration::from_secs(10);
const SLOW_THRESHOLD: Duration = Duration::from_secs(15);

impl ScribeState {
    pub fn new(session_name: String) -> Self {
        Self {
            annotations: Vec::new(),
            status: ScribeStatus::Idle,
            session_name,
            session_started: false,
            error: None,
            last_response: None,
            result: Arc::new(Mutex::new(None)),
            status_log: Vec::new(),
            idle_since: None,
            pending: false,
            last_sent: String::new(),
            check_started: None,
        }
    }

    /// Clear display state when switching articles. Keeps session alive.
    pub fn clear_display(&mut self) {
        self.annotations.clear();
        self.status_log.clear();
        self.error = None;
        self.last_response = None;
        self.last_sent.clear();
        self.idle_since = None;
        self.status = ScribeStatus::Idle;
    }

    pub fn push_log(&mut self, msg: &str) {
        self.status_log.push(msg.to_string());
    }

    /// Signal that document content changed. Resets the idle timer.
    pub fn content_changed(&mut self) {
        self.idle_since = Some(Instant::now());
    }

    /// Returns true when a check should fire: idle long enough, content
    /// different from last send, and no check in flight.
    pub fn should_check(&self, content: &str, is_scribe_active: bool) -> bool {
        if !is_scribe_active || self.pending {
            return false;
        }
        let Some(idle_since) = self.idle_since else { return false };
        if idle_since.elapsed() < IDLE_THRESHOLD {
            return false;
        }
        content != self.last_sent
    }

    /// Prepare a check request. Transitions state to pending.
    /// `content` is the visible portion of the document.
    /// `start_line` is the 1-based line number of the first line in `content`.
    pub fn begin_check(&mut self, content: &str, start_line: usize) -> CheckRequest {
        let needs_start = !self.session_started;
        self.pending = true;
        self.status = ScribeStatus::Checking;
        self.status_log.clear();
        if needs_start {
            self.status_log.push("Starting session...".to_string());
        }
        self.status_log.push("Sending to scribe...".to_string());
        self.check_started = Some(Instant::now());
        self.last_sent = content.to_string();
        self.idle_since = None;
        self.session_started = true;

        CheckRequest {
            session_name: self.session_name.clone(),
            session_needs_start: needs_start,
            content: content.to_string(),
            start_line,
        }
    }

    /// Process a successful response from overmind.
    pub fn handle_response(&mut self, response: &str) {
        log::info!("[scribe] handle_response: {} bytes", response.len());
        self.annotations = extract_annotations(response);
        let count = self.annotations.len();
        let elapsed = self.check_started
            .map(|t| format!(" ({:.1}s)", t.elapsed().as_secs_f32()))
            .unwrap_or_default();
        log::info!("[scribe] parsed {count} annotations");
        self.status_log.clear();
        self.status_log.push(format!("{count} annotations{elapsed}"));
        self.error = None;
        self.status = ScribeStatus::Idle;
        self.pending = false;
        self.check_started = None;
        self.last_response = Some(Instant::now());
    }

    /// Process a failed overmind call.
    pub fn handle_error(&mut self, err: String) {
        log::warn!("scribe check failed: {err}");
        self.status_log.clear();
        self.status_log.push(format!("Error: {err}"));
        self.annotations.clear();
        self.error = Some(err);
        self.status = ScribeStatus::Error;
        self.pending = false;
        self.check_started = None;
    }

    /// Pick up result from the background thread (non-blocking).
    pub fn poll_result(&mut self) {
        let result = if let Ok(mut guard) = self.result.try_lock() {
            guard.take()
        } else {
            return;
        };
        if let Some(result) = result {
            match result {
                Ok(response) => self.handle_response(&response),
                Err(err) => self.handle_error(err),
            }
        }
    }

    /// Update status to slow if check has been running too long.
    pub fn update_slow(&mut self) {
        if let Some(started) = self.check_started {
            if self.pending && started.elapsed() > SLOW_THRESHOLD {
                self.status = ScribeStatus::CheckingSlow;
            }
        }
    }

    pub fn is_pending(&self) -> bool {
        self.pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> ScribeState {
        ScribeState::new("scribe-test".to_string())
    }

    // --- should_check ---

    #[test]
    fn should_check_returns_false_when_not_active() {
        let mut state = test_state();
        state.content_changed();
        // Simulate elapsed time by setting idle_since in the past
        state.idle_since = Some(Instant::now() - Duration::from_secs(15));
        assert!(!state.should_check("hello", false));
    }

    #[test]
    fn should_check_returns_false_when_pending() {
        let mut state = test_state();
        state.content_changed();
        state.idle_since = Some(Instant::now() - Duration::from_secs(15));
        state.pending = true;
        assert!(!state.should_check("hello", true));
    }

    #[test]
    fn should_check_returns_false_before_idle_threshold() {
        let mut state = test_state();
        state.content_changed(); // just now
        assert!(!state.should_check("hello", true));
    }

    #[test]
    fn should_check_returns_false_when_content_unchanged() {
        let mut state = test_state();
        state.last_sent = "hello".to_string();
        state.idle_since = Some(Instant::now() - Duration::from_secs(15));
        assert!(!state.should_check("hello", true));
    }

    #[test]
    fn should_check_returns_true_when_idle_and_content_changed() {
        let mut state = test_state();
        state.idle_since = Some(Instant::now() - Duration::from_secs(15));
        assert!(state.should_check("new content", true));
    }

    // --- begin_check ---

    #[test]
    fn begin_check_transitions_to_pending() {
        let mut state = test_state();
        let request = state.begin_check("some content", 1);
        assert!(state.is_pending());
        assert_eq!(state.status, ScribeStatus::Checking);
        assert_eq!(request.content, "some content");
        assert_eq!(request.start_line, 1);
    }

    #[test]
    fn begin_check_preserves_start_line() {
        let mut state = test_state();
        let request = state.begin_check("content", 42);
        assert_eq!(request.start_line, 42);
    }

    #[test]
    fn begin_check_stores_last_sent() {
        let mut state = test_state();
        state.begin_check("sent content", 1);
        assert_eq!(state.last_sent, "sent content");
    }

    // --- handle_response ---

    #[test]
    fn handle_response_parses_annotations() {
        let mut state = test_state();
        state.pending = true;
        let response = r#"[{"line": 5, "tier": "error", "message": "typo"}]"#;
        state.handle_response(response);
        assert_eq!(state.annotations.len(), 1);
        assert_eq!(state.annotations[0].line, 5);
        assert_eq!(state.status, ScribeStatus::Idle);
        assert!(!state.is_pending());
        assert!(state.last_response.is_some());
        assert!(state.error.is_none());
    }

    // --- handle_error ---

    #[test]
    fn handle_error_stores_error_and_resets_pending() {
        let mut state = test_state();
        state.pending = true;
        state.handle_error("overmind crashed".to_string());
        assert!(!state.is_pending());
        assert_eq!(state.status, ScribeStatus::Error);
        assert_eq!(state.error.as_deref(), Some("overmind crashed"));
        assert!(state.annotations.is_empty());
    }

    // --- update_slow ---

    #[test]
    fn update_slow_sets_checking_slow_after_threshold() {
        let mut state = test_state();
        state.pending = true;
        state.check_started = Some(Instant::now() - Duration::from_secs(20));
        state.status = ScribeStatus::Checking;
        state.update_slow();
        assert_eq!(state.status, ScribeStatus::CheckingSlow);
    }

    #[test]
    fn update_slow_does_nothing_before_threshold() {
        let mut state = test_state();
        state.pending = true;
        state.check_started = Some(Instant::now());
        state.status = ScribeStatus::Checking;
        state.update_slow();
        assert_eq!(state.status, ScribeStatus::Checking);
    }

    // --- poll_result ---

    #[test]
    fn poll_result_handles_success() {
        let mut state = test_state();
        state.pending = true;
        if let Ok(mut slot) = state.result.lock() {
            *slot = Some(Ok(r#"[{"line": 1, "tier": "hint", "message": "rephrase"}]"#.to_string()));
        }
        state.poll_result();
        assert!(!state.is_pending());
        assert_eq!(state.annotations.len(), 1);
        assert_eq!(state.status, ScribeStatus::Idle);
    }

    #[test]
    fn poll_result_handles_error() {
        let mut state = test_state();
        state.pending = true;
        if let Ok(mut slot) = state.result.lock() {
            *slot = Some(Err("connection failed".to_string()));
        }
        state.poll_result();
        assert!(!state.is_pending());
        assert_eq!(state.status, ScribeStatus::Error);
        assert_eq!(state.error.as_deref(), Some("connection failed"));
    }

    #[test]
    fn poll_result_is_noop_when_no_result() {
        let mut state = test_state();
        state.pending = true;
        state.status = ScribeStatus::Checking;
        state.poll_result();
        assert!(state.is_pending());
        assert_eq!(state.status, ScribeStatus::Checking);
    }

    // --- content_changed ---

    #[test]
    fn content_changed_resets_idle_timer() {
        let mut state = test_state();
        assert!(state.idle_since.is_none());
        state.content_changed();
        assert!(state.idle_since.is_some());
    }

    // --- clear_display ---

    #[test]
    fn clear_display_resets_annotations_and_log_but_keeps_session() {
        let mut state = test_state();
        state.session_started = true;
        state.annotations = vec![
            Annotation { line: 1, tier: Tier::Error, message: "typo".into() },
        ];
        state.status_log = vec!["5 annotations (2.1s)".to_string()];
        state.error = Some("old error".to_string());
        state.last_sent = "old content".to_string();
        state.status = ScribeStatus::Error;

        state.clear_display();

        assert!(state.annotations.is_empty());
        assert!(state.status_log.is_empty());
        assert!(state.error.is_none());
        assert!(state.last_sent.is_empty());
        assert_eq!(state.status, ScribeStatus::Idle);
        // Session stays alive
        assert!(state.session_started);
        assert_eq!(state.session_name, "scribe-test");
    }

    // --- push_log ---

    #[test]
    fn push_log_appends_messages() {
        let mut state = test_state();
        state.push_log("Starting session...");
        state.push_log("Sending...");
        assert_eq!(state.status_log.len(), 2);
        assert_eq!(state.status_log[0], "Starting session...");
        assert_eq!(state.status_log[1], "Sending...");
    }

    // --- status_log lifecycle ---

    #[test]
    fn begin_check_populates_status_log() {
        let mut state = test_state();
        state.begin_check("content", 1);
        assert!(!state.status_log.is_empty());
        assert!(state.status_log.iter().any(|l| l.contains("Sending")));
    }

    #[test]
    fn begin_check_first_time_logs_session_start() {
        let mut state = test_state();
        state.begin_check("content", 1);
        assert!(state.status_log.iter().any(|l| l.contains("Starting session")));
    }

    #[test]
    fn begin_check_warm_session_skips_session_start_log() {
        let mut state = test_state();
        state.session_started = true;
        state.begin_check("content", 1);
        assert!(!state.status_log.iter().any(|l| l.contains("Starting session")));
    }

    #[test]
    fn handle_response_clears_log_and_shows_count() {
        let mut state = test_state();
        state.pending = true;
        state.check_started = Some(Instant::now());
        state.status_log = vec!["Sending...".to_string()];
        state.handle_response(r#"[{"line": 1, "tier": "error", "message": "fix"}]"#);
        assert_eq!(state.status_log.len(), 1);
        assert!(state.status_log[0].contains("1 annotation"));
    }

    #[test]
    fn handle_error_clears_log_and_shows_error() {
        let mut state = test_state();
        state.pending = true;
        state.status_log = vec!["Sending...".to_string()];
        state.handle_error("timeout".to_string());
        assert_eq!(state.status_log.len(), 1);
        assert!(state.status_log[0].contains("Error: timeout"));
    }

    #[test]
    fn render_lines_with_focus_sorts_visible_first() {
        let annotations = vec![
            Annotation { line: 2, tier: Tier::Error, message: "off-screen error".into() },
            Annotation { line: 10, tier: Tier::Hint, message: "visible hint".into() },
            Annotation { line: 15, tier: Tier::Error, message: "visible error".into() },
            Annotation { line: 30, tier: Tier::Research, message: "off-screen ref".into() },
        ];

        let lines = render_lines_with_focus(&annotations, 8, 20);

        assert_eq!(lines.len(), 4);
        // Visible annotations (lines 10, 15) come first
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert!(text[0].contains("L10"));
        assert!(text[1].contains("L15"));
        // Off-screen annotations follow
        assert!(text[2].contains("L2"));
        assert!(text[3].contains("L30"));
    }

    #[test]
    fn render_lines_with_focus_dims_offscreen_annotations() {
        let annotations = vec![
            Annotation { line: 5, tier: Tier::Error, message: "off-screen".into() },
            Annotation { line: 15, tier: Tier::Error, message: "visible".into() },
        ];

        let lines = render_lines_with_focus(&annotations, 10, 20);

        // Visible annotation (line 15) should not be dimmed
        let visible_line = &lines[0];
        let visible_styles: Vec<_> = visible_line.spans.iter().map(|s| s.style).collect();
        assert!(visible_styles.iter().any(|s| s.fg != Some(Color::DarkGray)));

        // Off-screen annotation (line 5) should be dimmed
        let dim_line = &lines[1];
        let dim_styles: Vec<_> = dim_line.spans.iter().map(|s| s.style).collect();
        assert!(dim_styles.iter().all(|s| s.fg == Some(Color::DarkGray)));
    }

    #[test]
    fn build_check_prompt_includes_content_and_json_instruction() {
        let content = "# Hello\n\nSome text here.";
        let prompt = build_check_prompt(content, 1);
        assert!(prompt.contains("# Hello"));
        assert!(prompt.contains("Some text here."));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn build_check_prompt_instructs_to_skip_code_blocks() {
        let prompt = build_check_prompt("some text", 1);
        assert!(prompt.contains("code block"));
    }

    #[test]
    fn build_check_prompt_includes_line_numbers_starting_from_offset() {
        let content = "first line\nsecond line\nthird line";
        let prompt = build_check_prompt(content, 20);
        assert!(prompt.contains("20: first line"));
        assert!(prompt.contains("21: second line"));
        assert!(prompt.contains("22: third line"));
    }

    #[test]
    fn extract_annotations_from_noisy_response() {
        let response = r#"Here are the annotations I found:

[{"line": 3, "tier": "error", "message": "typo: 'teh' should be 'the'"}]

Let me know if you need more details."#;

        let annotations = extract_annotations(response);
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].line, 3);
        assert_eq!(annotations[0].tier, Tier::Error);
    }

    #[test]
    fn extract_annotations_returns_empty_on_no_json() {
        let annotations = extract_annotations("No issues found in this document.");
        assert!(annotations.is_empty());
    }

    #[test]
    fn render_lines_produces_annotated_output() {
        let annotations = vec![
            Annotation { line: 3, tier: Tier::Error, message: "Misspelled: 'teh'".into() },
            Annotation { line: 7, tier: Tier::Hint, message: "Consider rephrasing".into() },
            Annotation { line: 12, tier: Tier::Research, message: "See RFC 7231".into() },
        ];

        let lines = render_lines(&annotations);

        assert_eq!(lines.len(), 3);
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert_eq!(text[0], "✗ L3: Misspelled: 'teh'");
        assert_eq!(text[1], "💡 L7: Consider rephrasing");
        assert_eq!(text[2], "📎 L12: See RFC 7231");
    }

    #[test]
    fn render_lines_returns_empty_for_no_annotations() {
        let lines = render_lines(&[]);
        assert!(lines.is_empty());
    }

    #[test]
    fn parse_annotations_sorts_errors_before_hints_on_same_line() {
        let json = r#"[
            {"line": 5, "tier": "hint", "message": "rephrase"},
            {"line": 5, "tier": "error", "message": "spelling"}
        ]"#;
        let annotations = parse_annotations(json);
        assert_eq!(annotations[0].tier, Tier::Error);
        assert_eq!(annotations[1].tier, Tier::Hint);
    }

    #[test]
    fn parse_annotations_skips_unknown_tier() {
        let json = r#"[{"line": 1, "tier": "warning", "message": "x"}]"#;
        let annotations = parse_annotations(json);
        assert_eq!(annotations.len(), 0);
    }

    #[test]
    fn parse_annotations_returns_empty_on_invalid_json() {
        let annotations = parse_annotations("not json at all");
        assert_eq!(annotations.len(), 0);
    }

    #[test]
    fn parse_annotations_from_valid_json() {
        let json = r#"[
            {"line": 10, "tier": "hint", "message": "Consider rephrasing for clarity"},
            {"line": 3, "tier": "error", "message": "Misspelled: 'teh' → 'the'"},
            {"line": 7, "tier": "research", "message": "RFC 7231 supports this claim"}
        ]"#;

        let annotations = parse_annotations(json);

        assert_eq!(annotations.len(), 3);
        assert_eq!(annotations[0].line, 3);
        assert_eq!(annotations[0].tier, Tier::Error);
        assert_eq!(annotations[0].message, "Misspelled: 'teh' → 'the'");
        assert_eq!(annotations[1].line, 7);
        assert_eq!(annotations[1].tier, Tier::Research);
        assert_eq!(annotations[2].line, 10);
        assert_eq!(annotations[2].tier, Tier::Hint);
    }
}
