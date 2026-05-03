# 設計仕様書（AI_PRJ_DESIGN）

## 1. システム構成

### 1.1 全体像

```
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
       (Anthropic Claude)           (Anthropic Claude)
                                          
レジストリディレクトリ:
  $XDG_RUNTIME_DIR/agent-cli/  (なければ /tmp/agent-cli/)
    └─ <agent-id>.sock   ... 各プロセスのIPCソケット
    └─ <agent-id>.json   ... メタ情報（PID、表示名、起動時刻、モデル名）
```

- 1プロセス＝1エージェント。プロセス内に複数エージェントは存在しない。
- 各プロセスはローカルにIPCサーバー（Unixドメインソケット）を立ち上げ、別プロセスからのプロンプトを受け付ける。
- ピア検出はレジストリディレクトリのソケット／メタファイル走査により行う。
- AIプロバイダー（既定はAnthropic Claude API）への通信はプロセスごとに独立して行う。

### 1.2 起動オプションとサブコマンド

#### 共通グローバルオプション

すべてのサブコマンドに先行して指定できる。

| オプション | 説明 |
|------------|------|
| `--config <path>` | 使用する設定ファイルパスを指定。未指定時は`~/.config/agent-cli/config.toml`。 |

**注意**：`run`サブコマンドのオプション（`--name`、`--provider`、`--model`、`--persona`、`--auto-approve-tools`）は、サブコマンド省略時（例：`agent-cli --persona <path>`）でも等価に解釈される（FR-01）。実装は`Cli`構造体に`RunArgs`を`#[command(flatten)]`でフラット化し、各フィールドに`global = true`を付与することで対応済み（T-508）。

#### サブコマンド

| 形式 | 説明 |
|------|------|
| `agent-cli` または `agent-cli run` | REPLを起動し、1エージェントとして対話開始。 |
| `agent-cli run --name <name>` | 表示名を指定して起動。 |
| `agent-cli run --provider <name>` | AIバックエンド（`claude`／`codex`／`ollama`／`llama.cpp`）を指定して起動。 |
| `agent-cli run --model <model>` | バックエンドのモデル名を上書き指定。 |
| `agent-cli run --persona <path>` | エージェントペルソナファイル（役割・スキル定義）を指定して起動。`agent-cli --persona <path>`（サブコマンド省略）も等価とする（FR-01）。 |
| `agent-cli --config <path> run ...` | 任意の設定ファイルで起動。 |
| `agent-cli list` | レジストリを走査し、稼働中のピア一覧を出力。 |
| `agent-cli send <peer> <text>` | 指定ピアにプロンプトを送信して終了（受信側で応答処理）。 |
| `agent-cli providers` | 利用可能なバックエンドと設定状況を一覧表示。 |
| `agent-cli doctor` | 設定・APIキー・バックエンド疎通・レジストリ・シェルツールを点検。 |
| `agent-cli selftest [--provider <name>]` | 短いプロンプトとツール実行で動作確認するスモークテスト。 |
| `agent-cli config show` | 設定を表示。 |
| `agent-cli config edit` | 設定をエディタで開く。 |
| `agent-cli config path` | 現在使用される設定ファイルの解決済みパスを表示。 |

### 1.3 REPLコマンド（`run`中の入力で`/`プレフィックス）

| コマンド | 説明 |
|----------|------|
| `/list` | ピア一覧表示。 |
| `/send <peer> <text>` | ピアへプロンプト送信。 |
| `/tools` | 有効ツール一覧表示。 |
| `/cancel` | 進行中のAI応答／ツール実行を中断。 |
| `/persona` | 現在のペルソナ（役割・スキル）を表示。 |
| `/reload-persona` | ペルソナファイルを再読込してシステムプロンプトに反映（履歴は保持）。 |
| `/peer <id>` | 指定ピアのペルソナ概要を表示。 |
| `/help` | ヘルプ表示。承認スキップ手段（`/auto`／`auto_approve_tools`／`--auto-approve-tools`）を必ず併記する（FR-04-2）。 |
| `/auto [on\|off\|status]` | ツール承認スキップの実行時切替（FR-04-2）。引数なし／`status` で現在値表示。 |
| `/quit` | アプリ終了。進行中のAI応答／ツール実行をキャンセルし、IPCソケット・レジストリメタを削除して即時終了する。 |
| `/exit` | `/quit` の完全エイリアス。同一の終了シーケンスを起動する（FR-13）。 |
| （`/`なし入力） | 自エージェントへの通常プロンプト。 |

REPL外の終了経路として、標準入力EOF（`Ctrl+D`）／`Ctrl+C`（SIGINT）／`SIGTERM`も `/quit` と同等の終了処理（4.9）に合流させる。

## 2. モジュール構成（Rust crate構造）

```
agent-cli/
├── Cargo.toml
├── src/
│   ├── main.rs              // clapでサブコマンドを分岐するエントリ。
│   ├── cli.rs               // CLI引数定義（clap derive）。
│   ├── app.rs               // REPLフロントエンド本体（runサブコマンドの実装）。
│   ├── config.rs            // 設定ファイル読み書き。
│   ├── agent.rs             // 単一エージェントの会話ループ。
│   ├── id.rs                // AgentId生成・パース。
│   ├── persona.rs           // ペルソナファイル（YAMLフロントマター＋本文）の読込・検証。
│   ├── ai/
│   │   ├── mod.rs           // AIプロバイダー抽象（trait Provider）とファクトリ。
│   │   ├── claude.rs        // Anthropic Claude API（messages stream + tool_use + thinking）。
│   │   ├── codex.rs         // OpenAI（Chat/Responses API、tool calling対応）。
│   │   ├── ollama.rs        // Ollama HTTP API（/api/chat、stream、tools対応）。
│   │   ├── llamacpp.rs      // llama.cppサーバー（OpenAI互換 /v1/chat/completions）。
│   │   ├── tool_bridge.rs   // tool_use表現の差異を吸収する内部型変換。
│   │   └── stream.rs        // ストリーミング応答パース共通処理。
│   ├── tools/
│   │   ├── mod.rs           // Tool抽象（trait Tool）とレジストリ。
│   │   ├── shell.rs         // シェルコマンド実行ツール。
│   │   ├── fs_read.rs       // ファイル読み取りツール。
│   │   ├── fs_write.rs      // ファイル書き込み／編集ツール。
│   │   └── send_to.rs       // ピアエージェント宛て送信ツール。
│   ├── ipc/
│   │   ├── mod.rs           // IPCメッセージ型。
│   │   ├── server.rs        // Unixドメインソケットのlistener。
│   │   ├── client.rs        // 送信側クライアント。
│   │   └── registry.rs      // レジストリディレクトリの走査・登録・解除。
│   ├── log.rs               // 会話・ツールログ出力。
│   └── error.rs             // エラー型定義。
└── README.md
```

## 3. 主要データ構造

### 3.1 AgentId

```rust
pub struct AgentId(pub String); // ULIDベースの一意ID
impl AgentId {
    pub fn new() -> Self { /* ULID生成 */ }
}
impl Display for AgentId { /* "agent-01HX..." */ }
```

表示名（`name`）はメタ情報として別途保持し、レジストリ表示時に併記する。

### 3.2 設定（`Config`）

```toml
# ~/.config/agent-cli/config.toml
[provider]
# 使用するバックエンド: "claude" | "codex" | "ollama" | "llama.cpp"
kind = "claude"

[provider.claude]
model       = "claude-opus-4-7"
api_key_env = "ANTHROPIC_API_KEY"
base_url    = "https://api.anthropic.com"
thinking    = true

[provider.codex]
model       = "gpt-4.1"   # 例
api_key_env = "OPENAI_API_KEY"
base_url    = "https://api.openai.com/v1"

[provider.ollama]
model    = "llama3.1:8b"
base_url = "http://127.0.0.1:11434"

[provider."llama.cpp"]
model    = "default"
base_url = "http://127.0.0.1:8080"   # OpenAI互換エンドポイント

[runtime]
auto_approve_tools = false
log_dir            = "~/.local/share/agent-cli/logs"
registry_dir       = ""   # 空ならXDG_RUNTIME_DIR/agent-cli または /tmp/agent-cli を使用
agents_dir         = "~/.config/agent-cli/agents"
persona_file       = ""   # 空なら <agents_dir>/<name>.md → 組み込み既定 の順で解決

[tools]
enabled = ["shell", "fs_read", "fs_write", "send_to"]

[tools.shell]
timeout_secs   = 60
max_output_kb  = 256

[ui]
show_thinking = "collapsed"   # "collapsed" | "expanded" | "hidden"
```

`agent-cli run --provider <name>`／`--model <model>`は当該セクションの値を一時的に上書きする。

#### 設定ファイル解決の優先順位

1. グローバルオプション`--config <path>`で明示指定されたパス。
2. 環境変数`AGENT_CLI_CONFIG`に設定されたパス（任意）。
3. 既定パス`~/.config/agent-cli/config.toml`。

挙動：

- (1) または (2) が指定された場合、そのファイルが存在しなければエラーで終了する（自動生成しない）。
- (3) が使用される場合のみ、ファイルが存在しなければ既定値で生成する。
- 解決後のパスは`agent-cli config path`で確認できる。
- 同一ホストで異なる設定ファイルを指定した複数の`agent-cli`プロセスを並行起動できる。`registry_dir`を別々にすれば独立した名前空間として動作し、同一にすれば相互にピアとして検出できる。

### 3.3 IPCメッセージ

```rust
#[derive(Serialize, Deserialize)]
pub enum IpcMessage {
    Prompt { from: AgentId, from_name: Option<String>, text: String },
    Ack    { id: u64 },
    Ping,
    Pong,
}
```

### 3.4 レジストリメタ情報

```rust
#[derive(Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id:         AgentId,
    pub name:       Option<String>,
    pub pid:        u32,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub provider:   String,                    // "claude" | "codex" | "ollama" | "llama.cpp"
    pub model:      String,
    pub socket:     PathBuf,
    pub persona:    Option<PersonaSummary>,    // ペルソナの概要（role / skills / description）
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PersonaSummary {
    pub role:        String,
    pub skills:      Vec<String>,
    pub description: Option<String>,
    pub source_path: Option<PathBuf>,
}
```

`<registry_dir>/<agent-id>.json`として保存し、起動時に書き、終了時に削除する。

### 3.5 ペルソナ（`persona.rs`）

```rust
#[derive(Deserialize)]
pub struct PersonaFrontmatter {
    pub name:           Option<String>,
    pub role:           String,
    pub skills:         Vec<String>,
    pub description:    Option<String>,
    pub model:          Option<String>,
    pub temperature:    Option<f32>,
    pub allowed_tools:  Option<Vec<String>>,
    pub denied_tools:   Option<Vec<String>>,
}

pub struct Persona {
    pub frontmatter: PersonaFrontmatter,
    pub body:        String,           // システムプロンプトに連結する本文
    pub source_path: Option<PathBuf>,
}

impl Persona {
    pub fn load(path: &Path) -> Result<Self>;     // YAMLフロントマター + Markdown本文を分離
    pub fn builtin_default() -> Self;             // 組み込み汎用ペルソナ
    pub fn to_system_prompt(&self) -> String;     // role/skills/bodyからシステムプロンプトを合成
    pub fn summary(&self) -> PersonaSummary;
}
```

#### ペルソナファイル例

```markdown
---
name: alice
role: コードレビュアー
skills:
  - Rust
  - 静的解析
  - セキュリティレビュー
description: 安全性とパフォーマンスを重視するレビュアー
allowed_tools:
  - shell
  - fs_read
denied_tools:
  - fs_write
---

あなたは熟練のコードレビュアーです。常に以下を意識してレビューしてください。

- 所有権・ライフタイム上の問題を最優先で指摘する
- パフォーマンスへの影響を定量的に述べる
- 修正案を提示する際は最小差分を心がける
```

#### システムプロンプトの合成順

1. 組み込みのベースシステムプロンプト（agent-cliとしての基本指示）。
2. ペルソナ`role`／`skills`／`description`から構築した役割サマリ。
3. ペルソナ本文（Markdown）。

### 3.6 Provider抽象

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;            // "claude" | "codex" | "ollama" | "llama.cpp"
    fn capabilities(&self) -> Capabilities;    // thinking / tool_use / streaming など
    async fn complete_stream(
        &self,
        messages: &[Message],
        tools:    &[ToolSpec],
    ) -> Result<BoxStream<'_, ProviderEvent>>;
}

pub struct Capabilities {
    pub streaming: bool,
    pub tool_use:  bool,
    pub thinking:  bool,
}

pub enum ProviderEvent {
    Thinking { text: String },
    Text     { delta: String },
    ToolUse  { id: String, name: String, args: serde_json::Value },
    Done,
    Error    { message: String },
}
```

ファクトリ`ai::build(config: &Config) -> Box<dyn Provider>`が、`provider.kind`に応じて該当実装を返す。

#### バックエンドごとの実装方針

| バックエンド | tool_use | thinking | ストリーミング | 備考 |
|--------------|----------|----------|----------------|------|
| claude | ネイティブ対応 | ネイティブ対応 | SSE | Anthropic Messages API。最も機能が揃う基準実装。 |
| codex | function calling | 非対応（疑似実装：応答前に`<thinking>`タグを要求するプロンプトで近似） | SSE | OpenAI互換。`tool_use`はOpenAIのfunction callingへ変換。 |
| ollama | tools対応モデル時のみ（`/api/chat`の`tools`） | 非対応 | NDJSON | モデルがtoolsをサポートしない場合は`Capabilities::tool_use=false`。 |
| llama.cpp | OpenAI互換のtools対応版で利用可 | 非対応 | SSE | `/v1/chat/completions`を利用。サーバー側ビルドに依存。 |

`tool_bridge.rs`が、Claudeのcontent block表現とOpenAI形式のfunction call表現を共通の`ProviderEvent::ToolUse`へ正規化する。

### 3.7 Tool抽象

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn schema(&self) -> serde_json::Value;
    async fn invoke(&self, args: serde_json::Value, ctx: &ToolCtx) -> Result<ToolOutput>;
}

pub struct ToolCtx<'a> {
    pub self_id:  &'a AgentId,
    pub config:   &'a Config,
    pub registry: &'a Registry,   // send_toツールがピア解決に利用
}
```

### 3.8 Agent内部メッセージ

```rust
pub enum AgentInput {
    UserPrompt(String),                                           // REPL直接入力
    PeerPrompt { from: AgentId, from_name: Option<String>, text: String }, // IPC受信
    SetSystemPrompt(String),                                      // ペルソナ再読込時にシステムプロンプトを差し替え
    Cancel,
}

pub enum AgentEvent {
    Thinking  { text: String },
    Text      { delta: String },
    ToolCall  { name: String, args: serde_json::Value },
    ToolResult{ name: String, ok: bool, output: String },
    Done,
    Error     { message: String },
    Info      { message: String },                                // 補助情報（cancel受領、システムプロンプト更新通知など）
}
```

`SetSystemPrompt`受信時は、`history`先頭の`Message::System`を新しい内容で差し替える（無ければ先頭に挿入）。会話履歴のユーザー／アシスタント発話は保持する。

## 4. 主要フロー

### 4.1 起動シーケンス（`run`サブコマンド）

1. CLI引数を解釈し`run`であれば以下を実行。
2. 設定ファイルパスを解決（`--config` → `AGENT_CLI_CONFIG` → 既定パスの優先順位）。
3. 設定ファイルを読み込む。明示指定パスが存在しなければエラー終了、既定パスかつ未存在の場合のみ既定値で生成。`--provider`／`--model`オプションがあれば設定値を上書き。
4. `AgentId`を生成。
5. ペルソナファイルを解決して読み込む。優先順位は (1) `--persona <path>` → (2) `[runtime] persona_file` → (3) `~/.config/agent-cli/agents/<name>.md` → (4) 組み込み既定。明示指定が未存在の場合はエラー終了、規定パス未存在は組み込みへフォールバック。
6. ペルソナの`allowed_tools`／`denied_tools`を反映してツールレジストリを構築。Provider未対応のtool_useは警告を出す。
7. レジストリディレクトリを準備し、IPCサーバー（`<registry_dir>/<agent-id>.sock`）を起動。パーミッション0600。
8. `<registry_dir>/<agent-id>.json`にメタ情報（バックエンド種別・モデル名・ペルソナサマリを含む）を書き込む。
9. `ai::build(&config)`でバックエンドに応じたProvider実装を構築し、必要な接続確認（APIキー存在チェック、ローカルサーバー疎通）を行う。ペルソナの`model`／`temperature`があればここで反映。
10. システムプロンプトを合成（ベース＋ペルソナ`role`／`skills`／本文）し、会話履歴の先頭にセット。
11. REPLヘッダーに`name`／`role`／`skills`を表示し、入力ループを開始。
12. 終了時はIPCソケットとメタファイルを削除。

### 4.2 入力処理ループ

REPL入力・IPC受信・終了シグナル・AI応答完了通知を`tokio::select!`で合流させる。

1. 入力ソース：
   - 標準入力（`crossterm`／`tokio::io::BufReader::lines`によるライン入力）。`lines()`が`None`を返した時点（EOF＝`Ctrl+D`）は終了要求として扱う。
   - IPCサーバーから`mpsc`で流入するメッセージ。
   - シグナル（`tokio::signal`によるSIGINT／SIGTERM受信）。
   - 共通の`tokio::sync::watch`型shutdownチャネル（`/quit`ハンドラやシグナルハンドラから発火）。
   - 会話ループから流入する「AI応答完了通知」（FR-03-2 のためのプロンプト再描画トリガー、4.2A 参照）。
2. 入力先頭が`/`ならREPLコマンドとしてDispatcherへ。`/quit`はshutdownチャネルへ`true`を送り、当該ループを抜ける。
3. それ以外は`AgentInput::UserPrompt`として会話ループへ。
4. IPC受信は`AgentInput::PeerPrompt`として会話ループへ。
5. EOF／SIGINT／SIGTERMのいずれを検出した場合も、shutdownチャネルへ通知し、4.9の終了処理に合流する。

### 4.2A プロンプト同期（FR-03-2 対応）

FR-03-2「REPL入出力サイクル」を実装するため、入力プロンプト（`> `）の描画は AI 応答の境界に合わせる。具体的には次のとおり。

- 状態管理：`enum PromptState { Ready, Pending }` を入力ループが保持する。`Ready` のときのみ標準入力からの読取を行い、入力受領で `Pending` に遷移する。
- 通知経路：会話ループ（`agent.rs`）が `AgentEvent::Done` を発行する直前または直後に、入力ループへ「AI 応答完了」通知を送る。これにより `Pending` → `Ready` に戻し、改行＋`> ` を再描画する。
  - 実装手段は専用の `tokio::sync::Notify` または `mpsc::channel::<()>` を 1 本立て、display task または agent タスクから `notify_one()` する。
- IPC 経由の `PeerPrompt` も同様に AI 応答を引き起こすため、同じ完了通知に乗せる。`PromptState` がすでに `Ready` の場合（ユーザー入力なしで peer プロンプトが処理された場合）は通知を捨ててもよい。
- ストリーミング途中でユーザーが Enter を押しても、`Pending` 状態のため新しい行入力は受け付けず、内部バッファにエコー描画もしない（端末の表示は荒れない）。
- `/cancel`／SIGINT などで応答を中断した場合も、入力ループを `Ready` に戻して次のプロンプトを表示する。
- `auto_approve_tools=false` でツール承認プロンプトが出る間は、当該プロンプト独自の y/N 待機が優先される（プロンプト同期はそれより上位の境界で機能する）。

#### 視覚レイアウトの保証

`Pending → Ready` 遷移時に出力されるシーケンスは次の不変条件を満たすこと（FR-03-2）：

1. AI 応答の最終 Text／Thinking／Tool 出力の末尾が改行で終わっていない場合でも、`AgentEvent::Done` 受領時に `display_event` が `println!()` を実行し、必ず改行を 1 つ加える。
2. その後 `Pending → Ready` 遷移を契機に、入力ループ次イテレーションで `print_prompt()` が `> ` を出力する。
3. 結果として端末上には次の構造が現れる：

   ```
   > <ユーザー入力>
   <AI 応答>
   >
   ```

4. ユーザー入力直後（Pending 進入時）は新しい `> ` を絶対に描画しない。応答前に二重プロンプトが見える事象は本設計で禁止される。
5. もし応答が空で終わるケース（テキスト・思考・ツール呼び出しいずれも emit されない `Done` のみ）でも、`println!()` による改行は必ず実施し、次のプロンプトが現在行の末尾にくっつかないようにする。

これにより、ストリーミング出力と REPL プロンプトが画面上で混在せず、応答完了後に必ず明示的に新しい入力プロンプトが描画される。

### 4.3 会話ループ（`agent.rs`）

1. `AgentInput`を受信し、種類に応じて会話履歴へ追加。
   - `PeerPrompt`は送信元IDを明示してsystem側で履歴に記録。
2. AIプロバイダーへリクエスト送信。
3. ストリーミング応答を受信して`AgentEvent`を発行。
   - thinking → 設定に応じて折りたたみ／展開／非表示で描画。
   - tool_use → ツールレジストリで解決。
     - `auto_approve_tools=false`ならy/N承認。
     - 実行 → 結果を会話履歴へ追加し、AIへ続報送信。
   - text → 逐次stdoutへ描画。
4. 応答完了（`AgentEvent::Done` 発行時）に、入力ループへ「次のプロンプト準備可」を通知（4.2A）。
5. 次入力待機に戻る。

### 4.3A ツール承認の入出力統合（FR-04-1）

承認プロンプト（"approve? [y/N]:"）と応答は、REPL のメイン入力ループ（4.2／4.2A）と stdin 読取経路を共有する。`std::io::stdin().read_line()` を agent タスクから直接呼ぶ実装は禁止する（tokio 側の stdin reader と OS パイプを奪い合い、入力が取り違えられるため）。

#### 構成要素

- `ApprovalRequest` 構造体：`{ name: String, args: Value, response: tokio::sync::oneshot::Sender<bool> }`。
- `Agent` 構造体に `approval_tx: Option<tokio::sync::mpsc::Sender<ApprovalRequest>>` を持たせる（無設定時は従来どおり自動拒否扱い、または auto_approve に従う）。
- 入力ループの状態を 3 値に拡張：`enum PromptState { Ready, Pending, AwaitingApproval(tokio::sync::oneshot::Sender<bool>) }`。
- `app::run` で `mpsc::channel::<ApprovalRequest>(8)` を生成し、`agent` と `run_input_loop` に分配する。

#### シーケンス

1. agent が tool_use を受領し、`auto_approve == false` のとき：
   - `(resp_tx, resp_rx) = oneshot::channel()` を作る。
   - `approval_tx.send(ApprovalRequest { name, args, response: resp_tx }).await`。
   - `let approved = resp_rx.await.unwrap_or(false);`
2. 入力ループ：
   - `tokio::select!` の arm に `approval_rx.recv()` を追加（`!matches!(state, AwaitingApproval(_))` ガード）。
   - 受領時：承認バナーを stdout へ描画し、状態を `AwaitingApproval(resp_tx)` に遷移。
   - 次の stdin 行を読み、`y/Y/yes/Yes/YES` のいずれかで `true`、それ以外で `false` として `resp_tx.send()`。
   - 状態を `Pending` に戻す（agent が tool 実行→続報→Done を出すまで stdin 読取は停止）。
3. agent は `approved` を踏まえ、ツールを実行または拒否としてログし、続報へ進む。

#### `/auto` の扱い

- `auto_approve` を `Arc<std::sync::atomic::AtomicBool>` で共有する。
- `Agent` が承認チェックする際、まず `auto_approve.load(Ordering::SeqCst)` を確認し、`true` なら approval channel を経由せず即承認。
- REPL の `/auto` コマンドハンドラがこの AtomicBool をトグルする。表示は `[auto] tool approval: on/off`。
- 起動時の値は CLI フラグ／設定ファイルから決定し、AtomicBool に格納。

#### shutdown 時の挙動

- 入力ループ shutdown 経路で `AwaitingApproval(resp_tx)` 状態のまま break する場合、`resp_tx.send(false)` で承認拒否を返してから `oneshot::Sender` を drop する（agent の `resp_rx.await` が `Err(_)` で即座に解消される）。
- agent 側は `unwrap_or(false)` で安全側倒し、その turn は denied 扱いとして終了。

### 4.4 ツール実行：シェルコマンド（`tools/shell.rs`）

- 入力スキーマ：`{ "cmd": "string", "cwd": "string?", "timeout_secs": "number?" }`
- 実装：`tokio::process::Command`で`bash -lc <cmd>`を起動。
- 出力：stdout・stderr・exit_codeを構造化して返す。
- 制限：`timeout_secs`（既定60秒）と`max_output_kb`（既定256KB）でガード。
- 承認：既定はy/N承認。`auto_approve_tools=true`時のみスキップ。

### 4.5 ツール実行：ファイル読み書き（`tools/fs_read.rs`、`tools/fs_write.rs`）

- read：パス、オプションでoffset／limit。バイナリは検出してエラーまたは要約。
- write：既定では既存ファイル上書きに警告し、`overwrite: true`で許可。

### 4.6 ツール実行：ピア送信（`tools/send_to.rs`）

- 入力スキーマ：`{ "peer": "string", "text": "string" }`
- `peer`はagent-idまたは表示名。レジストリから解決し、見つからなければエラー。
- IPCクライアントで`IpcMessage::Prompt`を送信。受信完了の`Ack`で成功とみなす。
- 結果として「送信成功／失敗」をAIへ返す（応答待ち合わせはしない）。

### 4.7 ピア検出（`ipc/registry.rs`）

- レジストリディレクトリ配下の`*.json`を列挙し、対応する`*.sock`の存在を確認。
- ソケットがない／PIDが死んでいるものは「stale」として除外、可能であれば掃除する。
- `agent-cli list`は整形して標準出力へ。

### 4.8 IPC受信（`ipc/server.rs`）

- listenしたソケットからの接続を受け、JSON Lines形式でメッセージをデシリアライズ。
- 受信メッセージは`mpsc`チャネルで会話ループへ流す。
- 不正フォーマットは`Error`応答を返し、接続を閉じる。
- shutdownチャネル（4.9）からの終了通知を`tokio::select!`で監視し、新規`accept`を停止して既存接続を閉じる。

### 4.9 終了処理（shutdown coordination）

ユーザー操作またはシグナルから終了要求を受領した際、以下の手順でプロセスを確実に終了する。本機構は FR-13（アプリ終了）の実装基盤である。

#### 終了トリガー

| トリガー | 検出方法 |
|----------|----------|
| `/quit` REPLコマンド | Dispatcherがshutdownチャネルへ通知。 |
| 標準入力EOF（`Ctrl+D`） | `BufReader::lines().next_line().await`が`Ok(None)`を返した時点でshutdownチャネルへ通知。 |
| SIGINT（`Ctrl+C`） | `tokio::signal::ctrl_c()`を別タスクで待ち受け、受信時にshutdownチャネルへ通知。 |
| SIGTERM | `tokio::signal::unix::signal(SignalKind::terminate())`を別タスクで待ち受け、受信時にshutdownチャネルへ通知。 |

#### 連携方法

- 共通の`tokio::sync::watch::Sender<bool>`／`Receiver<bool>`を起動時に1組生成し、入力ループ・IPCサーバー・会話ループ・各バックグラウンドタスクに`Receiver`をクローンして配布する。
- いずれかのトリガーが`Sender::send(true)`を実行すると、すべての`Receiver`が`true`を観測し、各タスクは自タスクのクリーンアップ後に終了する。
- 受信プロンプト処理中であっても、shutdown通知は最優先で処理する（`tokio::select!`の各armでshutdown監視を併走）。

#### クリーンアップ手順

1. 進行中のAIストリーム／ツール実行をキャンセル（`AgentInput::Cancel`相当を発行）。
2. IPCサーバーの`accept`ループを停止し、開いている接続をクローズ。
3. `<registry_dir>/<agent-id>.sock` と `<registry_dir>/<agent-id>.json` を削除。
4. ログハンドルを`flush`して閉じる。
5. プロセスを終了コード`0`で終了（致命的な異常時のみ非0）。

#### 実装上の注意

- `crossterm`を生入力モード（raw mode）で使用している場合、終了前に必ずraw modeを解除する（`crossterm::terminal::disable_raw_mode()`）。これを怠ると端末が壊れた状態で戻り、ユーザーが`reset`を要する。
- ステップ3はDropガード（`scopeguard`等）またはRAII的なオブジェクトに集約し、panic時にも確実に動くようにする。
- `tokio::main`からの戻り後にプロセスが即終了するよう、無限ループ・detachタスクを残さない。すべてのタスクは`JoinHandle`を保持し、shutdown後に`join`または`abort`する。
- 別ホストでのワンライナー導入検証（FR-09-2）で、`/quit`／`Ctrl+D`いずれでも終了しない事象が報告されているため、本節の手順を実装した上で単体テスト（擬似stdinのEOFで`App::run`が完了する／`/quit`コマンドで`App::run`が完了する／終了後にソケット・メタが残らない）を追加する。

## 5. エラーハンドリング方針

- すべてのfallible関数は`Result<T, AppError>`を返す。
- `AppError`は`thiserror`で定義し、`Config`／`Provider`／`Tool`／`Ipc`／`Registry`／`Ui`等のvariantを持つ。
- ユーザー向けには簡潔なメッセージ、詳細は`tracing`のデバッグログに残す。

### 5.1 プロバイダエラーの診断情報付与（FR-09-3）

`ai/<backend>.rs` の `complete_stream` 実装で HTTP 4xx／5xx を受領した場合、`AppError::Provider` に診断情報を埋め込み、REPL／`doctor`／`selftest` 表示時に併記する。具体的な構造と挙動：

- `ProviderError` 構造体（`error.rs` または `ai/mod.rs`）：
  - `provider: &'static str`（`"claude"`／`"codex"`／`"ollama"`／`"llama.cpp"`）
  - `status: u16`（HTTPステータス）
  - `body: String`（応答本文。長尺なら先頭〜中盤を保持）
  - `request_id: Option<String>`（応答ヘッダー `request-id`／`x-request-id` または本文中の `request_id` フィールドから抽出）
  - `config_path: PathBuf`（解決済み設定ファイルパス）
  - `api_key_env: String`（当該バックエンドの `api_key_env`）
  - `api_key_mask: Option<String>`（環境変数値の先頭4文字＋`...`＋末尾4文字。空文字／未設定は `None`）
  - `hint: Option<String>`（特定パターン検出時の対処ヒント）
- パターン検出（`hint` の生成）：
  - Anthropic `invalid_request_error` ＋ 本文に `credit balance is too low`：「Anthropic アカウントのクレジット残高が不足しています。https://console.anthropic.com/settings/billing で確認・購入するか、別アカウントの API キーを `<api_key_env>` に設定してください。」
  - `invalid_api_key`／`authentication_error`／HTTP 401：「API キーが無効または失効しています。`<api_key_env>` の値を確認するか、Anthropic Console から再発行してください。」
  - HTTP 429／`rate_limit_error`：「レート制限に達しました。数分待つか、より低頻度な使用に切り替えてください。」
- 表示フォーマット（REPL の `[error]` 出力を例示）：

  ```
  [error] provider error (claude): HTTP 400 Bad Request
    request_id : req_011Caej2JtMYvLF9GMAfUuAf
    config     : /home/hidemi/.local/config/agent-cli/config.toml
    api_key_env: ANTHROPIC_API_KEY (sk-a...x9k2)
    detail     : {"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API. ..."}}
    hint       : Anthropic アカウントのクレジット残高が不足しています。https://console.anthropic.com/settings/billing で確認・購入するか、別アカウントの API キーを ANTHROPIC_API_KEY に設定してください。
  ```

- `doctor` の Claude／Codex 疎通ステップは、現状の「到達確認」に加え、本診断情報を表示できる経路を持つ。`tracing::debug` ではフルレスポンスを残しつつ、ユーザー向けには上記のサマリ形式で出力する。
- セキュリティ：API キーの全文は決して出力しない。マスク表示は先頭4文字＋末尾4文字に限定し、それ以外（中央部・全文）はログにも残さない。
- 2026-05-03 報告事象（`.aiprj/instructions.md`）：`/home/hidemi/.local/config/agent-cli/config.toml` という非XDG標準パスの設定ファイルを使用しているため、`agent-cli config path` で「実際にどの設定ファイルが解決されているか」を確認できる経路を README／`doc/troubleshooting.md` でも案内する。

## 6. 依存ライブラリ（想定）

| 用途 | クレート |
|------|---------|
| CLI解析 | `clap`（derive） |
| 非同期ランタイム | `tokio` |
| HTTPクライアント | `reqwest` |
| シリアライズ | `serde`、`serde_json`、`toml` |
| 日時 | `chrono` |
| エラー | `thiserror`、`anyhow` |
| 端末入出力 | `crossterm` |
| ログ | `tracing`、`tracing-subscriber` |
| 非同期trait | `async-trait` |
| 一意ID | `ulid` |
| YAMLフロントマター | `serde_yaml`または`gray_matter` |

## 7. セキュリティ・安全設計

- APIキーはプロセス環境変数から取得し、設定ファイルへ平文保存しない。
- IPCソケットは0600で作成し、所有者のみがアクセス可能とする。
- 外部公開ポートを開かない。通信はローカルUnixドメインソケットに限定。
- ツール実行は既定で承認を求める。`auto_approve_tools=true`設定時のみスキップ。
- 受信プロンプト（PeerPrompt）は送信元IDを会話履歴に明記し、追跡可能とする。

## 8. テスト・検証方針

### 8.1 自動テスト

- 単体テスト：設定読み書き、`AgentId`、ツールレジストリ、IPCシリアライズ、レジストリ走査、Provider別のレスポンスパーサ（モックHTTPで各バックエンドのストリーム形式を検証）。
- 結合テスト：
  - モックProviderで会話ループ。
  - テンポラリ`registry_dir`でIPC往復（2プロセス相当のテストハーネス）。
  - シェルツール実行（`echo`／タイムアウト／出力サイズ超過）。
  - REPL終了経路（4.9）：擬似stdinのEOFで`App::run`が完了し、`/quit`入力で`App::run`が完了し、いずれの場合も`<registry_dir>/<agent-id>.sock`／`.json`が削除されていること。
  - REPL入出力サイクル（4.2A／FR-03-2）：擬似stdinから 2 行のユーザー入力を順に投入し、それぞれが `AgentInput::UserPrompt` として agent ループに到達し、各 `Done` の後に次のプロンプトが受理されることを単体テストで保証する。
  - ツール承認入出力統合（4.3A／FR-04-1）：擬似 `ApprovalRequest` を入力ループへ送信し、続けて擬似 stdin に "y\n" を投入すると `oneshot` 応答が `true` で帰ること、`n\n` の場合は `false` で帰ること、また AwaitingApproval 中はユーザー入力が `AgentInput::UserPrompt` として agent ループへ流出しないことを単体テストで保証する。
- CI想定：`cargo fmt --check`／`cargo clippy -- -D warnings`／`cargo test`をすべて通過させる。

### 8.2 自己診断（`agent-cli doctor`）

点検項目：

1. 設定ファイルの存在・パース可否。
2. 選択中バックエンドに必要な環境変数（APIキー）の存在。
3. バックエンドへの疎通（HTTPのHEADまたは軽量GET、Anthropicは認証エラー応答で到達確認）。
4. レジストリディレクトリの作成・書き込み可否。
5. シェルツールに用いる`bash`の存在と起動可否。
6. ログ出力先ディレクトリの書き込み可否。

各項目はOK／NGを表示し、NG時は対処方法のヒントを併記する。終了コードは全OKで0、NGがあれば非0。

### 8.3 スモークテスト（`agent-cli selftest`）

決定性を確保するため 3 ステージ構成で実行する。各ステージは独立に成否を表示し、いずれか失敗時に全体 FAIL（終了コード非0）。

#### Stage 1：Provider 往復

- `--provider`で指定されたバックエンド（未指定時は`provider.kind`）で Provider を構築。
- 短いプロンプト（例：「Reply with the literal text OK.」）を投げ、応答に`OK`が含まれることを確認。
- 60 秒のタイムアウトで保護。

#### Stage 2：シェルツール実行

- ツール実行系統の決定性検証として、`ToolRegistry`から`shell`ツールを取り出して直接`echo selftest`を実行する（AI 経由の tool_use ループには依存しない）。
- 標準出力に`selftest`が含まれること、`exit_code=0`であることを検証。

#### Stage 3：IPC ラウンドトリップ

- 一時ディレクトリに`IpcServer::bind`で自プロセス内ソケットを起動し、`client::send(Ping)`で`Pong`応答が得られることを検証。
- 外部プロセスを起動せず、IPC レイヤー単体の健全性を確認。

#### Stage 4：子プロセス起動と IPC 検証

- `std::env::current_exe()`で自バイナリのパスを取得し、隔離した一時設定（外部到達不可な`base_url`、`tools.enabled = []`）で`agent-cli run --name selftest-child --auto-approve-tools`を子プロセスとして起動。
- 最大 5 秒以内に子のレジストリエントリ（`<registry_dir>/<agent-id>.json`）が出現することを確認。
- 子のソケットへ `client::send(Ping)` を投げ、`Pong` を受け取ることを確認。
- 続けて `client::send(Prompt{from, text:"selftest-prompt"})` を投げ、`Ack` 応答が返ることを確認。これにより、T-601-C「プロセス間メッセージ授受」の IPC 層（送信→ Ack）を CI で自動検証する。
- 子の標準入力を閉じることで REPL を終了させ、最大 3 秒で正常終了することを確認（残っていれば `kill`）。
- これにより「プロセス起動 → レジストリ登録 → IPC バインド → クロスプロセス IPC（Ping/Pong および Prompt/Ack） → 終了処理」の一連が検証される。
- AI 応答の生成までは検証対象外（Stage 5 で扱う）。

#### Stage 5：子プロセスの AI 応答（peer prompt + AI response）

- Stage 1（Provider 往復）が成功した場合のみ実行する。失敗時は `SKIP`。
- 親プロセスの `Config` をベースに、`registry_dir`／`log_dir`／`agents_dir` のみテンポラリへ差し替えた子用設定を生成（provider 設定や API キー env はそのまま）。
- 子プロセスを起動 → レジストリ登録待機（最大 10 秒）。
- 子のソケットへ`Prompt{ text: "Reply with a single word: HELLO" }`を送信し`Ack`を受領。
- 子の会話ログ（`<log_dir>/<agent-id>/*.jsonl`）を 90 秒以内ポーリングし、`{"kind":"assistant","text":"..."}`の出現を検出。検出時はその text を画面に表示。
- 子の標準入力を閉じて REPL を終了させ、最大 5 秒で正常終了を待ち、残れば`kill`。
- これにより「ピアからのプロンプト受信 → AI 呼び出し → text 応答生成 → ログ書き込み」が CI レベルで自動検証される。T-601-C「AI 応答部分」もこの Stage で代理検証される（実機 ollama などが利用できる環境において）。

#### 出力例

```text
[selftest] stage 1 (provider OK round-trip)
[selftest]   provider: claude model=claude-opus-4-7
[selftest]   response: OK
[selftest]   stage 1 ok
[selftest] stage 2 (tool execution: shell)
[selftest]   stage 2 ok (shell tool executed)
[selftest] stage 3 (IPC round-trip)
[selftest]   stage 3 ok (Ping/Pong)
[selftest] stage 4 (subprocess registration + IPC)
[selftest]   stage 4 ok (subprocess agent-01HX... registered, Ping/Pong + Prompt/Ack)
[selftest] stage 5 (subprocess peer prompt + AI response)
[selftest]   stage 5 ok (peer responded: "HELLO")
[selftest] result  : OK
```

### 8.4 手動受け入れ（必須対象：`claude`、`ollama` (glm-5.1:cloud)）

完成判定の手動受け入れは、以下の2バックエンドで必須とする。`codex`／`llama.cpp`は任意検証。

#### 必須シナリオ A：claude単独

- `agent-cli --config <claude.toml> run --provider claude`で起動し対話できる。
- `agent-cli doctor`／`agent-cli selftest --provider claude`がそれぞれ終了コード0。
- シェルツール経由で`ls`／`echo`等のコマンドが実行できる。

#### 必須シナリオ B：ollama単独（glm-5.1:cloud）

- `agent-cli --config <ollama.toml> run --provider ollama --model glm-5.1:cloud`で起動し対話できる。
- `agent-cli doctor`／`agent-cli selftest --provider ollama`がそれぞれ終了コード0。
- シェルツール経由のコマンド実行ができる（モデルがtool_use非対応の場合は擬似実装または手動指示で代替可とし、その挙動を記録する）。

#### 必須シナリオ C：claude × ollama 異種2プロセス協調

- ターミナルAで`agent-cli run --provider claude --name alice`を起動。
- ターミナルBで`agent-cli run --provider ollama --model glm-5.1:cloud --name bob`を起動。
- 両プロセスは同一の`registry_dir`を共有する設定で起動する。
- Aで`/list`にalice／bob双方が表示される。
- Aから`/send bob "hello"`を実行 → Bが受信してollama応答が観測できる。
- 逆方向（Bから`/send alice ...`）も成立する。

#### 任意シナリオ

- `--provider codex`、`--provider llama.cpp`での起動・対話・selftestは任意。実施した場合は結果を作業ログに残す。

#### 検証結果の記録

- 検証実行時のバックエンド・モデル名・コミットハッシュ・実行日時・各シナリオの合否を作業ログ（`.aiprj/AI_LOG/YYYY-MM-DD_NNN.md`）に記録する。

#### 半自動実行スクリプト

- 必須シナリオA／B、任意シナリオD1（codex）／D2（llamacpp）を `scripts/manual_acceptance.sh` で半自動実行できる。
- 環境変数 `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `OLLAMA_URL` / `LLAMACPP_URL` の有無に応じて自動 SKIP。
- scenario C は手動操作部分のため、手順を表示して PASS 扱い（実際の対話は人間が実施）。
- 終了コード：FAIL があれば 1、それ以外（PASS と SKIP のみ）は 0。

## 9. ペルソナ機構の補足

### ペルソナファイルの解決

```
1. --persona <path>           （明示指定。未存在ならエラー終了）
2. [runtime] persona_file     （設定ファイル指定。未存在ならエラー終了）
3. <agents_dir>/<name>.md     （--name に対応する規定パス。未存在は組み込みへフォールバック）
4. 組み込み既定ペルソナ
```

`agents_dir`の既定は`~/.config/agent-cli/agents/`。`[runtime] agents_dir`で変更可能。

### `agent-cli list`の出力例（ペルソナ反映後）

```
ID                       NAME    PROVIDER  MODEL              ROLE             SKILLS
agent-01HXABCDEF...      alice   claude    claude-opus-4-7    コードレビュアー   Rust, 静的解析
agent-01HXGHIJKL...      bob     ollama    glm-5.1:cloud      プランナー        計画立案, 要件分析
```

### `/peer <id>`の出力例

```
[bob] role=プランナー, skills=[計画立案, 要件分析]
description: 大規模タスクをサブタスクに分解して進捗を管理する
```

### ペルソナとツール権限

- `allowed_tools`が指定されている場合は、そのリストに含まれるツールのみ有効化（ホワイトリスト）。
- 未指定で`denied_tools`がある場合は、設定の`tools.enabled`からブラックリスト除外。
- 両者未指定なら`tools.enabled`をそのまま使用。
- `send_to`を`denied_tools`に含めれば、当該エージェントはピアへメッセージを送れない（受信は可）。

## 10. 対象OSとtmuxとの関係

### 10.1 対象OS

- 対象はLinuxのみ。macOSおよびWindowsはサポートしない。
- 想定アーキテクチャはx86_64およびaarch64。
- 配布物はLinux向け単一バイナリとする（必要に応じてmusl静的リンクのビルドも検討）。
- Linux固有機能（Unixドメインソケット、`XDG_RUNTIME_DIR`、`/proc`によるPID生存確認など）を前提として実装してよい。

### 10.2 tmuxとの関係

- 本アプリはtmux依存を持たない。tmux内・tmux外いずれでも動作する。
- ユーザーは複数の`agent-cli`プロセスを別々の端末（または別ペイン）で起動して協調させてもよいが、tmuxは必須要件ではない。

## 10A. インストールスクリプト（`install.sh`）

リポジトリ直下に`install.sh`を配置し、`curl ... | sh`によるワンライナー導入を可能にする。

### 10A.1 仕様

- 言語：POSIX `sh`互換（`bash`専用構文を避ける）
- 対象：Linux（x86_64／aarch64）。それ以外は`uname -s`／`uname -m`で検出してエラー終了
- 取得方法：
  - 実行カレントが`agent-cli`リポジトリ内（`Cargo.toml`にパッケージ`agent-cli`を含む）であれば、ローカルソースから`cargo install --path .`でビルド
  - そうでなければ環境変数`AGENT_CLI_REPO`（既定`https://github.com/aquaxis/agent-cli.git`）と`AGENT_CLI_REF`（既定`main`）から一時ディレクトリへ`git clone`し、`cargo install --path . --root $AGENT_CLI_PREFIX`を実行
- インストール先：
  - `AGENT_CLI_PREFIX`既定：`$HOME/.local`
  - 実バイナリは`$AGENT_CLI_PREFIX/bin/agent-cli`に配置
  - インストール後、`PATH`に当該`bin`が含まれるかを点検し、含まれない場合は警告
- 依存：`cargo`／`git`／`uname`／`mktemp`／`mkdir`／`rm`
  - `cargo`／`git`が無い場合は、`rustup`／`apt`等の導入手順を提示して終了
- 出力：実行した手順を逐次標準出力へ表示
- 終了コード：成功`0`、失敗時は非0

### 10A.2 環境変数

| 変数 | 既定値 | 説明 |
|------|--------|------|
| `AGENT_CLI_REPO` | `https://github.com/aquaxis/agent-cli.git` | 取得元リポジトリ |
| `AGENT_CLI_REF` | `main` | チェックアウトする ref（branch／tag／commit） |
| `AGENT_CLI_PREFIX` | `$HOME/.local` | インストール先プレフィックス |
| `AGENT_CLI_INSTALL_FORCE` | （未設定） | `1`で既存バイナリを上書き |

### 10A.3 README記載

`README.md`の「インストール」セクションに、ワンライナー例とソースビルド例を併記する。

## 11. ドキュメント構成

リポジトリに配置するドキュメント類の構成は以下とする。

```
agent-cli/
├── README.md                    # 日本語版（既定言語）
├── README.ja.md                 # 日本語版（README.mdの別名／同一内容）
├── README.en.md                 # 英語版
├── CONTRIBUTING.md              # 開発参加ガイド
├── CHANGELOG.md                 # 変更履歴（Keep a Changelog／SemVer）
├── LICENSE                      # ライセンス全文
└── doc/
    ├── usage.md                 # コマンド／REPL／ユースケース
    ├── config.md                # 設定リファレンス（最も詳細）
    ├── architecture.md          # 構成と内部仕様の概要
    ├── tools.md                 # ツールリファレンス
    ├── troubleshooting.md       # トラブルシューティング
    └── providers/
        ├── claude.md
        ├── codex.md
        ├── ollama.md
        └── llamacpp.md
```

### 11.1 `doc/config.md`の章立て（規定）

設定方法を詳細に解説するため、以下の章立てを必須とする。

1. 設定ファイルの場所と解決順序（`--config` → `AGENT_CLI_CONFIG` → 既定パス）。
2. 全体構造の概要図と各セクションの役割。
3. 全項目リファレンス（`provider`／`provider.claude`／`provider.codex`／`provider.ollama`／`provider."llama.cpp"`／`runtime`／`tools`／`tools.shell`／`ui`）。各項目はキー・型・既定値・許容値・必須／任意・例・注意点を表で記載。
4. 完全サンプル：最小構成、推奨構成、全機能有効構成の3パターン。
5. APIキー・秘密情報の管理方法（環境変数、`api_key_env`、`.envrc`／`systemd EnvironmentFile`の例、平文保存の禁止）。
6. 複数プロファイル運用（プロジェクト別`--config`、`registry_dir`の分離／共有によるピア空間の制御）。
7. シェルツールのチューニング（`timeout_secs`／`max_output_kb`）。
8. UI表示モード（`ui.show_thinking`）。
9. よくある設定ミスと診断（`agent-cli doctor`の読み方、エラーメッセージ早見表）。
10. 設定変更の反映と再起動の要否。

### 11.2 `README.md`の章立て（規定）

1. プロジェクト概要・特徴。
2. 対応バックエンド早見表。
3. インストール（バイナリ取得／ソースビルド）。
4. クイックスタート（5分で動かす最短手順）。
5. **設定方法**：自動生成ファイルの場所、最低限編集する項目、コピペで動くサンプル、`--config`／`AGENT_CLI_CONFIG`の使い分け、複数プロファイル例。詳細は`doc/config.md`へリンク。
6. 主要コマンド早見表。
7. 検証（`cargo test`／`agent-cli doctor`／`agent-cli selftest`）。
8. ドキュメント目次（`doc/`配下へのリンク）。
9. ライセンス・コントリビューション。

### 11.3 ドキュメント運用

- 機能追加・変更・廃止と同じPRで関連ドキュメントを更新することを必須とする（CONTRIBUTING.mdに明記）。
- 公開API（`Provider`／`Tool`／`Config`等）にはrustdoc（`///`）を付与し、`cargo doc --no-deps`でブラウズ可能とする。
- 日本語ドキュメントはJTFスタイル（全角丸括弧、長音、全角／半角間スペース回避）に準拠する。
- サンプルコマンド・サンプル設定はCIで形式チェック（TOMLパース、コマンドヘルプ整合性）できるよう努める。

## 12. 将来拡張

- AIバックエンドの追加（vLLM、Gemini、Bedrock等）。
- TUIモード（ratatui）。
- 会話履歴の永続化と再開。
- ツールのプラグイン機構（動的ロード）。
- 同期型送信（`send_and_wait`）：応答が返るまで待機するピア通信。
- 同一プロセス内での複数バックエンド使い分け（現行設計は1プロセス1バックエンド）。
