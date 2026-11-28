use super::*;

/// Headless run: register and serve peer prompts over IPC with no interactive
/// REPL. This is the target a detached `spawn` launches (`Command::Serve`); it
/// may also be run directly for a foreground headless agent.
///
/// It builds the same provider / IPC server / registry / agent / display stack
/// as [`run`], but omits the stdin input loop entirely — a detached process has
/// `stdin` pointed at `/dev/null`, and [`run`] treats stdin EOF as shutdown,
/// which would kill the agent immediately. With no console to answer tool
/// approval, serve mode forces `auto_approve = true` and builds the agent with
/// `approval_tx: None`, so it never blocks on a y/N prompt. Shutdown converges
/// on `SIGINT` / `SIGTERM` (via the signal task) or an `IpcMessage::Shutdown`.
pub async fn run_headless(mut config: Config, source: ConfigSource, args: RunArgs) -> Result<()> {
    config.apply_overrides(args.provider.as_deref(), args.model.as_deref());

    let id = AgentId::new();
    let name = args.name.clone();
    let group = config.resolve_group(args.group.as_deref());
    let agents_dir = config.agents_dir()?;
    // A headless agent reads the directory it was started in exactly as an
    // interactive one does — a detached child inherits that directory.
    let project_persona_path = config::local_persona_candidate();
    let resolution: PersonaResolution = persona::resolve(
        args.persona.as_deref(),
        &config.runtime.persona_file,
        &agents_dir,
        name.as_deref(),
        project_persona_path.as_deref(),
    )?;

    if resolution.builtin_used {
        tracing::info!("using builtin default persona");
    }
    if resolution.project_used {
        if let Some(p) = &resolution.persona.source_path {
            tracing::info!("using project persona {}", p.display());
        }
    }

    config.apply_persona_overrides(
        resolution.persona.frontmatter.model.as_deref(),
        resolution.persona.frontmatter.temperature,
    );

    let provider = ai::build(&mut config, &source)?;

    let registry_dir = config.registry_dir()?;
    let socket_path = registry_dir.join(format!("{}.sock", id.as_str()));

    let mut ipc_server = IpcServer::bind(socket_path.clone()).await?;
    let mut ipc_rx = ipc_server
        .take_rx()
        .expect("IpcServer rx should be available immediately after bind");
    // Shared with the accept loop's front-door gate; also the Agent's skip
    // condition for its queued prompts (only a genuinely stopping session
    // refuses its queue).
    let stopping = ipc_server.stopping();

    // The chain of agents that created this one, set by the launcher on this
    // process' environment. Empty means a human started it: depth 0, and this
    // agent is the root of its own tree.
    let ancestors = crate::ipc::registry::ancestors_from_env();

    let entry = RegistryEntry {
        id: id.clone(),
        name: name.clone(),
        group: group.clone(),
        pid: std::process::id(),
        started_at: Utc::now(),
        provider: config.provider.kind.clone(),
        model: provider.model().to_string(),
        socket: socket_path.clone(),
        persona: Some(resolution.persona.summary()),
        ancestors: ancestors.clone(),
        turn: None,
    };
    let registry_handle = RegistryHandle::register(&registry_dir, &entry).await?;
    let registry_handle = Arc::new(registry_handle);

    let log = ConversationLog::open(&config.log_dir()?, &id).await?;

    let allowed = resolution.persona.frontmatter.allowed_tools.clone();
    let denied = resolution.persona.frontmatter.denied_tools.clone();
    let mut tools = ToolRegistry::build(&config, allowed.as_deref(), denied.as_deref());
    let mcp_tools = crate::mcp::connect_all(&config.mcp).await;
    tools.attach(mcp_tools, allowed.as_deref(), denied.as_deref());

    let history = Agent::build_initial_history(&resolution.persona);
    // A headless agent has no console to answer tool-approval prompts, so it
    // must auto-approve and never request confirmation (approval_tx: None).
    // `[permissions] deny` is therefore the *only* thing that can stop a tool
    // call here, which is precisely why a deny outranks auto-approval.
    let auto_approve = Arc::new(AtomicBool::new(true));
    let permissions = Arc::new(crate::permissions::Ruleset::from_config(
        &config.permissions,
    ));
    for warning in permissions.warnings() {
        tracing::warn!(target: "permissions", "{warning}");
    }

    // Headless: no console, so no user input can raise this token — only the
    // shutdown path, so the in-flight turn closes cleanly (`finish_cancelled`
    // instead of being dropped mid-stream).
    let cancel = Arc::new(CancelToken::default());

    let agent = Agent {
        id: id.clone(),
        group: group.clone(),
        persona: resolution.persona,
        provider,
        tools,
        config: config.clone(),
        config_source: source.clone(),
        registry_dir: registry_dir.clone(),
        ancestors: ancestors.clone(),
        log: Some(log),
        auto_approve,
        permissions,
        // Headless: no console, so nothing user input can raise this token.
        // The shutdown path raises it so the in-flight turn closes cleanly
        // (`finish_cancelled`, not a silent drop).
        cancel: cancel.clone(),
        stopping: stopping.clone(),
        // A headless agent launched with the `--exit-when-idle` flag ends
        // its loop when its work is done (the input channel is empty after
        // the last processed input); the serve loop's wait below fires the
        // normal teardown on the agent task's end.
        exit_when_idle: args.exit_when_idle,
        approval_tx: None,
        history,
    };

    let (input_tx, input_rx) = mpsc::channel::<AgentInput>(32);
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(64);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Serve mode never colours: `Theme::plain()` makes this the same bytes as
    // before, and keeps every headless consumer's parsing intact.
    let theme = Theme::plain();
    println!(
        "{}",
        theme.out(
            Role::Banner,
            &format!(
                "agent-cli serving headless: id={} name={} provider={} model={}",
                id.as_str(),
                name.as_deref().unwrap_or("-"),
                config.provider.kind,
                agent.provider.model(),
            )
        )
    );
    let _ = std::io::stdout().flush();

    let mut agent_handle = tokio::spawn(async move { agent.run(input_rx, event_tx).await });

    // Raised when the shutdown begins; the IPC server answers new prompts
    // itself instead of acking them into a queue that would be destroyed.
    let stopping = ipc_server.stopping();

    // SIGINT / SIGTERM handler
    let signal_task = {
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            wait_for_termination_signal().await;
            let _ = shutdown_tx.send(true);
        })
    };

    // Forward IPC prompts to the agent; an IpcMessage::Shutdown triggers shutdown.
    let input_tx_for_ipc = input_tx.clone();
    let ipc_task = {
        let mut shutdown_rx = shutdown_rx.clone();
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    res = ipc_rx.recv() => {
                        match res {
                            Some(IpcMessage::Prompt { from, from_name, text, reply_to }) => {
                                if input_tx_for_ipc
                                    .send(AgentInput::PeerPrompt { from, from_name, text, reply_to })
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            // A peer reporting a result: added to the
                            // conversation, no turn run, no answer produced.
                            Some(IpcMessage::Context { from, from_name, text }) => {
                                let text = crate::agent::peer_context_text(
                                    &from,
                                    from_name.as_deref(),
                                    &text,
                                );
                                if input_tx_for_ipc
                                    .send(AgentInput::Context {
                                        text,
                                        source: crate::agent::ContextSource::Peer { from, from_name },
                                    })
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Some(IpcMessage::Shutdown) => {
                                let _ = shutdown_tx.send(true);
                                break;
                            }
                            Some(_) => {}
                            None => break,
                        }
                    }
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        })
    };

    let show_thinking = config.ui.show_thinking_mode();
    let show_tool_results = config.ui.show_tool_results;
    let display_task = tokio::spawn(async move {
        let mut display_state = DisplayState::new(stdin_is_tty());
        display_state.show_tool_results = show_tool_results;
        while let Some(ev) = event_rx.recv().await {
            display_event(ev, show_thinking, &mut display_state);
        }
    });

    // Block until a shutdown notification arrives (signal or IPC Shutdown) —
    // or, with `--exit-when-idle`, until the agent's own loop ends: the
    // agent breaks out of its input loop when the work is done and the input
    // channel has gone quiet, and that quiet end raises the same shutdown
    // watch here, so the teardown below is exactly the normal one.
    {
        let mut shutdown_rx = shutdown_rx.clone();
        loop {
            if *shutdown_rx.borrow() {
                break;
            }
            tokio::select! {
                changed = shutdown_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                // The agent's loop ended on its own (quiet exit). Distinguish
                // it from an aborted task by waiting for the handle's result.
                res = &mut agent_handle, if args.exit_when_idle => {
                    if let Err(e) = res {
                        tracing::warn!(error = %e, "agent task ended with an error (quiet exit)");
                    }
                    let _ = shutdown_tx.send(true);
                    break;
                }
            }
        }
    }

    // Shutdown begins: reject further prompts (the server answers them with
    // the stopping error), finish the current turn cleanly, skip queued
    // prompts with a truthful reply, and only then tear down.
    stopping.store(true, std::sync::atomic::Ordering::SeqCst);

    ipc_task.abort();
    let _ = ipc_task.await;
    signal_task.abort();
    let _ = signal_task.await;

    drop(input_tx);
    // Cancel the current turn so it closes with `finish_cancelled` instead of
    // being dropped mid-stream, and let the agent loop skip the remaining
    // queue with truthful replies before ending on the closed channel.
    cancel.cancel();

    match tokio::time::timeout(SHUTDOWN_DRAIN_TIMEOUT, &mut agent_handle).await {
        // finished within the drain window
        Ok(_) => {}
        // did not finish in time: detach, then abort the task itself
        Err(_) => agent_handle.abort(),
    }
    let _ = agent_handle.await;
    let _ = display_task.await;

    drop(ipc_server);
    registry_handle.cleanup();
    IpcServer::cleanup(&socket_path);
    Ok(())
}

/// The one-shot mode: one prompt in, the answer out, the session ends. `run -p
/// "<prompt>"` (the `Command::OneShot` arm) lands here. The assembly mirrors
/// `run_headless`'s — persona, provider, tools, MCP, the conversation log and
/// the registry entry are the same one identity the REPL would build — but
/// nothing listens: no IPC server is bound, no REPL input loop runs, and the
/// process exits as soon as the one turn completes. The answer streams to
/// stdout as the model produces it (the `Text` deltas, plain), and nothing else
/// prints (the one-shot's output stays pipe-friendly: the answer alone, errors
/// to stderr).
pub async fn run_oneshot(
    mut config: Config,
    source: ConfigSource,
    args: RunArgs,
    prompt: String,
) -> Result<()> {
    config.apply_overrides(args.provider.as_deref(), args.model.as_deref());

    let id = AgentId::new();
    let name = args.name.clone();
    let agents_dir = config.agents_dir()?;
    let project_persona_path = config::local_persona_candidate();
    let resolution: PersonaResolution = persona::resolve(
        args.persona.as_deref(),
        &config.runtime.persona_file,
        &agents_dir,
        name.as_deref(),
        project_persona_path.as_deref(),
    )?;

    config.apply_persona_overrides(
        resolution.persona.frontmatter.model.as_deref(),
        resolution.persona.frontmatter.temperature,
    );

    let provider = ai::build(&mut config, &source)?;

    // The one-shot is a session: it registers like any other identity (a peer
    // can `send`/`ask` it only while it lives, which is the duration of the
    // one turn — the registry entry is removed at the end), shares the same
    // ancestors rules, and keeps the conversation log (the one turn's record).
    let registry_dir = config.registry_dir()?;
    let entry = RegistryEntry {
        id: id.clone(),
        name: name.clone(),
        group: config.resolve_group(args.group.as_deref()),
        pid: std::process::id(),
        started_at: Utc::now(),
        provider: config.provider.kind.clone(),
        model: provider.model().to_string(),
        // No socket is bound: nothing can prompt the process over IPC, and the
        // empty socket field keeps the registry honest about that.
        socket: PathBuf::new(),
        persona: Some(resolution.persona.summary()),
        ancestors: crate::ipc::registry::ancestors_from_env(),
        turn: None,
    };
    let registry_handle = RegistryHandle::register(&registry_dir, &entry).await?;
    let registry_handle = Arc::new(registry_handle);

    let log = ConversationLog::open(&config.log_dir()?, &id).await?;

    let allowed = resolution.persona.frontmatter.allowed_tools.clone();
    let denied = resolution.persona.frontmatter.denied_tools.clone();
    let mut tools = ToolRegistry::build(&config, allowed.as_deref(), denied.as_deref());
    let mcp_tools = crate::mcp::connect_all(&config.mcp).await;
    tools.attach(mcp_tools, allowed.as_deref(), denied.as_deref());

    let history = Agent::build_initial_history(&resolution.persona);
    // No console answers an approval mid-shot: the headless rule applies.
    let auto_approve = Arc::new(AtomicBool::new(true));
    let permissions = Arc::new(crate::permissions::Ruleset::from_config(
        &config.permissions,
    ));
    for warning in permissions.warnings() {
        tracing::warn!(target: "permissions", "{warning}");
    }
    let cancel = Arc::new(CancelToken::default());

    let agent = Agent {
        id: id.clone(),
        group: entry.group.clone(),
        persona: resolution.persona,
        provider,
        tools,
        config: config.clone(),
        config_source: source.clone(),
        registry_dir: registry_dir.clone(),
        ancestors: entry.ancestors.clone(),
        log: Some(log),
        auto_approve,
        permissions,
        cancel: cancel.clone(),
        stopping: Arc::new(AtomicBool::new(false)),
        exit_when_idle: true,
        approval_tx: None,
        history,
    };

    let (input_tx, input_rx) = mpsc::channel::<AgentInput>(8);
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(64);

    let mut agent_handle = tokio::spawn(async move { agent.run(input_rx, event_tx).await });

    // The one prompt, then the input channel closes: the agent processes it
    // and its loop ends (exit_when_idle), the task finishes, the answer has
    // streamed through the events below.
    if input_tx.send(AgentInput::UserPrompt(prompt)).await.is_err() {
        return Err(crate::error::AppError::Other(
            "the agent task ended before the prompt was accepted".into(),
        ));
    }
    drop(input_tx);

    let mut turn_text = String::new();
    let mut turn_errored: Option<String> = None;
    while let Some(ev) = event_rx.recv().await {
        match ev {
            AgentEvent::Text { delta } => {
                print!("{delta}");
                let _ = std::io::Write::flush(&mut std::io::stdout());
                turn_text.push_str(&delta);
            }
            AgentEvent::Error { message } => {
                turn_errored = Some(message);
            }
            AgentEvent::Done => break,
            _ => {}
        }
    }
    // Drain the agent task's own end.
    let _ = tokio::time::timeout(SHUTDOWN_DRAIN_TIMEOUT, &mut agent_handle).await;

    if let Some(message) = turn_errored {
        eprintln!("error: {message}");
        registry_handle.cleanup();
        std::process::exit(1);
    }
    if !turn_text.ends_with('\n') {
        println!();
    }
    registry_handle.cleanup();
    Ok(())
}
