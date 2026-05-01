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
├── main.rs              ... CLI エントリ／サブコマンド分岐
├── cli.rs               ... clap 引数定義
├── app.rs               ... `run` の REPL 本体
├── agent.rs             ... 単一エージェントの会話ループ
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
    ├── server.rs        ... UnixListener（0600）
    ├── client.rs        ... UnixStream
    └── registry.rs      ... <agent-id>.{sock,json} 走査
```

## 3. 主要データフロー

### 3.1 ユーザープロンプト処理

```text
stdin -> stdin task -> mpsc -> Agent loop -> Provider -> ProviderEvent stream
                                                  |
                                                  +-- text_delta -> mpsc -> display
                                                  +-- thinking -> mpsc -> display
                                                  +-- tool_use -> ToolRegistry -> ToolOutput
                                                                                 |
                                                                                 v
                                                                  Agent loop へ次反復
```

ツール実行は最大 8 反復。誤発火を防ぐため `auto_approve_tools=false` の場合は y/N 承認を経由します。

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

## 7. 対象 OS

Linux のみ。Unix ドメインソケット、`XDG_RUNTIME_DIR`、`/proc/<pid>` を前提に実装しています。
