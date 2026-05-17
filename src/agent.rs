use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::stream::StreamExt;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use crate::ai::{Message, Provider, ProviderEvent, ToolCall};
use crate::config::Config;
use crate::error::Result;
use crate::id::AgentId;
use crate::log::{ConversationLog, LogEvent};
use crate::persona::Persona;
use crate::tools::{ToolCtx, ToolRegistry};

/// Tool execution approval request sent to the REPL input loop (FR-04-1 / design doc 4.3A).
///
/// The `agent` task waits for the user's y/N response via `response` oneshot::Sender.
pub struct ApprovalRequest {
    pub tool_name: String,
    pub args: Value,
    pub response: oneshot::Sender<bool>,
}

#[derive(Debug, Clone)]
pub enum AgentInput {
    UserPrompt(String),
    PeerPrompt {
        from: AgentId,
        from_name: Option<String>,
        text: String,
    },
    SetSystemPrompt(String),
    /// Initialize conversation history (keep only system prompt, remove all User/Assistant/ToolResult).
    /// Issued from the REPL command `/clear`.
    ClearHistory,
    Cancel,
}

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Thinking {
        text: String,
    },
    Text {
        delta: String,
    },
    ToolCall {
        name: String,
        args: Value,
    },
    ToolResult {
        name: String,
        ok: bool,
        output: String,
    },
    Done,
    Error {
        message: String,
    },
    Info {
        message: String,
    },
}

pub struct Agent {
    pub id: AgentId,
    #[allow(dead_code)]
    pub name: Option<String>,
    pub persona: Persona,
    pub provider: Box<dyn Provider>,
    pub tools: ToolRegistry,
    #[allow(dead_code)]
    pub config: Config,
    pub registry_dir: std::path::PathBuf,
    pub log: Option<ConversationLog>,
    /// Shared via `Arc<AtomicBool>` for runtime toggle via `/auto` REPL command (FR-04-2).
    pub auto_approve: Arc<AtomicBool>,
    /// Channel to send approval requests to the input loop. `None` means approval is not possible, so deny (FR-04-1).
    pub approval_tx: Option<mpsc::Sender<ApprovalRequest>>,
    pub history: Vec<Message>,
}

impl Agent {
    pub fn build_initial_history(persona: &Persona) -> Vec<Message> {
        vec![Message::System {
            content: persona.to_system_prompt(),
        }]
    }

    pub async fn run(
        mut self,
        mut input_rx: mpsc::Receiver<AgentInput>,
        event_tx: mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        let log = self.log.take();
        while let Some(msg) = input_rx.recv().await {
            match msg {
                AgentInput::UserPrompt(text) => {
                    if let Some(l) = &log {
                        l.write(LogEvent::User { text: &text }).await.ok();
                    }
                    self.history.push(Message::User { content: text });
                    self.process_turn(&event_tx, log.as_ref()).await?;
                }
                AgentInput::PeerPrompt {
                    from,
                    from_name,
                    text,
                } => {
                    if let Some(l) = &log {
                        l.write(LogEvent::PeerPrompt {
                            from: from.as_str(),
                            text: &text,
                        })
                        .await
                        .ok();
                    }
                    let header = match &from_name {
                        Some(n) => format!("[peer prompt from {} ({})]", n, from.as_str()),
                        None => format!("[peer prompt from {}]", from.as_str()),
                    };
                    let combined = format!("{header}\n{text}");
                    self.history.push(Message::User { content: combined });
                    let _ = event_tx
                        .send(AgentEvent::Info {
                            message: format!("peer prompt received from {}", from.as_str()),
                        })
                        .await;
                    self.process_turn(&event_tx, log.as_ref()).await?;
                }
                AgentInput::SetSystemPrompt(prompt) => {
                    if matches!(self.history.first(), Some(Message::System { .. })) {
                        self.history[0] = Message::System { content: prompt };
                    } else {
                        self.history.insert(0, Message::System { content: prompt });
                    }
                    let _ = event_tx
                        .send(AgentEvent::Info {
                            message: "system prompt updated".into(),
                        })
                        .await;
                }
                AgentInput::ClearHistory => {
                    // Rebuild initial history (System only) from the current persona.
                    // To replace the persona, use SetSystemPrompt separately.
                    let removed = self
                        .history
                        .iter()
                        .filter(|m| !matches!(m, Message::System { .. }))
                        .count();
                    self.history = Agent::build_initial_history(&self.persona);
                    let _ = event_tx
                        .send(AgentEvent::Info {
                            message: format!(
                                "conversation history cleared ({removed} message(s) removed; persona retained)"
                            ),
                        })
                        .await;
                }
                AgentInput::Cancel => {
                    let _ = event_tx
                        .send(AgentEvent::Info {
                            message: "cancel requested".into(),
                        })
                        .await;
                }
            }
        }
        Ok(())
    }

    async fn process_turn(
        &mut self,
        event_tx: &mpsc::Sender<AgentEvent>,
        log: Option<&ConversationLog>,
    ) -> Result<()> {
        // Hybrid history-window management (opt-in `[history]`). Runs before
        // the provider call so the compacted history is what gets sent.
        if self.config.history.enabled {
            self.maybe_compact_history(event_tx).await;
        }

        // Configurable cap (config.toml: [runtime] max_tool_iterations).
        // Default raised from the historical 8 to 24 so design-then-debug
        // orchestrators (the AI conductor generates HDL, then iterates on
        // lint feedback) finish their final fs_write inside the loop.
        let max_iterations = self.config.runtime.max_tool_iterations.max(1);
        for _ in 0..max_iterations {
            let specs = self.tools.specs();
            let mut stream = match self.provider.complete_stream(&self.history, &specs).await {
                Ok(s) => s,
                Err(e) => {
                    let _ = event_tx
                        .send(AgentEvent::Error {
                            message: e.to_string(),
                        })
                        .await;
                    // FR-03-2: Always emit a `Done` even on error, so the REPL input
                    // loop does not get stuck in Pending state.
                    let _ = event_tx.send(AgentEvent::Done).await;
                    return Ok(());
                }
            };

            let mut assistant_text = String::new();
            let mut pending_tools: Vec<(String, String, Value)> = Vec::new();
            let mut had_error = false;

            while let Some(ev) = stream.next().await {
                match ev {
                    ProviderEvent::Thinking { text } => {
                        if let Some(l) = log {
                            l.write(LogEvent::Thinking { text: &text }).await.ok();
                        }
                        let _ = event_tx.send(AgentEvent::Thinking { text }).await;
                    }
                    ProviderEvent::Text { delta } => {
                        assistant_text.push_str(&delta);
                        let _ = event_tx.send(AgentEvent::Text { delta }).await;
                    }
                    ProviderEvent::ToolUse { id, name, args } => {
                        pending_tools.push((id, name, args));
                    }
                    ProviderEvent::Error { message } => {
                        had_error = true;
                        let _ = event_tx
                            .send(AgentEvent::Error {
                                message: message.clone(),
                            })
                            .await;
                    }
                    ProviderEvent::Done => break,
                }
            }

            // Record the assistant turn (with the tool calls it made) BEFORE
            // any ToolResult is appended. A tool-only turn has empty text but
            // must still produce an assistant message carrying `tool_calls`,
            // otherwise the following `tool` message has no qualifying
            // predecessor and OpenAI-shaped providers (DeepSeek via opencode)
            // reject the next request with HTTP 400. Build from a borrow —
            // `pending_tools` is moved by the tool-execution loop below.
            let tool_calls: Vec<ToolCall> = pending_tools
                .iter()
                .map(|(id, name, args)| ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: args.clone(),
                })
                .collect();

            if !assistant_text.is_empty() || !tool_calls.is_empty() {
                if let Some(l) = log {
                    l.write(LogEvent::Assistant {
                        text: &assistant_text,
                    })
                    .await
                    .ok();
                }
                self.history.push(Message::Assistant {
                    content: assistant_text.clone(),
                    tool_calls,
                });
            }

            if had_error || pending_tools.is_empty() {
                let _ = event_tx.send(AgentEvent::Done).await;
                return Ok(());
            }

            // Tool execution
            let ctx = ToolCtx {
                self_id: self.id.clone(),
                registry_dir: self.registry_dir.clone(),
            };
            for (id, name, args) in pending_tools {
                let _ = event_tx
                    .send(AgentEvent::ToolCall {
                        name: name.clone(),
                        args: args.clone(),
                    })
                    .await;
                if let Some(l) = log {
                    l.write(LogEvent::ToolCall {
                        name: &name,
                        args: &args,
                    })
                    .await
                    .ok();
                }

                if !self.auto_approve.load(Ordering::SeqCst) {
                    let approved =
                        request_approval(self.approval_tx.as_ref(), name.clone(), args.clone())
                            .await;
                    if !approved {
                        let output = "user denied tool execution";
                        if let Some(l) = log {
                            l.write(LogEvent::ToolResult {
                                name: &name,
                                ok: false,
                                output,
                            })
                            .await
                            .ok();
                        }
                        let _ = event_tx
                            .send(AgentEvent::ToolResult {
                                name: name.clone(),
                                ok: false,
                                output: output.into(),
                            })
                            .await;
                        self.history.push(Message::ToolResult {
                            tool_use_id: id.clone(),
                            content: output.into(),
                            is_error: true,
                        });
                        continue;
                    }
                }

                let result = match self.tools.get(&name) {
                    Some(tool) => match tool.invoke(args.clone(), &ctx).await {
                        Ok(out) => out,
                        Err(e) => crate::tools::ToolOutput::err(format!("tool {name} error: {e}")),
                    },
                    None => crate::tools::ToolOutput::err(format!("tool not found: {name}")),
                };
                if let Some(l) = log {
                    l.write(LogEvent::ToolResult {
                        name: &name,
                        ok: result.ok,
                        output: &result.content,
                    })
                    .await
                    .ok();
                }
                let _ = event_tx
                    .send(AgentEvent::ToolResult {
                        name: name.clone(),
                        ok: result.ok,
                        output: result.content.clone(),
                    })
                    .await;
                self.history.push(Message::ToolResult {
                    tool_use_id: id,
                    content: result.content,
                    is_error: !result.ok,
                });
            }
            // Call provider again with tool results
        }
        let _ = event_tx
            .send(AgentEvent::Info {
                message: "max tool-use iterations reached".into(),
            })
            .await;
        let _ = event_tx.send(AgentEvent::Done).await;
        Ok(())
    }

    /// Hybrid history-window management (opt-in `[history]`). When the
    /// estimated context exceeds `max_context_tokens`, summarize the old span
    /// via the LLM into one system message; if still over budget, drop the
    /// oldest old-span messages. Best-effort: any summarization failure
    /// degrades to drop-only and never fails the turn. The leading
    /// system/persona prefix and the most recent `keep_recent_turns` are
    /// always preserved verbatim.
    async fn maybe_compact_history(&mut self, event_tx: &mpsc::Sender<AgentEvent>) {
        let max = self.config.history.max_context_tokens;
        let keep = self.config.history.keep_recent_turns;
        if crate::history::estimate_tokens(&self.history) <= max {
            return;
        }
        let span = match crate::history::old_span(&self.history, keep) {
            Some(s) if s.start < s.end => s,
            _ => return,
        };

        // Clone the old span out so the provider call does not borrow history.
        let transcript = crate::history::render_transcript(&self.history[span.clone()]);
        let n_old = span.len();
        let summ_msgs = vec![
            Message::System {
                content: "You are a summarizer. Condense the conversation excerpt into a \
                          compact summary that preserves facts, decisions, file paths, \
                          identifiers, and open tasks. Output only the summary."
                    .to_string(),
            },
            Message::User {
                content: format!("Summarize this earlier conversation excerpt:\n\n{transcript}"),
            },
        ];

        let mut summary = String::new();
        if let Ok(mut stream) = self.provider.complete_stream(&summ_msgs, &[]).await {
            while let Some(ev) = stream.next().await {
                match ev {
                    ProviderEvent::Text { delta } => summary.push_str(&delta),
                    ProviderEvent::Done | ProviderEvent::Error { .. } => break,
                    _ => {}
                }
            }
        }
        let summary = summary.trim().to_string();

        if !summary.is_empty() {
            let replacement = Message::System {
                content: format!("[Summary of {n_old} earlier message(s)]\n{summary}"),
            };
            self.history
                .splice(span.clone(), std::iter::once(replacement));
            let _ = event_tx
                .send(AgentEvent::Info {
                    message: format!("history compacted: {n_old} message(s) summarized"),
                })
                .await;
        }

        // Still over budget → drop oldest eligible messages one at a time.
        let mut dropped = 0usize;
        while crate::history::estimate_tokens(&self.history) > max {
            match crate::history::old_span(&self.history, keep) {
                Some(s) if s.start < s.end => {
                    self.history.remove(s.start);
                    dropped += 1;
                }
                _ => break,
            }
        }
        if dropped > 0 {
            let _ = event_tx
                .send(AgentEvent::Info {
                    message: format!("history compacted: {dropped} oldest message(s) dropped"),
                })
                .await;
        }
    }
}

/// Send an approval request to the REPL input loop and wait for a response (FR-04-1 / design doc 4.3A).
///
/// - `approval_tx` is `None` (tests or no approval handler connected): return `false` for safety.
/// - Send fails (input loop already terminated): `false`.
/// - oneshot receive fails (input loop discarded state): `false`.
///
/// Direct `std::io::stdin().read_line()` is prohibited because it competes with tokio's
/// stdin reader, causing the approval `y` input to be misattributed.
async fn request_approval(
    approval_tx: Option<&mpsc::Sender<ApprovalRequest>>,
    tool_name: String,
    args: Value,
) -> bool {
    let Some(tx) = approval_tx else {
        return false;
    };
    let (resp_tx, resp_rx) = oneshot::channel::<bool>();
    let req = ApprovalRequest {
        tool_name,
        args,
        response: resp_tx,
    };
    if tx.send(req).await.is_err() {
        return false;
    }
    resp_rx.await.unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokio::sync::mpsc;

    use crate::ai::testing::MockProvider;
    use crate::config::Config;
    use crate::persona::Persona;
    use crate::tools::ToolRegistry;

    fn build_test_agent(scripts: Vec<Vec<ProviderEvent>>, history: Vec<Message>) -> Agent {
        let cfg: Config = toml::from_str(crate::config::tests_default_config()).unwrap();
        let tools = ToolRegistry::build(&cfg, None, None);
        Agent {
            id: AgentId::new(),
            name: Some("test".into()),
            persona: Persona::builtin_default(),
            provider: Box::new(MockProvider::new(scripts)),
            tools,
            config: cfg,
            registry_dir: PathBuf::from("/tmp/agent-cli-tests"),
            log: None,
            auto_approve: Arc::new(AtomicBool::new(true)),
            approval_tx: None,
            history,
        }
    }

    #[tokio::test]
    async fn agent_emits_text_and_done() {
        let history = Agent::build_initial_history(&Persona::builtin_default());
        let agent = build_test_agent(
            vec![vec![
                ProviderEvent::Text {
                    delta: "hello ".into(),
                },
                ProviderEvent::Text {
                    delta: "world".into(),
                },
                ProviderEvent::Done,
            ]],
            history,
        );
        let (in_tx, in_rx) = mpsc::channel::<AgentInput>(8);
        let (ev_tx, mut ev_rx) = mpsc::channel::<AgentEvent>(32);

        let handle = tokio::spawn(async move { agent.run(in_rx, ev_tx).await });
        in_tx
            .send(AgentInput::UserPrompt("hi".into()))
            .await
            .unwrap();

        let mut text = String::new();
        let mut saw_done = false;
        while let Some(ev) = ev_rx.recv().await {
            match ev {
                AgentEvent::Text { delta } => text.push_str(&delta),
                AgentEvent::Done => {
                    saw_done = true;
                    break;
                }
                _ => {}
            }
        }
        assert_eq!(text, "hello world");
        assert!(saw_done);

        drop(in_tx);
        let _ = handle.await;
    }

    #[tokio::test]
    async fn agent_completes_tool_use_cycle_with_shell() {
        // Turn 1: request a shell tool call
        // Turn 2: reflect tool result and finish with text
        let scripts = vec![
            vec![
                ProviderEvent::ToolUse {
                    id: "call-1".into(),
                    name: "shell".into(),
                    args: serde_json::json!({"cmd": "echo agent-cli-test"}),
                },
                ProviderEvent::Done,
            ],
            vec![
                ProviderEvent::Text {
                    delta: "tool ran".into(),
                },
                ProviderEvent::Done,
            ],
        ];
        let history = Agent::build_initial_history(&Persona::builtin_default());
        let agent = build_test_agent(scripts, history);
        let (in_tx, in_rx) = mpsc::channel::<AgentInput>(8);
        let (ev_tx, mut ev_rx) = mpsc::channel::<AgentEvent>(32);

        let handle = tokio::spawn(async move { agent.run(in_rx, ev_tx).await });
        in_tx
            .send(AgentInput::UserPrompt("run echo".into()))
            .await
            .unwrap();

        let mut tool_called = false;
        let mut tool_ok = false;
        let mut text = String::new();
        let mut tool_output_contains_test = false;
        let mut saw_done = false;
        while let Some(ev) = ev_rx.recv().await {
            match ev {
                AgentEvent::ToolCall { name, .. } if name == "shell" => {
                    tool_called = true;
                }
                AgentEvent::ToolResult { name, ok, output } if name == "shell" => {
                    tool_ok = ok;
                    tool_output_contains_test = output.contains("agent-cli-test");
                }
                AgentEvent::Text { delta } => text.push_str(&delta),
                AgentEvent::Done => {
                    saw_done = true;
                    break;
                }
                _ => {}
            }
        }

        assert!(tool_called, "tool call event missing");
        assert!(tool_ok, "tool result was not ok");
        assert!(tool_output_contains_test, "stdout did not include marker");
        assert_eq!(text, "tool ran");
        assert!(saw_done);

        drop(in_tx);
        let _ = handle.await;
    }

    /// FR-04-3 / design doc 4.3B: Verify that when the tool_use loop reaches `max_tool_iterations`,
    /// `AgentEvent::Info { message: "max tool-use iterations reached" }` and
    /// `AgentEvent::Done` are emitted in that order, and `AgentEvent::Error` is not emitted.
    /// Default (24) is too slow, so override to `max_tool_iterations = 4` for this test.
    #[tokio::test]
    async fn agent_emits_max_tool_iterations_info_when_loop_caps() {
        // Prepare 5 scripts, but process_turn caps at 4 iterations (5th is never called).
        let mut scripts = Vec::new();
        for i in 0..5 {
            scripts.push(vec![
                ProviderEvent::ToolUse {
                    id: format!("call-{i}"),
                    name: "shell".into(),
                    args: serde_json::json!({"cmd": "echo loop"}),
                },
                ProviderEvent::Done,
            ]);
        }
        let history = Agent::build_initial_history(&Persona::builtin_default());
        let mut agent = build_test_agent(scripts, history);
        agent.config.runtime.max_tool_iterations = 4;
        let (in_tx, in_rx) = mpsc::channel::<AgentInput>(8);
        let (ev_tx, mut ev_rx) = mpsc::channel::<AgentEvent>(64);

        let handle = tokio::spawn(async move { agent.run(in_rx, ev_tx).await });
        in_tx
            .send(AgentInput::UserPrompt("loop forever".into()))
            .await
            .unwrap();

        let mut tool_calls = 0usize;
        let mut tool_results = 0usize;
        let mut info_messages: Vec<String> = Vec::new();
        let mut error_count = 0usize;
        let mut saw_done = false;
        while let Some(ev) = ev_rx.recv().await {
            match ev {
                AgentEvent::ToolCall { name, .. } if name == "shell" => tool_calls += 1,
                AgentEvent::ToolResult { name, .. } if name == "shell" => tool_results += 1,
                AgentEvent::Info { message } => info_messages.push(message),
                AgentEvent::Error { .. } => error_count += 1,
                AgentEvent::Done => {
                    saw_done = true;
                    break;
                }
                _ => {}
            }
        }

        assert_eq!(tool_calls, 4, "expected 4 tool calls (max_iterations cap)");
        assert_eq!(tool_results, 4, "expected 4 tool results");
        assert_eq!(
            error_count, 0,
            "no Error events should be emitted on loop cap"
        );
        assert!(
            info_messages
                .iter()
                .any(|m| m == "max tool-use iterations reached"),
            "Info {{ message: 'max tool-use iterations reached' }} not found, got: {info_messages:?}"
        );
        assert!(saw_done, "Done event missing");

        drop(in_tx);
        let _ = handle.await;
    }

    /// FR-04-3 boundary (lower limit): `max_tool_iterations = 0` is clamped to 1 iteration by `.max(1)`.
    /// With 1 iteration, emitting a tool_use means the loop cannot continue, so Info + Done fire immediately.
    #[tokio::test]
    async fn agent_clamps_zero_max_tool_iterations_to_one() {
        let scripts = vec![
            vec![
                ProviderEvent::ToolUse {
                    id: "call-0".into(),
                    name: "shell".into(),
                    args: serde_json::json!({"cmd": "echo clamped"}),
                },
                ProviderEvent::Done,
            ],
            // Second script onward is never called (capped at 1 iteration).
            vec![ProviderEvent::Done],
        ];
        let history = Agent::build_initial_history(&Persona::builtin_default());
        let mut agent = build_test_agent(scripts, history);
        agent.config.runtime.max_tool_iterations = 0; // clamped to 1 via .max(1)
        let (in_tx, in_rx) = mpsc::channel::<AgentInput>(8);
        let (ev_tx, mut ev_rx) = mpsc::channel::<AgentEvent>(32);

        let handle = tokio::spawn(async move { agent.run(in_rx, ev_tx).await });
        in_tx
            .send(AgentInput::UserPrompt("test clamp".into()))
            .await
            .unwrap();

        let mut tool_calls = 0usize;
        let mut info_messages: Vec<String> = Vec::new();
        let mut saw_done = false;
        while let Some(ev) = ev_rx.recv().await {
            match ev {
                AgentEvent::ToolCall { .. } => tool_calls += 1,
                AgentEvent::Info { message } => info_messages.push(message),
                AgentEvent::Done => {
                    saw_done = true;
                    break;
                }
                _ => {}
            }
        }
        assert_eq!(tool_calls, 1, "0 should be clamped to 1 iteration");
        assert!(
            info_messages
                .iter()
                .any(|m| m == "max tool-use iterations reached"),
            "Info should fire because 1 iteration with tool_use cannot continue"
        );
        assert!(saw_done);

        drop(in_tx);
        let _ = handle.await;
    }

    #[tokio::test]
    async fn agent_set_system_prompt_replaces_first_message() {
        let history = Agent::build_initial_history(&Persona::builtin_default());
        let agent = build_test_agent(vec![vec![ProviderEvent::Done]], history);
        let (in_tx, in_rx) = mpsc::channel::<AgentInput>(8);
        let (ev_tx, mut ev_rx) = mpsc::channel::<AgentEvent>(32);

        let handle = tokio::spawn(async move { agent.run(in_rx, ev_tx).await });
        in_tx
            .send(AgentInput::SetSystemPrompt("NEW SYSTEM".into()))
            .await
            .unwrap();

        // Receive one Info notification and finish
        let mut got_info = false;
        if let Some(AgentEvent::Info { message }) = ev_rx.recv().await {
            got_info = message.contains("system prompt");
        }
        assert!(got_info);

        drop(in_tx);
        let _ = handle.await;
    }

    /// `ClearHistory` issued from `/clear` resets history to the persona-derived System message only.
    #[tokio::test]
    async fn clear_history_resets_to_system_only() {
        // Prepare 1 script that returns text on turn 1 (next turn is not consumed)
        let scripts = vec![vec![
            ProviderEvent::Text { delta: "hi".into() },
            ProviderEvent::Done,
        ]];
        let history = Agent::build_initial_history(&Persona::builtin_default());
        let agent = build_test_agent(scripts, history);
        let (in_tx, in_rx) = mpsc::channel::<AgentInput>(8);
        let (ev_tx, mut ev_rx) = mpsc::channel::<AgentEvent>(32);

        // There is no way to inspect Agent's internal history after process_turn,
        // so we indirectly verify via the Info message count difference
        // ("0 message(s) removed" vs "2 message(s) removed").
        let handle = tokio::spawn(async move { agent.run(in_rx, ev_tx).await });

        // Turn 1: consume UserPrompt -> Assistant text -> Done
        in_tx
            .send(AgentInput::UserPrompt("first".into()))
            .await
            .unwrap();
        // Consume text and Done
        let mut saw_done = false;
        while let Some(ev) = ev_rx.recv().await {
            if matches!(ev, AgentEvent::Done) {
                saw_done = true;
                break;
            }
        }
        assert!(saw_done);

        // At this point history = [System, User("first"), Assistant("hi")] (2 non-System out of 3).
        in_tx.send(AgentInput::ClearHistory).await.unwrap();
        let info = ev_rx.recv().await.expect("info expected");
        match info {
            AgentEvent::Info { message } => {
                assert!(
                    message.contains("history cleared"),
                    "unexpected info: {message}"
                );
                assert!(
                    message.contains("2 message(s) removed"),
                    "expected 2 messages removed, got: {message}"
                );
            }
            other => panic!("expected Info, got {:?}", other),
        }

        drop(in_tx);
        let _ = handle.await;
    }

    /// After `ClearHistory`, history contains only the System message and the persona lineage is preserved.
    /// (Ensures the next turn starts from the same state as the very first turn.)
    #[tokio::test]
    async fn clear_history_then_next_prompt_starts_fresh() {
        let scripts = vec![
            vec![
                ProviderEvent::Text {
                    delta: "first-reply".into(),
                },
                ProviderEvent::Done,
            ],
            vec![
                ProviderEvent::Text {
                    delta: "second-reply".into(),
                },
                ProviderEvent::Done,
            ],
        ];
        let history = Agent::build_initial_history(&Persona::builtin_default());
        let agent = build_test_agent(scripts, history);
        let (in_tx, in_rx) = mpsc::channel::<AgentInput>(8);
        let (ev_tx, mut ev_rx) = mpsc::channel::<AgentEvent>(32);

        let handle = tokio::spawn(async move { agent.run(in_rx, ev_tx).await });

        // Turn 1 -> Done
        in_tx
            .send(AgentInput::UserPrompt("first".into()))
            .await
            .unwrap();
        while let Some(ev) = ev_rx.recv().await {
            if matches!(ev, AgentEvent::Done) {
                break;
            }
        }

        // Clear history -> consume and discard Info
        in_tx.send(AgentInput::ClearHistory).await.unwrap();
        let _ = ev_rx.recv().await;

        // Turn 2 also fires text -> Done as prepared in the script
        in_tx
            .send(AgentInput::UserPrompt("second".into()))
            .await
            .unwrap();
        let mut text = String::new();
        let mut saw_done = false;
        while let Some(ev) = ev_rx.recv().await {
            match ev {
                AgentEvent::Text { delta } => text.push_str(&delta),
                AgentEvent::Done => {
                    saw_done = true;
                    break;
                }
                _ => {}
            }
        }
        assert_eq!(text, "second-reply");
        assert!(saw_done);

        drop(in_tx);
        let _ = handle.await;
    }

    /// FR-04-1: When approval `true` is returned via approval_tx, the tool is executed and its result is reflected.
    #[tokio::test]
    async fn approval_channel_grants_tool_execution() {
        let scripts = vec![
            vec![
                ProviderEvent::ToolUse {
                    id: "call-1".into(),
                    name: "shell".into(),
                    args: serde_json::json!({"cmd": "echo approved"}),
                },
                ProviderEvent::Done,
            ],
            vec![
                ProviderEvent::Text {
                    delta: "after-tool".into(),
                },
                ProviderEvent::Done,
            ],
        ];
        let history = Agent::build_initial_history(&Persona::builtin_default());
        let mut agent = build_test_agent(scripts, history);
        // Disable auto_approve to require the approval channel
        agent.auto_approve = Arc::new(AtomicBool::new(false));
        let (approval_tx, mut approval_rx) = mpsc::channel::<ApprovalRequest>(4);
        agent.approval_tx = Some(approval_tx);

        let (in_tx, in_rx) = mpsc::channel::<AgentInput>(8);
        let (ev_tx, mut ev_rx) = mpsc::channel::<AgentEvent>(32);

        // Stub handler that returns y (true) for approval requests
        tokio::spawn(async move {
            while let Some(req) = approval_rx.recv().await {
                let _ = req.response.send(true);
            }
        });

        let handle = tokio::spawn(async move { agent.run(in_rx, ev_tx).await });
        in_tx
            .send(AgentInput::UserPrompt("run echo".into()))
            .await
            .unwrap();

        let mut tool_ok = false;
        let mut output_contains_marker = false;
        let mut text = String::new();
        let mut saw_done = false;
        while let Some(ev) = ev_rx.recv().await {
            match ev {
                AgentEvent::ToolResult { name, ok, output } if name == "shell" => {
                    tool_ok = ok;
                    output_contains_marker = output.contains("approved");
                }
                AgentEvent::Text { delta } => text.push_str(&delta),
                AgentEvent::Done => {
                    saw_done = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(tool_ok, "tool should be approved and succeed");
        assert!(
            output_contains_marker,
            "expected echo output to include marker"
        );
        assert_eq!(text, "after-tool");
        assert!(saw_done);

        drop(in_tx);
        let _ = handle.await;
    }

    /// FR-04-1: When approval `false` is returned, the tool is not executed and "user denied tool execution" is returned.
    #[tokio::test]
    async fn approval_channel_denial_skips_tool() {
        let scripts = vec![
            vec![
                ProviderEvent::ToolUse {
                    id: "call-deny".into(),
                    name: "shell".into(),
                    args: serde_json::json!({"cmd": "echo denied"}),
                },
                ProviderEvent::Done,
            ],
            vec![
                ProviderEvent::Text { delta: "ok".into() },
                ProviderEvent::Done,
            ],
        ];
        let history = Agent::build_initial_history(&Persona::builtin_default());
        let mut agent = build_test_agent(scripts, history);
        agent.auto_approve = Arc::new(AtomicBool::new(false));
        let (approval_tx, mut approval_rx) = mpsc::channel::<ApprovalRequest>(4);
        agent.approval_tx = Some(approval_tx);

        let (in_tx, in_rx) = mpsc::channel::<AgentInput>(8);
        let (ev_tx, mut ev_rx) = mpsc::channel::<AgentEvent>(32);

        tokio::spawn(async move {
            while let Some(req) = approval_rx.recv().await {
                let _ = req.response.send(false);
            }
        });

        let handle = tokio::spawn(async move { agent.run(in_rx, ev_tx).await });
        in_tx
            .send(AgentInput::UserPrompt("denied".into()))
            .await
            .unwrap();

        let mut tool_denied = false;
        let mut saw_done = false;
        while let Some(ev) = ev_rx.recv().await {
            match ev {
                AgentEvent::ToolResult { name, ok, output } if name == "shell" => {
                    tool_denied = !ok && output.contains("denied tool execution");
                }
                AgentEvent::Done => {
                    saw_done = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(tool_denied, "tool result should reflect user denial");
        assert!(saw_done);

        drop(in_tx);
        let _ = handle.await;
    }

    /// FR-04-2: When `auto_approve = true`, execution proceeds immediately without going through approval_tx.
    #[tokio::test]
    async fn auto_approve_atomic_skips_approval_channel() {
        let scripts = vec![
            vec![
                ProviderEvent::ToolUse {
                    id: "call-auto".into(),
                    name: "shell".into(),
                    args: serde_json::json!({"cmd": "echo auto"}),
                },
                ProviderEvent::Done,
            ],
            vec![ProviderEvent::Done],
        ];
        let history = Agent::build_initial_history(&Persona::builtin_default());
        let mut agent = build_test_agent(scripts, history);
        // auto_approve defaults to true in build_test_agent, but make it explicit
        agent.auto_approve.store(true, Ordering::SeqCst);
        let (approval_tx, mut approval_rx) = mpsc::channel::<ApprovalRequest>(4);
        agent.approval_tx = Some(approval_tx);

        let (in_tx, in_rx) = mpsc::channel::<AgentInput>(8);
        let (ev_tx, mut ev_rx) = mpsc::channel::<AgentEvent>(32);

        let handle = tokio::spawn(async move { agent.run(in_rx, ev_tx).await });
        in_tx
            .send(AgentInput::UserPrompt("auto".into()))
            .await
            .unwrap();

        // Verify tool execution did not go through approval
        let mut tool_ok = false;
        let mut saw_done = false;
        while let Some(ev) = ev_rx.recv().await {
            match ev {
                AgentEvent::ToolResult { name, ok, .. } if name == "shell" => {
                    tool_ok = ok;
                }
                AgentEvent::Done => {
                    saw_done = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(tool_ok);
        assert!(saw_done);
        // Nothing arrived on approval_rx
        assert!(
            approval_rx.try_recv().is_err(),
            "approval channel should not be invoked when auto_approve=true"
        );

        drop(in_tx);
        let _ = handle.await;
    }
}
