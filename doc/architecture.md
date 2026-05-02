# アーキテクチャ概要（`architecture.md`）

`AI_PRJ_DESIGN.md` の要約版です。実装の地図として読んでください。

## 1. 全体像

```text
+--------------------+        +--------------------+
| agent-cli (proc A) |        | agent-cli (proc B) |
|  - 1 AI agent      |        |  - 1 AI agent      |
|  - REPL front-end  |        |  - REPL front-end  |
|  - Tools registry  |        |  - Tools registry  |
|  - IPC server      |<------>|  - IPC server      |
|  - IPC client      | local  |  - IPC client      |
+----------+---------+  IPC   +----------+---------+
           |                              |
           v                              v
       AI Provider API              AI Provider API

レジストリディレクトリ:
  $XDG_RUNTIME_DIR/agent-cli/   または /tmp/agent-cli/
    └─ <agent-id>.sock   ... 各プロセスのIPCソケット
    └─ <agent-id>.json   ... メタ情報（name/provider/model/persona/...）
```

- 1 プロセス＝1 エージェント。
- プロセス間連携は **ローカル Unix ドメインソケット**（`0600`）。外部公開ポートは開きません。
- バックエンドへの HTTP 通信はプロセスごとに独立。

## 2. モジュール構成

```text
src/
├── main.rs              ... CLI エントリ／サブコマンド分岐／std::process::exit による確定終了
├── cli.rs               ... clap 引数定義
├── app.rs               ... `run` の REPL 本体／run_input_loop／PromptState／handle_auto_command／wait_for_termination_signal
├── agent.rs             ... 単一エージェントの会話ループ／ApprovalRequest／request_approval
├── commands.rs          ... list/send/providers/doctor/selftest/config
├── config.rs            ... 設定ファイル読込・解決順序
├── id.rs                ... AgentId
├── persona.rs           ... ペルソナ（YAML+本文）
├── log.rs               ... 会話ログ
├── error.rs             ... AppError
├── ai/
│   ├── mod.rs           ... Provider trait, build()
│   ├── claude.rs        ... Anthropic Messages（SSE, thinking, tool_use）
│   ├── codex.rs         ... OpenAI Chat Completions（SSE, function calling）
│   ├── ollama.rs        ... Ollama /api/chat（NDJSON, tool_calls）
│   ├── llamacpp.rs      ... llama.cpp /v1/chat/completions（OpenAI 互換）
│   ├── tool_bridge.rs   ... ツール定義の形式変換
│   └── stream.rs        ... SSE フレームの組み立て
├── tools/
│   ├── mod.rs           ... Tool trait, ToolRegistry
│   ├── shell.rs
│   ├── fs_read.rs
│   ├── fs_write.rs
│   └── send_to.rs
└── ipc/
    ├── mod.rs           ... IpcMessage
    ├── server.rs        ... UnixListener（0600）／Drop で accept abort + ソケット削除
    ├── client.rs        ... UnixStream
    └── registry.rs      ... <agent-id>.{sock,json} 走査／Drop で自動クリーンアップ
```

主要型：

- `Agent.auto_approve: Arc<AtomicBool>` — `/auto on|off` で実行時切替
- `Agent.approval_tx: Option<mpsc::Sender<ApprovalRequest>>` — 入力ループへの承認要求経路
- `enum PromptState { Ready, Pending, AwaitingApproval(oneshot::Sender<bool>) }` — REPL 入力ループの状態

## 3. 主要データフロー

### 3.1 ユーザープロンプト処理

```text
stdin -> run_input_loop -> mpsc -> Agent loop -> Provider -> ProviderEvent stream
            ^                          |
            |                          +-- text_delta -> mpsc -> display task -> stdout
            |                          +-- thinking   -> mpsc -> display task -> stdout
            |                          +-- tool_use   -> 承認 (3.3) -> ToolRegistry -> ToolOutput
            |                          +-- Done       -> mpsc -> display task -> agent_idle 通知 -> 入力ループ
            |
            +-- agent_idle 受領で Pending -> Ready に復帰し、次のプロンプト `> ` を再描画
```

- `run_input_loop` は `enum PromptState { Ready, Pending, AwaitingApproval(oneshot::Sender<bool>) }` を保持し、`tokio::select!` で 4 経路（shutdown／idle／approval／stdin）を多重化。
- ユーザー入力送信直後は `Pending` に遷移し、`Done` 受領（`display_task` から `mpsc::<()>` 経由）まで stdin 読取を抑止。これによりストリーミング出力と入力エコーの混在を防ぐ。
- ツール実行は最大 8 反復。`auto_approve_tools=false`（既定）の場合は 3.3 の承認チャネル経由で y/N を取得する。
- `Done` は通常応答完了だけでなく、`provider.complete_stream` の失敗時にも必ず発行され、入力ループが Pending のまま固まらない。

### 3.2 ピア間メッセージング

```text
proc A                                          proc B
------                                          ------
/send bob "hi" or send_to tool
   │
   ▼
ipc::client::send (UnixStream)
   │ JSONL: {"kind":"prompt","from":"<A id>","text":"hi"}
   ▼
                                          UnixListener
                                              │
                                              ▼
                                        IpcMessage::Prompt
                                              │
                                              ▼ mpsc
                                       AgentInput::PeerPrompt
                                              │
                                              ▼
                                       Agent loop (B)
                                              │
                                              ▼
                                      Provider 応答 -> 画面表示
```

### 3.3 ツール実行承認の入出力統合

承認は agent タスクと入力ループ間の 2 本立てチャネルで行います（`std::io::stdin` 直読みは禁止）。

```text
Agent::process_turn (auto_approve=false)
   │
   ├── ApprovalRequest { tool_name, args, response: oneshot::Sender<bool> }
   │       │
   │       ▼ mpsc::Sender<ApprovalRequest>
   │   run_input_loop  (PromptState::AwaitingApproval(resp_tx) に遷移)
   │       │
   │       │ "[tool approval] ... approve? [y/N]:" を描画
   │       │
   │       ▼ stdin から次の 1 行を取得
   │   y/yes -> resp_tx.send(true)、それ以外 -> false
   │       │
   │       ▼ oneshot::Receiver<bool>
   └── 承認結果に応じて tool 実行 or "user denied tool execution"
```

- `auto_approve` は `Arc<AtomicBool>` で agent と REPL 間で共有され、REPL コマンド `/auto on|off|status` で実行時切替可能。
- 承認待機中（`AwaitingApproval`）に shutdown シグナルが入ると、`resp_tx.send(false)` で安全側倒し → agent の `oneshot::Receiver::await` が即解消し、ぶら下がりを防止。

## 4. レジストリ仕様

`<registry_dir>/<agent-id>.json`：

```json
{
  "id":"agent-01HX...",
  "name":"alice",
  "pid":12345,
  "started_at":"2026-05-01T10:00:00Z",
  "provider":"claude",
  "model":"claude-opus-4-7",
  "socket":"/tmp/agent-cli/agent-01HX....sock",
  "persona": {"role":"...","skills":[...],"description":"...","source_path":"..."}
}
```

走査時：

- `*.json` を読み、対応する `*.sock` の存在を確認
- 同期に `/proc/<pid>` の存在で PID 生存確認
- いずれかが欠けていれば stale として `<agent-id>.{sock,json}` を掃除

## 5. Provider 抽象

```rust
#[async_trait]
trait Provider {
    fn name(&self) -> &'static str;       // "claude" | "codex" | "ollama" | "llama.cpp"
    fn capabilities(&self) -> Capabilities;
    fn model(&self) -> &str;
    async fn complete_stream(&self, messages: &[Message], tools: &[ToolSpec])
        -> Result<EventStream<'_>>;
}

enum ProviderEvent {
    Thinking { text: String },
    Text     { delta: String },
    ToolUse  { id: String, name: String, args: Value },
    Done,
    Error    { message: String },
}
```

各バックエンドは内部表現が違っても、同じ `ProviderEvent` 列に正規化して上位に渡します。

## 6. ペルソナ機構

優先順位：

```text
1. --persona <path>
2. [runtime] persona_file
3. <agents_dir>/<name>.md
4. 組み込み既定（汎用アシスタント）
```

ペルソナの `role`／`skills`／本文はシステムプロンプトに合成。`allowed_tools`／`denied_tools` は `ToolRegistry::build` で反映され、結果は `/tools` で確認できます。再読込は REPL の `/reload-persona`（履歴保持）。

## 7. 終了処理（shutdown coordination）

`/quit`／`/exit`／`Ctrl+D`（EOF）／`Ctrl+C`（SIGINT）／`SIGTERM` のいずれを契機としても、同じ終了シーケンスへ合流します。

```text
[/quit /exit ハンドラ]   [stdin EOF 検出]   [SIGINT/SIGTERM ハンドラ]
              \              |              /
               \             v             /
                +-- shutdown_tx.send(true) (tokio::sync::watch) --+
                                    │
                                    ▼
        ┌─────────────────────────────────────┐
        │ stdin_task.abort()                  │
        │ ipc_task.abort()                    │
        │ signal_task.abort()                 │
        │ drop(input_tx)                      │
        │ agent_handle (500ms タイムアウト)    │
        │ display_task.await                  │
        │ drop(ipc_server)  → IpcServer::Drop │
        │   - accept ループ abort             │
        │   - <id>.sock 削除                  │
        │ registry_handle (RegistryHandle::Drop)│
        │   - <id>.sock / <id>.json 削除      │
        └─────────────────────────────────────┘
                                    │
                                    ▼
                          std::process::exit(0)
```

- `IpcServer` と `RegistryHandle` は `Drop` 実装で abort + ファイル削除を行うため、panic 時にも残骸が残らない。
- `main` は `std::process::exit(0/1)` を明示的に呼び、tokio runtime drop が `tokio::io::stdin()` のブロッキングスレッドを待つ事象を回避。
- 開発機では 5 経路すべて 1 秒以内に正常終了し、レジストリ残留物なしを確認済（`/quit` 110ms / `/exit` 110ms / `Ctrl+D` 110ms / `SIGINT` 19ms / `SIGTERM` 3ms）。
- 承認待機中（`AwaitingApproval`）の場合は、入力ループ break 時に `oneshot::Sender::send(false)` で安全側倒し（3.3 参照）。

## 8. 対象 OS

Linux のみ。Unix ドメインソケット、`XDG_RUNTIME_DIR`、`/proc/<pid>`、`tokio::signal::unix` を前提に実装しています。
