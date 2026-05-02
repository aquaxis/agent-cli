use futures::stream::StreamExt;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::ai::{Message, Provider, ProviderEvent};
use crate::config::Config;
use crate::error::Result;
use crate::id::AgentId;
use crate::log::{ConversationLog, LogEvent};
use crate::persona::Persona;
use crate::tools::{ToolCtx, ToolRegistry};

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
    pub auto_approve: bool,
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

                if !self.auto_approve {
                    let approved = approval_prompt(&name, &args).await;
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

async fn approval_prompt(name: &str, args: &Value) -> bool {
    use std::io::{stdin, stdout, Write};
    let _ = writeln!(stdout(), "\n[tool approval] {name} {}", args);
    let _ = write!(stdout(), "approve? [y/N]: ");
    let _ = stdout().flush();
    let mut line = String::new();
    let _ = stdin().read_line(&mut line);
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
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
            auto_approve: true,
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
}
