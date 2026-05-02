use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::stream::StreamExt;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use crate::ai::{Message, Provider, ProviderEvent};
use crate::config::Config;
use crate::error::Result;
use crate::id::AgentId;
use crate::log::{ConversationLog, LogEvent};
use crate::persona::Persona;
use crate::tools::{ToolCtx, ToolRegistry};

/// REPL 入力ループへ送る、ツール実行承認のリクエスト（FR-04-1／設計書 4.3A）。
///
/// `agent` タスクは `response` の `oneshot::Sender` 経由でユーザーの y/N 応答を待つ。
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
    /// `/auto` REPL コマンドで実行時切替されるため `Arc<AtomicBool>` で共有（FR-04-2）。
    pub auto_approve: Arc<AtomicBool>,
    /// 入力ループへ承認リクエストを流すチャネル。`None` の場合は承認できないので拒否扱い（FR-04-1）。
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
        let max_iterations = 8;
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
                    // FR-03-2：エラー終了でも 1 ターン分の `Done` を必ず発行し、
                    // REPL 入力ループが Pending のまま固まらないようにする。
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

            if !assistant_text.is_empty() {
                if let Some(l) = log {
                    l.write(LogEvent::Assistant {
                        text: &assistant_text,
                    })
                    .await
                    .ok();
                }
                self.history.push(Message::Assistant {
                    content: assistant_text.clone(),
                });
            }

            if had_error || pending_tools.is_empty() {
                let _ = event_tx.send(AgentEvent::Done).await;
                return Ok(());
            }

            // ツール実行
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
            // ツール結果を踏まえて再度プロバイダ呼び出し
        }
        let _ = event_tx
            .send(AgentEvent::Info {
                message: "max tool-use iterations reached".into(),
            })
            .await;
        let _ = event_tx.send(AgentEvent::Done).await;
        Ok(())
    }
}

/// REPL 入力ループへ承認リクエストを送って応答を待つ（FR-04-1／設計書 4.3A）。
///
/// - `approval_tx` が `None`（テストや承認ハンドラ未接続時）：安全側で `false` を返す。
/// - 送信失敗（入力ループ終了済み）：`false`。
/// - oneshot 受信失敗（入力ループが状態を破棄）：`false`。
///
/// 旧実装の `std::io::stdin().read_line()` 直読みは禁止（tokio 側の stdin reader と
/// stdin を奪い合い、承認 `y` 入力が取り違えられる事象が報告された）。
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
        // 1 ターン目: shell ツール呼び出しを要求
        // 2 ターン目: ツール結果を反映してテキストで終了
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

        // Info 通知を 1 件受け取って終了
        let mut got_info = false;
        if let Some(AgentEvent::Info { message }) = ev_rx.recv().await {
            got_info = message.contains("system prompt");
        }
        assert!(got_info);

        drop(in_tx);
        let _ = handle.await;
    }

    /// FR-04-1：approval_tx 経由で承認 `true` が返れば、ツールが実行されその結果が反映される。
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
        // auto_approve を OFF にして承認チャネルを必須化
        agent.auto_approve = Arc::new(AtomicBool::new(false));
        let (approval_tx, mut approval_rx) = mpsc::channel::<ApprovalRequest>(4);
        agent.approval_tx = Some(approval_tx);

        let (in_tx, in_rx) = mpsc::channel::<AgentInput>(8);
        let (ev_tx, mut ev_rx) = mpsc::channel::<AgentEvent>(32);

        // 承認リクエストに対して y（true）を返す擬似ハンドラ
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

    /// FR-04-1：承認 `false` が返ればツールは実行されず "user denied tool execution" が返る。
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

    /// FR-04-2：`auto_approve = true` のとき、approval_tx を経由せず即実行される。
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
        // auto_approve は build_test_agent の既定で true なので明示しないが、念のため
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

        // ツール実行が approval を経由していないことを確認
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
        // approval_rx に何も来ていない
        assert!(
            approval_rx.try_recv().is_err(),
            "approval channel should not be invoked when auto_approve=true"
        );

        drop(in_tx);
        let _ = handle.await;
    }
}
