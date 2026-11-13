//! Hybrid history-window management (opt-in `[history]`).
//!
//! Pure helpers used by the agent loop: a cheap token estimate, selection of
//! the "old" span eligible for compaction, and transcript rendering for the
//! summarization request. The actual provider summarization call and the
//! splice/drop mutation live in `agent.rs`; everything here is side-effect free
//! and unit-tested.

use crate::ai::Message;

/// Per-message estimate: the content's characters, weighted CJK-heavy
/// (CJK codepoints tokenize at roughly one token per ~1.5 characters, Latin
/// text at ~4, so a weighted character count ÷ 4 stays close on both sides of
/// the mix), plus the hidden fields a request also carries: an assistant's
/// `tool_calls` argument JSON and its `reasoning_content` (DeepSeek thinking
/// mode echoes them back — omitting them from the estimate let a
/// JSON-heavy/CJK history blow far past the configured budget before the
/// estimate said anything).
fn message_weighted_chars(m: &Message) -> usize {
    let mut chars = match m {
        Message::System { content } | Message::User { content } => weighted_chars(content),
        Message::Assistant {
            content,
            tool_calls,
            reasoning_content,
        } => {
            let extra: usize = tool_calls
                .iter()
                .map(|c| {
                    let mut n = c.arguments.to_string().chars().count();
                    n += c.name.len();
                    n += c.id.len();
                    n
                })
                .sum();
            let reasoning = reasoning_content
                .as_deref()
                .map(weighted_chars)
                .unwrap_or(0);
            weighted_chars(content) + extra + reasoning
        }
        Message::ToolResult { content, .. } => weighted_chars(content),
    };
    // Tool-call envelopes (the "tool_use_id"/name scaffolding the wire adds
    // per result) are invisible to the content-only estimate; a small
    // per-message constant keeps the sum close to the real prompt.
    chars += 16;
    chars
}

fn weighted_chars(s: &str) -> usize {
    // CJK wide characters ≈ 1 token per 1–1.5 chars; a single token spans
    // ~4 "narrow" chars. Counting every wide char as 3 narrow ones keeps the
    // estimate ≥ the real token count for the CJK-heavy histories (the
    // budget must not be exceeded, so over-estimating CJK is the safe side),
    // while narrow text divides by 4 exactly as before.
    s.chars().map(|c| if is_wide(c) { 3 } else { 1 }).sum()
}

/// Whether `c` renders wide (CJK ideographs, kana, hangul, fullwidth forms,
/// CJK punctuation) — the ranges that tokenize near one token per character.
fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F   // Hangul Jamo
        | 0x2E80..=0x303E // CJK Radicals, Kangxi, CJK Symbols and Punctuation
        | 0x3041..=0x33FF // Hiragana, Katakana, CJK compat
        | 0x3400..=0x4DBF // CJK Ext A
        | 0x4E00..=0x9FFF // CJK Unified Ideographs
        | 0xA000..=0xA4CF // Yi
        | 0xAC00..=0xD7A3 // Hangul Syllables
        | 0xF900..=0xFAFF // CJK Compatibility Ideographs
        | 0xFE30..=0xFE4F // CJK Compatibility Forms
        | 0xFF00..=0xFF60 // Fullwidth Forms
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x2FA1F // CJK Ext B..F
    )
}

/// Rough token estimate over all messages' payload — contents, tool-call
/// argument JSON, reasoning content — with CJK-heavy weighting, all divided
/// by 4. Deliberately tokenizer-free to avoid a heavy dependency; the estimate
/// must err **over** the real token count (an under-estimate lets the real
/// request exceed the provider's context window and die with
/// "prompt too long", so the weighting rounds up where it can).
pub fn estimate_tokens(messages: &[Message]) -> usize {
    let chars: usize = messages.iter().map(message_weighted_chars).sum();
    chars / 4
}

/// Index range `[start, end)` of messages eligible for compaction: everything
/// after the leading `System` prefix and before the last `keep_recent_turns`
/// messages. `None` when nothing qualifies (too short / all system / all
/// recent).
pub fn old_span(messages: &[Message], keep_recent_turns: usize) -> Option<std::ops::Range<usize>> {
    let start = messages
        .iter()
        .position(|m| !matches!(m, Message::System { .. }))
        .unwrap_or(messages.len());
    let total = messages.len();
    if total <= start {
        return None;
    }
    let recent = keep_recent_turns.min(total - start);
    let mut end = total - recent;
    // Never let the kept (un-summarized) tail begin with a `ToolResult`:
    // splitting an `Assistant { tool_calls }` from its following tool
    // result(s) would leave an orphan `tool` message with no qualifying
    // predecessor, which OpenAI-shaped providers (DeepSeek via opencode)
    // reject with HTTP 400. Pull the boundary back so the whole
    // assistant/tool-result group stays together in the kept tail (and is
    // not what gets collapsed into the summary).
    while end > start && end < total && matches!(messages[end], Message::ToolResult { .. }) {
        end -= 1;
    }
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
            Message::Assistant { content, .. } => ("Assistant", content),
            Message::ToolResult { content, .. } => ("ToolResult", content),
        };
        s.push_str(tag);
        s.push_str(": ");
        s.push_str(c);
        s.push_str("\n\n");
    }
    s.trim_end().to_string()
}

/// Truncate **oversized `ToolResult` contents inside the kept window** — the
/// compact path's last resort. Summarization and the drop loop can only
/// touch the old span; the kept tail (the most recent turns) is preserved
/// verbatim by the contract, so one enormous tool result sitting in it —
/// pages of a beat log, a build dump — was previously unremovable and drove
/// the real request past the provider's context window ("prompt too long").
///
/// For every `ToolResult` in the kept window whose **weighted** size exceeds
/// `budget_chars`, the content is cut to at most that many weighted-width
/// characters, keeping the head (where the tool begins its answer) and
/// appending a marker naming what was cut. A truncation the model cannot
/// notice is not a truncation, so the marker is explicit. Returns the number
/// of results truncated. Contents already within the budget are untouched,
/// and non-tool messages never are.
pub fn truncate_tail_tool_results(
    messages: &mut [Message],
    keep_recent_turns: usize,
    budget_chars: usize,
) -> usize {
    let total = messages.len();
    // The kept window: the last `keep_recent_turns` messages (never more
    // than the message count). A message that is both inside the kept
    // window AND a ToolResult is the rescue target — the old span's results
    // belong to the summarizer/drop path, not this one.
    let window_start = total.saturating_sub(keep_recent_turns);
    let mut truncated = 0usize;
    for m in &mut messages[window_start..] {
        if let Message::ToolResult {
            tool_use_id: _,
            content,
            is_error: _,
        } = m
        {
            let measured = weighted_chars(content);
            if measured <= budget_chars {
                continue;
            }
            // Cut by weighted width, so the kept head has the same share of
            // the budget whichever script the content is in.
            let mut kept_units = 0usize;
            let mut cut_at = content.len();
            for (idx, c) in content.char_indices() {
                if kept_units > budget_chars {
                    cut_at = idx;
                    break;
                }
                kept_units += if is_wide(c) { 3 } else { 1 };
            }
            let dropped = content.chars().count() - content[..cut_at].chars().count();
            let marker = format!("\n… [truncated {dropped} characters by history compaction]");
            *content = content[..cut_at].to_string() + &marker;
            truncated += 1;
        }
    }
    truncated
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
        Message::Assistant {
            content: s.into(),
            tool_calls: vec![],
            reasoning_content: None,
        }
    }

    #[test]
    fn estimate_is_weighted_chars_over_four_including_overhead() {
        // 12 narrow chars (8 + 4), no tool calls, no reasoning: each message
        // adds the 16-unit envelope, so (12 + 32) / 4 = 11.
        let msgs = vec![user("abcdefgh"), asst("ijkl")];
        assert_eq!(estimate_tokens(&msgs), 11);
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

    fn asst_call(content: &str, id: &str) -> Message {
        Message::Assistant {
            content: content.into(),
            tool_calls: vec![crate::ai::ToolCall {
                id: id.into(),
                name: "shell".into(),
                arguments: serde_json::json!({}),
            }],
            reasoning_content: None,
        }
    }
    fn tr(id: &str) -> Message {
        Message::ToolResult {
            tool_use_id: id.into(),
            content: "ok".into(),
            is_error: false,
        }
    }

    #[test]
    fn old_span_does_not_split_assistant_toolresult_pair() {
        // Without the boundary guard, keep=1 would put the span end at index
        // 3 (the ToolResult), summarizing away its Assistant{tool_calls} at
        // index 2 and orphaning the tool message. The guard must pull `end`
        // back to 2 so the pair stays in the kept tail.
        let msgs = vec![
            sys("persona"),       // 0
            user("u1"),           // 1
            asst_call("a", "c1"), // 2  (Assistant with tool_calls)
            tr("c1"),             // 3  (its ToolResult)
        ];
        let span = old_span(&msgs, 1).expect("span");
        assert_eq!(span, 1..2, "boundary pulled back before the pair");
        // The kept tail [2..] starts with the Assistant, not the ToolResult.
        assert!(matches!(msgs[span.end], Message::Assistant { .. }));
    }

    #[test]
    fn old_span_walks_back_over_multiple_tool_results() {
        let msgs = vec![
            sys("p"),             // 0
            user("u"),            // 1
            asst_call("a", "c1"), // 2
            tr("c1"),             // 3
            tr("c2"),             // 4  (multi-tool turn)
        ];
        // keep=1 → raw end=4 (tr c2); guard walks 4→3→2.
        let span = old_span(&msgs, 1).expect("span");
        assert_eq!(span, 1..2);
    }
    fn tr_big(id: &str, content: &str) -> Message {
        Message::ToolResult {
            tool_use_id: id.into(),
            content: content.into(),
            is_error: false,
        }
    }

    #[test]
    fn cjk_text_estimates_heavier_than_the_latin_rule() {
        // 8 wide chars + 16 per-message overhead = 40 weighted units -> 10
        // tokens, versus the latin rule's 8/4 = 2. The estimate must err high
        // for CJK: an under-estimate is exactly what let the real request
        // blow past the provider's window while the budget said it fit.
        let msgs = vec![user("一二三四五六七八")];
        assert_eq!(estimate_tokens(&msgs), 10);
        let msgs2 = vec![user("abcdefgh")];
        assert_eq!(estimate_tokens(&msgs2), 6); // 8+16=24 /4 = 6
    }

    #[test]
    fn estimate_counts_tool_call_arguments_and_reasoning() {
        let a = Message::Assistant {
            content: "running".into(),
            tool_calls: vec![crate::ai::ToolCall {
                id: "call-0123456789abcdef".into(),
                name: "bash".into(),
                arguments: serde_json::json!({"command": "x"}),
            }],
            reasoning_content: Some("chain-of-thought hidden from content".into()),
        };
        // content(7)+overhead(16)+args json+ids+reasoning chars, all /4 —
        // strictly more than the content-only rule ever counted (23/4=5).
        assert!(estimate_tokens(&[a]) > 5);
    }

    #[test]
    fn truncate_cuts_only_the_oversized_kept_tool_result() {
        let big = "x".repeat(400);
        let mut msgs = vec![sys("p"), user("u"), asst("a"), tr_big("c1", &big)];
        // keep=1: the kept window is the last message (the ToolResult).
        let n = truncate_tail_tool_results(&mut msgs, 1, 100);
        assert_eq!(n, 1);
        match &msgs[3] {
            Message::ToolResult { content, .. } => {
                assert!(content.starts_with("xxx"), "head kept");
                assert!(content.contains("truncated"), "marker present: {content}");
                assert!(content.len() < 300, "much smaller than the original");
            }
            other => panic!("expected the tool result, got {other:?}"),
        }
    }

    #[test]
    fn truncate_leaves_results_within_the_budget_untouched() {
        let small = "y".repeat(50);
        let mut msgs = vec![tr_big("c1", &small)];
        let n = truncate_tail_tool_results(&mut msgs, 1, 100);
        assert_eq!(n, 0);
        match &msgs[0] {
            Message::ToolResult { content, .. } => {
                assert_eq!(content.len(), 50, "untouched within the budget");
            }
            other => panic!("expected the tool result, got {other:?}"),
        }
    }

    #[test]
    fn truncate_never_touches_the_old_span() {
        let big = "z".repeat(400);
        let mut msgs = vec![
            sys("p"),
            user("u"),
            asst("a"),
            tr_big("c1", &big),
            asst("last"),
        ];
        // keep=1 keeps only the last message (not a ToolResult) — nothing in
        // the kept window is oversized, the old span's monster stays (the
        // summarizer/drop path owns it).
        let n = truncate_tail_tool_results(&mut msgs, 1, 100);
        assert_eq!(n, 0);
        match &msgs[3] {
            Message::ToolResult { content, .. } => {
                assert_eq!(content.len(), 400, "old span untouched");
            }
            other => panic!("expected the tool result, got {other:?}"),
        }
    }
}
