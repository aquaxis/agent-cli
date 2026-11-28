use super::*;

/// Line-oriented input loop (original behaviour, used for non-interactive / test mode).
pub(super) async fn run_input_loop_line<R>(
    reader: R,
    state: Arc<ReplState>,
    channels: InputLoopChannels,
    interactive: bool,
) where
    R: AsyncRead + Unpin,
{
    let InputLoopChannels {
        input_tx,
        shutdown_tx,
        mut shutdown_rx,
        mut agent_idle_rx,
        approval_rx,
        turn_in_flight_rx: _,
    } = channels;
    let buffered = BufReader::new(reader);
    let mut lines = buffered.lines();
    let mut prompt_state = PromptState::Ready;
    // Once the approval channel closes, set to `None` and switch to `pending()` wait (avoid busy loop)
    let mut approval_rx: Option<mpsc::Receiver<ApprovalRequest>> = Some(approval_rx);
    loop {
        if interactive && prompt_state.is_ready() {
            print_prompt();
        }
        tokio::select! {
            biased;
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
            // Only wait for AI response completion when in Pending state (stdin is paused)
            res = agent_idle_rx.recv(), if prompt_state.is_pending() => {
                match res {
                    Some(()) => {
                        prompt_state = PromptState::Ready;
                        // Prompt will be redrawn at the top of the next loop iteration
                    }
                    None => break, // display_task terminated -> no more events coming
                }
            }
            // Approval request arrived (only when not AwaitingApproval).
            req = async {
                match approval_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending::<Option<ApprovalRequest>>().await,
                }
            }, if !prompt_state.is_awaiting_approval() && approval_rx.is_some() => {
                match req {
                    Some(req) => {
                        if interactive {
                            raw_println(false, "");
                            println!(
                                "{}",
                                state.theme.out(
                                    Role::Confirm,
                                    &format!("[tool approval] {} {}", req.tool_name, req.args),
                                )
                            );
                            print!("{}", state.theme.out(Role::Confirm, "approve? [y/N]: "));
                            let _ = std::io::stdout().flush();
                        }
                        prompt_state = PromptState::AwaitingApproval(req.response);
                    }
                    None => {
                        // Approval channel closed -> don't wait anymore
                        approval_rx = None;
                    }
                }
            }
            // Read from stdin when Ready or AwaitingApproval
            next = lines.next_line(), if prompt_state.is_ready() || prompt_state.is_awaiting_approval() => {
                match next {
                    Ok(Some(line)) => {
                        let trimmed = line.trim_end_matches('\r').trim().to_string();

                        // If AwaitingApproval, interpret input as y/N and send response to agent
                        if prompt_state.is_awaiting_approval() {
                            let approved = is_approval_affirmative(&trimmed);
                            // Transition to Pending while extracting and sending the oneshot
                            let prev = std::mem::replace(&mut prompt_state, PromptState::Pending);
                            if let PromptState::AwaitingApproval(resp_tx) = prev {
                                let _ = resp_tx.send(approved);
                            }
                            // Wait for tool execution -> follow-up -> Done. Drain stale idle notifications.
                            while agent_idle_rx.try_recv().is_ok() {}
                            continue;
                        }

                        // A `!` line runs in the shell here too. There are no
                        // keys to poll on this path, so the command is simply
                        // awaited to completion — the timeout is what bounds it.
                        let classified = crate::shell::classify_line(&trimmed, state.shell.enabled);
                        if let crate::shell::Line::Shell(cmd) = classified {
                            push_history(&state, &trimmed).await;
                            run_shell_to_completion(cmd, &state, &input_tx, false).await;
                            continue;
                        }
                        if matches!(classified, crate::shell::Line::ShellUsage) {
                            raw_eprintln(false, "usage: !<command>  (runs it in the shell)");
                            continue;
                        }

                        if let Some(rest) = trimmed.strip_prefix('/') {
                            match handle_repl_command(rest, &input_tx, &state, false).await {
                                CommandResult::Quit => break,
                                CommandResult::SubmittedPrompt => {
                                    while agent_idle_rx.try_recv().is_ok() {}
                                    prompt_state = PromptState::Pending;
                                }
                                CommandResult::Continue => {}
                                // No select arm here (line mode has no prompt
                                // state to redraw through): the ask outcome is
                                // waited for synchronously, out of band.
                                CommandResult::SpawnedAsk(mut run) => match run.next().await {
                                    Some(Ok(text)) => println!("[peer reply] {text}"),
                                    Some(Err(why)) => eprintln!("[error] {why}"),
                                    None => {}
                                },
                            }
                            continue;
                        }
                        if trimmed.is_empty() {
                            continue;
                        }
                        // Save to history (persistent + in-memory, skip consecutive duplicates)
                        {
                            let last_line = state.history.read().await.last().cloned();
                            append_history(&state.history_path, &trimmed, last_line.as_deref());
                        }
                        {
                            let mut h = state.history.write().await;
                            if h.last().map(|last| last == &trimmed).unwrap_or(false) {
                                // Skip consecutive duplicate
                            } else {
                                h.push(trimmed.clone());
                                let len = h.len();
                                if len > HISTORY_LIMIT {
                                    h.drain(..len - HISTORY_LIMIT);
                                }
                            }
                        }
                        if input_tx.send(AgentInput::UserPrompt(trimmed)).await.is_err() {
                            break;
                        }
                        // Discard stale idle notifications from past peer prompts, then enter Pending
                        while agent_idle_rx.try_recv().is_ok() {}
                        prompt_state = PromptState::Pending;
                    }
                    Ok(None) => break, // EOF (Ctrl+D)
                    Err(_) => break,
                }
            }
        }
    }
    // When exiting AwaitingApproval via the shutdown route, send false (deny) to the
    // agent's oneshot::Receiver to avoid leaving it dangling.
    if let PromptState::AwaitingApproval(resp_tx) =
        std::mem::replace(&mut prompt_state, PromptState::Ready)
    {
        let _ = resp_tx.send(false);
    }
    let _ = shutdown_tx.send(true);
}
