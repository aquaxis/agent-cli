use super::*;

/// Outcome of `handle_repl_command` (custom-command support).
///
/// - `Continue`: command completed (or unknown); redraw the prompt (existing behavior).
/// - `SubmittedPrompt`: a custom command sent a `UserPrompt` to the agent; the
///   caller must drain stale idle notifications and enter `PromptState::Pending`
///   (same as a normal user prompt).
/// - `Quit`: terminate the REPL (`/quit`, `/exit`).
/// - `SpawnedAsk`: a `/send --ask` was handed to a background runner; the
///   caller stores it and lets a dedicated select arm print the outcome, so
///   the loop keeps taking keys while the reply is unpaid on its way.
#[derive(Debug)]
pub(super) enum CommandResult {
    Continue,
    SubmittedPrompt,
    Quit,
    SpawnedAsk(crate::tools::send_to::PeerAsk),
}

/// `/auto [on|off|status]` handler.
pub(super) fn handle_auto_command(arg: &str, state: &Arc<ReplState>, raw_mode: bool) {
    let arg = arg.trim().to_ascii_lowercase();
    match arg.as_str() {
        "on" | "true" | "1" => {
            state.auto_approve.store(true, Ordering::SeqCst);
            let msg = "[auto] tool approval: on (skipping y/N prompts)";
            raw_println(raw_mode, &state.theme.out(Role::Info, msg));
        }
        "off" | "false" | "0" => {
            state.auto_approve.store(false, Ordering::SeqCst);
            let msg = "[auto] tool approval: off (will ask y/N for each tool call)";
            raw_println(raw_mode, &state.theme.out(Role::Info, msg));
        }
        "" | "status" => {
            let cur = if state.auto_approve.load(Ordering::SeqCst) {
                "on"
            } else {
                "off"
            };
            let msg = format!("[auto] tool approval: {cur}");
            raw_println(raw_mode, &state.theme.out(Role::Info, &msg));
        }
        other => {
            let msg = format!("usage: /auto [on|off|status]  (got: {other})");
            raw_eprintln(raw_mode, &state.theme.err(Role::Info, &msg));
        }
    }
}

/// Print, once at startup, everything the `[permissions]` section said that
/// could not be honoured.
///
/// A rule that silently does nothing is the failure this whole feature exists
/// to end, so these are never swallowed — not even the ones that are merely
/// inert.
pub(super) fn report_permission_warnings(rules: &crate::permissions::Ruleset, theme: &Theme) {
    for warning in rules.warnings() {
        eprintln!(
            "{}",
            theme.err(Role::Info, &format!("[permissions] {warning}"))
        );
        tracing::warn!(target: "permissions", "{warning}");
    }
}

/// `/permissions` — what the rules are, where each came from, and what happens
/// to a call none of them matched.
pub(super) fn handle_permissions_command(state: &Arc<ReplState>, raw_mode: bool) {
    let rules = &state.permissions;
    let theme = &state.theme;
    let mut out = String::new();

    for (i, path) in state.config_source.chain().iter().enumerate() {
        let rank = if i + 1 == state.config_source.chain().len() {
            "config"
        } else {
            "base  "
        };
        let state_note = if path.exists() { "" } else { "  (absent)" };
        out.push_str(&format!("  {rank}: {}{state_note}\n", path.display()));
    }

    if rules.is_empty() {
        out.push_str(
            "  no rules configured — every tool call goes to the y/N prompt\n  \
             (add a [permissions] section to config.toml)\n",
        );
    } else {
        out.push_str(&format!("  deny ({}):\n", rules.deny_len()));
        for rule in rules.deny_rules() {
            out.push_str(&format!("    {}\n", rule.raw));
        }
        out.push_str(&format!("  allow ({}):\n", rules.allow_len()));
        for rule in rules.allow_rules() {
            out.push_str(&format!("    {}\n", rule.raw));
        }
        out.push_str(&format!("  default_mode: {}\n", rules.default_mode()));
        out.push_str("  a deny outranks an allow, the default mode, and /auto on\n");
    }
    for warning in rules.warnings() {
        out.push_str(&format!("  warning: {warning}\n"));
    }

    raw_println(
        raw_mode,
        &theme.out(Role::Info, &format!("[permissions]\n{}", out.trim_end())),
    );
}

/// Built-in REPL slash command names (the `match cmd` arms in
/// `handle_repl_command`). Used for live candidate suggestions.
pub(super) const BUILTIN_COMMANDS: &[&str] = &[
    "quit",
    "exit",
    "help",
    "auto",
    "permissions",
    "clear",
    "reset",
    "history",
    "list",
    "send",
    "spawn",
    "stop",
    "tools",
    "persona",
    "reload-persona",
    "peer",
    "cancel",
    "commands",
    "reload-commands",
];

/// Return command names (without `/`) that start with `prefix`, built-in names
/// first (in declaration order) then custom names (sorted), deduplicated. A
/// custom command that collides with a built-in is dropped (built-ins take
/// precedence). An empty `prefix` matches all.
pub(super) fn slash_candidates(prefix: &str, custom_names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in BUILTIN_COMMANDS {
        if b.starts_with(prefix) {
            out.push((*b).to_string());
        }
    }
    let builtin_set: std::collections::HashSet<&str> = BUILTIN_COMMANDS.iter().copied().collect();
    let mut customs: Vec<String> = custom_names
        .iter()
        .filter(|c| c.starts_with(prefix) && !builtin_set.contains(c.as_str()))
        .cloned()
        .collect();
    customs.sort();
    out.extend(customs);
    out
}

/// Build the single-line suggestion string shown below the prompt when the
/// user is typing a `/<name>` command (no space yet). Returns `None` when
/// there is nothing useful to suggest (no match, or the typed text is already
/// the sole exact match).
pub(super) fn command_suggestion(prefix: &str, custom_names: &[String]) -> Option<String> {
    let cands = slash_candidates(prefix, custom_names);
    if cands.is_empty() {
        return None;
    }
    if cands.len() == 1 && cands[0] == prefix {
        return None;
    }
    let display = cands
        .iter()
        .map(|c| format!("/{}", c))
        .collect::<Vec<_>>()
        .join("  ");
    Some(display)
}

/// Compute the live command suggestion for the current input line. Returns
/// `None` unless the line starts with `/` and has no space (i.e. the user is
/// still typing the command name).
pub(super) async fn current_suggestion(state: &Arc<ReplState>, line: &str) -> Option<String> {
    let after = line.strip_prefix('/')?;
    if after.contains(' ') {
        return None;
    }
    let customs: Vec<String> = state
        .commands
        .read()
        .await
        .iter()
        .map(|c| c.name.clone())
        .collect();
    command_suggestion(after, &customs)
}

/// Longest prefix shared by every candidate. Empty when the list is empty or
/// the candidates diverge at the first character.
pub(super) fn longest_common_prefix(cands: &[String]) -> String {
    let Some(first) = cands.first() else {
        return String::new();
    };
    let mut end = first.len();
    for c in &cands[1..] {
        let mut common = 0;
        for ((i, a), (_, b)) in first.char_indices().zip(c.char_indices()) {
            if a != b {
                break;
            }
            common = i + a.len_utf8();
        }
        end = end.min(common);
    }
    first[..end].to_string()
}

/// Tab completion for the slash-command being typed. Returns the replacement
/// line, or `None` when there is nothing to complete.
///
/// - Exactly one candidate: complete it and append a space, so an argument can
///   be typed straight away (`/sen` → `/send `).
/// - Several candidates: extend as far as they agree (`/re` →
///   `/reload-` for `reload-persona` / `reload-commands`). The candidate list is
///   already on screen above the prompt, so nothing else is printed.
/// - Nothing to add, no match, not a slash command, or an argument already
///   started: `None`, and the keypress is a no-op.
pub(super) fn command_completion(line: &str, custom_names: &[String]) -> Option<String> {
    let prefix = line.strip_prefix('/')?;
    if prefix.contains(' ') {
        return None;
    }
    let cands = slash_candidates(prefix, custom_names);
    match cands.len() {
        0 => None,
        1 => Some(format!("/{} ", cands[0])),
        _ => {
            let lcp = longest_common_prefix(&cands);
            (lcp.len() > prefix.len()).then(|| format!("/{lcp}"))
        }
    }
}

/// Truncate `s` to fit within `max_chars` display columns (command names are
/// ASCII, so char-based truncation is accurate), appending `…` when truncated.
pub(super) fn truncate_suggestion(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    t.push('…');
    t
}

pub(super) async fn handle_repl_command(
    rest: &str,
    input_tx: &mpsc::Sender<AgentInput>,
    state: &Arc<ReplState>,
    raw_mode: bool,
) -> CommandResult {
    let mut parts = rest.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("").trim();
    let arg = parts.next().unwrap_or("").trim();
    match cmd {
        "quit" | "exit" => return CommandResult::Quit,
        "help" => {
            raw_println(raw_mode, "Commands:");
            raw_println(raw_mode, "  /list                       List currently running peers (id / name / provider / model / role).");
            raw_println(raw_mode, "  /send [--report|--ask] <peer> <text>");
            raw_println(raw_mode, "                              Send to a peer (id or name): a one-shot prompt (default),");
            raw_println(raw_mode, "                              a report added to its conversation with no turn, or an ask");
            raw_println(
                raw_mode,
                "                              that waits for the answer (up to 120s).",
            );
            raw_println(
                raw_mode,
                "  /tools                      List tools enabled for this agent.",
            );
            raw_println(raw_mode, "  /persona                    Show this agent's persona (role / skills / source path).");
            raw_println(raw_mode, "  /reload-persona             Re-resolve and reload the persona; system prompt is replaced, history kept.");
            raw_println(raw_mode, "  /spawn [name] [provider]    Create a detached agent-cli peer that outlives this session.");
            raw_println(raw_mode, "  /stop <peer>                Stop a running peer (id or name); detached agents too.");
            raw_println(
                raw_mode,
                "  /peer <id_or_name>          Show a peer's persona summary.",
            );
            raw_println(raw_mode, "  /history [n]                Show last n (default 20) user inputs from this session.");
            raw_println(raw_mode, "  /clear, /reset              Clear conversation history (persona / system prompt are kept).");
            raw_println(raw_mode, "  /cancel                     Request cancel of the in-flight AI response or tool call.");
            raw_println(raw_mode, "  /auto [on|off|status]       Toggle tool-approval skip. No arg / 'status' shows current value.");
            raw_println(raw_mode, "  /permissions                Show the [permissions] rules in force and where they came from.");
            raw_println(raw_mode, "  /commands                   List custom slash commands from .agent-cli/commands.");
            raw_println(
                raw_mode,
                "  /reload-commands            Re-scan the custom commands directory.",
            );
            raw_println(raw_mode, "  /help                       Show this help.");
            raw_println(raw_mode, "  /quit, /exit                Terminate (full aliases). Ctrl+D, Ctrl+C, SIGTERM also exit cleanly.");
            raw_println(raw_mode, "");
            raw_println(raw_mode, "At the prompt:");
            raw_println(raw_mode, "  !<command>                  Run a shell command right away (no approval); its output");
            raw_println(raw_mode, "                              is shown and handed to the model as context. Esc stops it.");
            raw_println(raw_mode, "  Tab                         Complete the slash command being typed, from the list");
            raw_println(
                raw_mode,
                "                              shown on the row above the prompt.",
            );
            raw_println(raw_mode, "  Mouse wheel                 Scroll back through this session's log with the prompt");
            raw_println(raw_mode, "                              line pinned; the screen's bottom line reads inverted");
            raw_println(raw_mode, "                              \"Message below\" while you are scrolled — no spinner, no");
            raw_println(raw_mode, "                              elapsed time, at rest or mid-turn — and Esc or scrolling");
            raw_println(raw_mode, "                              to the bottom always returns to the live view with the");
            raw_println(
                raw_mode,
                "                              prompt re-displayed.",
            );
            raw_println(raw_mode, "  Mouse drag                  Select a range of the log; releasing copies it to the");
            raw_println(raw_mode, "                              clipboard as plain text. Shift-drag selects with the");
            raw_println(
                raw_mode,
                "                              terminal instead ([ui] mouse_select, copy_command).",
            );
            raw_println(raw_mode, "  Esc                         Stop a running turn or shell command; at the tool-approval");
            raw_println(raw_mode, "                              prompt, deny the pending tool. After the stop, the line");
            raw_println(
                raw_mode,
                "                              reads [info] cancelled by user and the prompt is",
            );
            raw_println(raw_mode, "                              re-displayed.");
            raw_println(
                raw_mode,
                "  Subagents                   Each running subagent shows its name and operating",
            );
            raw_println(raw_mode, "                              time stacked above the clock block; the row goes when");
            raw_println(raw_mode, "                              the subagent ends, and the clock takes the bottom line back");
            raw_println(raw_mode, "                              while the agent runs, the prompt when you can type again.");
            raw_println(raw_mode, "");
            raw_println(raw_mode, "Tool approval can be skipped via:");
            raw_println(
                raw_mode,
                "  - REPL command  : /auto on  (toggleable at runtime)",
            );
            raw_println(
                raw_mode,
                "  - CLI flag      : agent-cli run --auto-approve-tools",
            );
            raw_println(
                raw_mode,
                "  - Config file   : [runtime] auto_approve_tools = true",
            );
            raw_println(
                raw_mode,
                "  None of these override a [permissions] deny rule: a deny is refused",
            );
            raw_println(raw_mode, "  even with /auto on. See /permissions.");
            // Custom commands section.
            let cmds = state.commands.read().await;
            if !cmds.is_empty() {
                raw_println(raw_mode, "");
                raw_println(raw_mode, "Custom commands (.agent-cli/commands):");
                for cc in cmds.iter() {
                    let first_line = cc.content.lines().next().unwrap_or("");
                    raw_println(raw_mode, &format!("  /{}  {}", cc.name, first_line));
                }
            }
        }
        "auto" => handle_auto_command(arg, state, raw_mode),
        "permissions" => handle_permissions_command(state, raw_mode),
        "clear" | "reset" => {
            // Clear conversation history (keep only system prompt).
            if input_tx.send(AgentInput::ClearHistory).await.is_err() {
                raw_eprintln(raw_mode, "[error] failed to send clear request");
            }
        }
        "history" => {
            let n: usize = arg.parse().unwrap_or(20);
            let h = state.persona.read().await;
            let _ = h; // unused
            let hist = state.history.read().await;
            let total = hist.len();
            let start = total.saturating_sub(n);
            for (i, line) in hist.iter().enumerate().skip(start) {
                // Past input is replayed dimmed, so it stays distinct from the
                // line the user is writing now.
                let entry = format!("{:>4}  {}", i + 1, line);
                raw_println(raw_mode, &state.theme.out(Role::HistoryEntry, &entry));
            }
            if total == 0 {
                raw_println(raw_mode, "(empty)");
            }
        }
        "list" => list_peers(&state.registry_dir, raw_mode),
        "send" => {
            if let CommandResult::SpawnedAsk(ask) =
                send_to_peer(arg, &state.registry_dir, raw_mode).await
            {
                return CommandResult::SpawnedAsk(ask);
            }
        }
        "spawn" => spawn_peer(arg, state, raw_mode).await,
        "stop" => stop_peer_cmd(arg, &state.registry_dir, raw_mode).await,
        "tools" => {
            if state.tool_names.is_empty() {
                raw_println(raw_mode, "(no tools enabled)");
            } else {
                raw_println(raw_mode, &format!("tools: {}", state.tool_names.join(", ")));
            }
        }
        "persona" => {
            let p = state.persona.read().await;
            print_persona(&p, raw_mode);
        }
        "reload-persona" => reload_persona(state, input_tx, raw_mode).await,
        "peer" => peer_summary(arg, &state.registry_dir, raw_mode),
        "cancel" => {
            // Same signal as `Esc` during a turn: stops an in-flight turn (e.g.
            // one started by a peer prompt) rather than merely asking for it.
            // Raised only while a turn is actually in flight — raised while
            // the agent is idle, the flag would sit un-consumed (nothing is
            // unwinding, and the unwind-end clear never runs) and every later
            // peer prompt would skip until a manual prompt ran.
            raise_cancel_if_turn_in_flight(&state.cancel, &state.turn_in_flight);
            let _ = input_tx.send(AgentInput::Cancel).await;
        }
        "commands" => {
            // List custom commands with file path and first line.
            let cmds = state.commands.read().await;
            if cmds.is_empty() {
                raw_println(raw_mode, "(no custom commands)");
            } else {
                for cc in cmds.iter() {
                    let first_line = cc.content.lines().next().unwrap_or("");
                    raw_println(
                        raw_mode,
                        &format!("  /{}  {}  [{}]", cc.name, first_line, cc.path.display()),
                    );
                }
            }
        }
        "reload-commands" => {
            // Re-scan the commands directory without restarting.
            let dir = state.commands_dir.clone();
            let new_cmds = custom_commands::discover(&dir);
            let count = new_cmds.len();
            let mut cmds = state.commands.write().await;
            *cmds = new_cmds;
            raw_println(
                raw_mode,
                &format!("[reload-commands] {} command(s) loaded", count),
            );
        }
        _ => {
            // Built-in commands take precedence; otherwise look up a custom
            // command file and, if found, send its expanded content as a user
            // prompt.
            let cc = {
                let cmds = state.commands.read().await;
                cmds.iter().find(|c| c.name == cmd).cloned()
            };
            if let Some(cc) = cc {
                let prompt = custom_commands::expand_template(&cc.content, arg);
                if input_tx.send(AgentInput::UserPrompt(prompt)).await.is_err() {
                    raw_eprintln(raw_mode, "[error] failed to send custom command prompt");
                    return CommandResult::Continue;
                }
                // Record in history.
                let full_cmd = if arg.is_empty() {
                    format!("/{}", cmd)
                } else {
                    format!("/{} {}", cmd, arg)
                };
                push_history(state, &full_cmd).await;
                return CommandResult::SubmittedPrompt;
            }
            // Prefix match on custom commands. If exactly one custom command starts
            // with `cmd`, auto-execute it. If multiple, list candidates.
            let prefix_matches: Vec<CustomCommand> = {
                let cmds = state.commands.read().await;
                cmds.iter()
                    .filter(|c| c.name.starts_with(cmd))
                    .cloned()
                    .collect()
            };
            match prefix_matches.len() {
                0 => {
                    raw_eprintln(raw_mode, &format!("unknown command: {cmd}"));
                }
                1 => {
                    let cc = &prefix_matches[0];
                    let resolved_name = cc.name.clone();
                    let prompt = custom_commands::expand_template(&cc.content, arg);
                    raw_println(raw_mode, &format!("[auto] /{} → /{}", cmd, resolved_name));
                    if input_tx.send(AgentInput::UserPrompt(prompt)).await.is_err() {
                        raw_eprintln(raw_mode, "[error] failed to send custom command prompt");
                        return CommandResult::Continue;
                    }
                    let full_cmd = if arg.is_empty() {
                        format!("/{}", resolved_name)
                    } else {
                        format!("/{} {}", resolved_name, arg)
                    };
                    push_history(state, &full_cmd).await;
                    return CommandResult::SubmittedPrompt;
                }
                _ => {
                    raw_println(raw_mode, &format!("ambiguous command: /{cmd} matches:"));
                    for cc in &prefix_matches {
                        let first_line = cc.content.lines().next().unwrap_or("");
                        raw_println(raw_mode, &format!("  /{}  {}", cc.name, first_line));
                    }
                }
            }
        }
    }
    // Record built-in commands in history, except quit/exit.
    let full_cmd = if arg.is_empty() {
        format!("/{}", cmd)
    } else {
        format!("/{} {}", cmd, arg)
    };
    push_history(state, &full_cmd).await;
    CommandResult::Continue
}

/// `/spawn [name] [provider]` — launch a detached agent-cli peer sharing this
/// session's config (and therefore its `registry_dir`). The child outlives this
/// process (design §5, §6).
pub(super) async fn spawn_peer(arg: &str, state: &Arc<ReplState>, raw_mode: bool) {
    let mut it = arg.split_whitespace();
    let name = it.next().map(|s| s.to_string());
    let provider = it.next().map(|s| s.to_string());
    let run_args = RunArgs {
        name,
        // Inherit the launcher's group so the spawned child joins the same cohort.
        group: state.group.as_ref().map(|g| g.to_string()),
        provider,
        model: None,
        persona: None,
        auto_approve_tools: false,
        // A peer the REPL's /spawn creates exits when its work is done,
        // matching the spawn tool's children.
        exit_when_idle: true,
    };
    // A peer the REPL spawns belongs to this session's tree.
    // The child's chain is this session's chain plus this session itself.
    let mut chain = state.ancestors.clone();
    chain.push(state.id.clone());
    match crate::commands::spawn_detached(
        state.config_source.chain(),
        &state.registry_dir,
        &run_args,
        &chain,
    )
    .await
    {
        Ok(entry) => raw_println(
            raw_mode,
            &format!(
                "[spawn] detached agent id={} name={} pid={}",
                entry.id,
                entry.name.as_deref().unwrap_or("-"),
                entry.pid
            ),
        ),
        Err(e) => raw_eprintln(raw_mode, &format!("[spawn] failed: {e}")),
    }
}

/// `/stop <peer>` — request a peer (id or name) to shut down (design §7).
pub(super) async fn stop_peer_cmd(arg: &str, registry_dir: &Path, raw_mode: bool) {
    let peer = arg.trim();
    if peer.is_empty() {
        raw_eprintln(raw_mode, "usage: /stop <peer>");
        return;
    }
    if let Err(e) = crate::commands::stop_peer(registry_dir, peer).await {
        raw_eprintln(raw_mode, &format!("[stop] {e}"));
    }
}

pub(super) fn list_peers(registry_dir: &Path, raw_mode: bool) {
    match crate::ipc::registry::list_entries(registry_dir) {
        Ok(entries) => {
            if entries.is_empty() {
                raw_println(raw_mode, "(no agents running)");
                return;
            }
            for e in entries {
                let role = e
                    .persona
                    .as_ref()
                    .map(|p| p.role.clone())
                    .unwrap_or_default();
                raw_println(
                    raw_mode,
                    &format!(
                        "{}\t{}\t{}\t{}\t{}",
                        e.id,
                        e.name.clone().unwrap_or_else(|| "-".into()),
                        e.provider,
                        e.model,
                        role
                    ),
                );
            }
        }
        Err(e) => raw_eprintln(raw_mode, &format!("[error] {e}")),
    }
}

/// Parse the `/send` argument: an optional delivery option (`--prompt`, the
/// silent default; `--report`; `--ask`) followed by `<peer> <text>`. The
/// option reads naturally in either slot — `/send --ask <peer> <text>` or
/// `/send <peer> --report <text>` — and an unknown `--` word surfaces the
/// `send_to` tool's exact error, so both surfaces reject the same vocabulary.
/// The deprecated `wait_reply` boolean stays a tool-side concept.
pub(super) fn parse_send_args(
    arg: &str,
) -> std::result::Result<(Delivery, String, String), String> {
    const USAGE: &str = "usage: /send [--report|--ask] <peer> <text>";
    // Leading option tokens.
    let mut delivery: Option<Delivery> = None;
    let mut rest = arg;
    loop {
        let (tok, after) = match rest.split_once(' ') {
            Some((f, a)) => (f, a.trim_start()),
            None => (rest, ""),
        };
        if tok.starts_with("--") {
            delivery = Some(resolve_option_token(tok)?);
            rest = after;
        } else {
            break;
        }
    }
    // The peer, then possibly one option token right after it.
    let (peer_token, tail) = match rest.split_once(' ') {
        Some((f, a)) => (f, a.trim_start()),
        None => (rest, ""),
    };
    if tail.starts_with("--") {
        let (tok, after) = match tail.split_once(' ') {
            Some((f, a)) => (f, a.trim_start()),
            None => (tail, ""),
        };
        delivery = Some(resolve_option_token(tok)?);
        rest = after;
    } else {
        rest = tail;
    }
    let peer = peer_token.trim();
    let text = rest.trim();
    if peer.is_empty() || text.is_empty() {
        return Err(USAGE.to_string());
    }
    Ok((
        delivery.unwrap_or(Delivery::Prompt),
        peer.to_string(),
        text.to_string(),
    ))
}

/// Resolve an option token that carries its dashes (`--report` → Report; an
/// unknown `--verbose` errors with the tool's exact text, dashes and all).
pub(super) fn resolve_option_token(tok: &str) -> std::result::Result<Delivery, String> {
    match Delivery::resolve(Some(tok.trim_start_matches('-')), false) {
        Ok(d) => Ok(d),
        // The stripped word is not vocabulary either: re-resolve the token as
        // written so the error names what was typed.
        Err(_) => Delivery::resolve(Some(tok), false),
    }
}

pub(super) async fn send_to_peer(arg: &str, registry_dir: &Path, raw_mode: bool) -> CommandResult {
    let (delivery, peer_name, text) = match parse_send_args(arg) {
        Ok(parsed) => parsed,
        Err(why) => {
            raw_eprintln(raw_mode, &format!("[error] {why}"));
            return CommandResult::Continue;
        }
    };
    match crate::ipc::registry::resolve_peer(registry_dir, &peer_name) {
        Ok(p) => match delivery {
            Delivery::Prompt => {
                let msg = crate::ipc::IpcMessage::Prompt {
                    from: p.id.clone(),
                    from_name: None,
                    text,
                    reply_to: None,
                };
                if let Err(e) = crate::ipc::client::send(&p.socket, &msg).await {
                    raw_eprintln(raw_mode, &format!("[error] {e}"));
                } else {
                    raw_println(raw_mode, &format!("delivered to {}", p.id));
                }
                CommandResult::Continue
            }
            Delivery::Report => {
                match crate::tools::send_to::deliver_report(&p.id, registry_dir, &peer_name, &text)
                    .await
                {
                    Ok(outcome) => raw_println(raw_mode, &outcome),
                    Err(why) => raw_eprintln(raw_mode, &format!("[error] {why}")),
                }
                CommandResult::Continue
            }
            // The runner binds the reply socket and waits on its own task; the
            // loop keeps taking keys and the ask arm below prints the outcome
            // when it arrives, so a slow peer cannot hold the prompt hostage.
            Delivery::Ask => {
                let ask = crate::tools::send_to::PeerAsk::spawn(
                    &p.id,
                    registry_dir.to_path_buf(),
                    peer_name,
                    text,
                );
                raw_println(
                    raw_mode,
                    &format!("[send] asked {} (waiting for the reply, up to 120s)", p.id),
                );
                CommandResult::SpawnedAsk(ask)
            }
        },
        Err(e) => {
            raw_eprintln(raw_mode, &format!("[error] {e}"));
            CommandResult::Continue
        }
    }
}

pub(super) fn print_persona(persona: &Persona, raw_mode: bool) {
    raw_println(
        raw_mode,
        &format!(
            "name        : {}",
            persona.frontmatter.name.as_deref().unwrap_or("-")
        ),
    );
    raw_println(
        raw_mode,
        &format!("role        : {}", persona.frontmatter.role),
    );
    if !persona.frontmatter.skills.is_empty() {
        raw_println(
            raw_mode,
            &format!("skills      : {}", persona.frontmatter.skills.join(", ")),
        );
    }
    if let Some(d) = &persona.frontmatter.description {
        raw_println(raw_mode, &format!("description : {d}"));
    }
    if let Some(t) = &persona.frontmatter.temperature {
        raw_println(raw_mode, &format!("temperature : {t}"));
    }
    if let Some(allow) = &persona.frontmatter.allowed_tools {
        raw_println(raw_mode, &format!("allowed     : {}", allow.join(", ")));
    }
    if let Some(deny) = &persona.frontmatter.denied_tools {
        raw_println(raw_mode, &format!("denied      : {}", deny.join(", ")));
    }
    if let Some(p) = &persona.source_path {
        raw_println(raw_mode, &format!("source      : {}", p.display()));
    } else {
        raw_println(raw_mode, "source      : (builtin default)");
    }
}

pub(super) async fn reload_persona(
    state: &Arc<ReplState>,
    input_tx: &mpsc::Sender<AgentInput>,
    raw_mode: bool,
) {
    let resolution = match persona::resolve(
        state.cli_persona_path.as_deref(),
        &state.persona_file_setting,
        &state.agents_dir,
        state.name.as_deref(),
        state.project_persona_path.as_deref(),
    ) {
        Ok(r) => r,
        Err(e) => {
            raw_eprintln(raw_mode, &format!("[error] {e}"));
            return;
        }
    };
    let prompt = resolution.persona.to_system_prompt();
    {
        let mut guard = state.persona.write().await;
        *guard = resolution.persona;
    }
    if let Err(e) = input_tx.send(AgentInput::SetSystemPrompt(prompt)).await {
        raw_eprintln(raw_mode, &format!("[error] {e}"));
        return;
    }
    let p = state.persona.read().await;
    raw_println(
        raw_mode,
        &format!(
            "persona reloaded from {}",
            match &p.source_path {
                Some(path) => path.display().to_string(),
                None => "(builtin default)".to_string(),
            }
        ),
    );
}

pub(super) fn peer_summary(arg: &str, registry_dir: &Path, raw_mode: bool) {
    let key = arg.trim();
    if key.is_empty() {
        raw_eprintln(raw_mode, "usage: /peer <id_or_name>");
        return;
    }
    match crate::ipc::registry::resolve_peer(registry_dir, key) {
        Ok(e) => {
            println!(
                "[{}] name={} provider={} model={}",
                e.id,
                e.name.unwrap_or_else(|| "-".into()),
                e.provider,
                e.model
            );
            if let Some(p) = &e.persona {
                println!("role        : {}", p.role);
                if !p.skills.is_empty() {
                    println!("skills      : {}", p.skills.join(", "));
                }
                if let Some(d) = &p.description {
                    println!("description : {d}");
                }
            } else {
                println!("(no persona summary)");
            }
        }
        Err(e) => raw_eprintln(raw_mode, &format!("[error] {e}")),
    }
}
