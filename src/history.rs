//! Hybrid history-window management (opt-in `[history]`).
//!
//! Pure helpers used by the agent loop: a cheap token estimate, selection of
//! the "old" span eligible for compaction, and transcript rendering for the
//! summarization request. The actual provider summarization call and the
//! splice/drop mutation live in `agent.rs`; everything here is side-effect free
//! and unit-tested.

use crate::ai::Message;

fn message_len(m: &Message) -> usize {
    match m {
        Message::System { content }
        | Message::User { content }
        | Message::Assistant { content } => content.len(),
        Message::ToolResult { content, .. } => content.len(),
    }
}

/// Rough token estimate (~4 chars per token) over all message contents.
/// Deliberately tokenizer-free to avoid a heavy dependency; only the relative
/// magnitude versus the configured budget matters.
pub fn estimate_tokens(messages: &[Message]) -> usize {
    let chars: usize = messages.iter().map(message_len).sum();
    chars / 4
}

/// Index range `[start, end)` of messages eligible for compaction: everything
/// after the leading `System` prefix and before the last `keep_recent_turns`
/// messages. `None` when nothing qualifies (too short / all system / all
/// recent).
pub fn old_span(
    messages: &[Message],
    keep_recent_turns: usize,
) -> Option<std::ops::Range<usize>> {
    let start = messages
        .iter()
        .position(|m| !matches!(m, Message::System { .. }))
        .unwrap_or(messages.len());
    let total = messages.len();
    if total <= start {
        return None;
    }
    let recent = keep_recent_turns.min(total - start);
    let end = total - recent;
    if end > start {
        Some(start..end)
    } else {
        None
    }
}

/// Render a message slice as a plain transcript for the summarization prompt.
pub fn render_transcript(messages: &[Message]) -> String {
    let mut s = String::new();
    for m in messages {
        let (tag, c) = match m {
            Message::System { content } => ("System", content),
            Message::User { content } => ("User", content),
            Message::Assistant { content } => ("Assistant", content),
            Message::ToolResult { content, .. } => ("ToolResult", content),
        };
        s.push_str(tag);
        s.push_str(": ");
        s.push_str(c);
        s.push_str("\n\n");
    }
    s.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sys(s: &str) -> Message {
        Message::System { content: s.into() }
    }
    fn user(s: &str) -> Message {
        Message::User { content: s.into() }
    }
    fn asst(s: &str) -> Message {
        Message::Assistant { content: s.into() }
    }

    #[test]
    fn estimate_is_total_chars_over_four() {
        let msgs = vec![user("abcdefgh"), asst("ijkl")]; // 12 chars
        assert_eq!(estimate_tokens(&msgs), 3);
    }

    #[test]
    fn old_span_excludes_system_prefix_and_recent_turns() {
        let msgs = vec![
            sys("persona"),
            user("u1"),
            asst("a1"),
            user("u2"),
            asst("a2"),
            user("u3"),
        ];
        // keep_recent_turns = 2 → keep last 2 (a2,u3); old span = [1,4)
        let span = old_span(&msgs, 2).expect("span");
        assert_eq!(span, 1..4);
    }

    #[test]
    fn old_span_none_when_all_recent_or_system() {
        assert!(old_span(&[sys("s"), sys("s2")], 4).is_none());
        let msgs = vec![sys("s"), user("u1"), asst("a1")];
        assert!(old_span(&msgs, 4).is_none(), "keep covers everything");
    }

    #[test]
    fn render_transcript_tags_roles() {
        let t = render_transcript(&[user("hi"), asst("yo")]);
        assert_eq!(t, "User: hi\n\nAssistant: yo");
    }
}
