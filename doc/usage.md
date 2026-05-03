# 使い方ドキュメント（`usage.md`）

## サブコマンド一覧

```text
agent-cli [--config <path>] <subcommand>
```

### グローバルオプション

| オプション | 説明 |
|-----------|------|
| `--config <path>` | 使用する設定ファイル。`AGENT_CLI_CONFIG` 環境変数も同等に使える |

### サブコマンド

| 形式 | 用途 |
|------|------|
| `agent-cli run [...]` | REPL を起動（既定） |
| `agent-cli list` | 稼働中のピア一覧 |
| `agent-cli send <peer> <text>` | 指定ピアにプロンプト送信して終了 |
| `agent-cli providers` | 利用可能バックエンドの状態 |
| `agent-cli doctor` | 設定／API キー／疎通／レジストリ／bash の点検 |
| `agent-cli selftest [--provider <name>]` | スモークテスト |
| `agent-cli config show` | 現在の設定を出力 |
| `agent-cli config edit` | `$EDITOR` で設定を開く |
| `agent-cli config path` | 解決済み設定パスを出力 |

### `run` サブコマンドの引数

| オプション | 説明 |
|-----------|------|
| `--name <name>` | エージェント表示名 |
| `--provider <kind>` | バックエンドを上書き |
| `--model <model>` | モデルを上書き |
| `--persona <path>` | ペルソナファイルを明示指定 |
| `--auto-approve-tools` | ツール実行の y/N 承認をスキップ |

## REPL コマンド

REPL では `/` から始まる行はコマンド、それ以外はアクティブエージェントへの通常プロンプトです。

| コマンド | 用途 |
|---------|------|
| `/list` | ピア一覧（id・name・provider・model・role） |
| `/send <peer> <text>` | ピアへプロンプト送信 |
| `/tools` | このエージェントで有効なツール名一覧 |
| `/persona` | 自身のペルソナ（role／skills／description／tool 制限／ソースパス） |
| `/reload-persona` | ペルソナファイルを再読込し、システムプロンプトを更新（履歴は保持） |
| `/peer <id_or_name>` | 指定ピアのペルソナ概要を表示 |
| `/history [n]` | 過去 n 件（既定 20）のユーザー入力を表示 |
| `/clear`、`/reset` | 会話履歴を初期化（システムプロンプト＝ペルソナは保持、User／Assistant／ToolResult を全消去） |
| `/cancel` | 進行中の処理を中断（要求のみ。実行中のストリームを即時停止する保証はない） |
| `/auto [on\|off\|status]` | ツール承認スキップの実行時切替。引数なし／`status` で現在値を表示 |
| `/help` | コマンド一覧 |
| `/quit` / `/exit` | アプリ終了 |

### ツール実行承認のスキップ

シェル等のツール呼び出しは既定で y/N 承認を求めます。これをスキップする経路は次の 3 つです（任意の組み合わせで可）：

| 経路 | 例 | 反映タイミング |
|------|-----|----------------|
| 設定ファイル | `[runtime] auto_approve_tools = true` | `agent-cli` 起動時 |
| CLI フラグ | `agent-cli run --auto-approve-tools` | 起動時のみ（一時的に上書き） |
| REPL コマンド | `/auto on` | 即時。`/auto off` で再度承認モードへ戻せる |

`/auto status`（または引数なしの `/auto`）で現在値を確認できます。承認モードのままでツール要求を受けると `[tool approval] <tool> <args>` バナーと `approve? [y/N]: ` が表示されます。`y`／`yes` のみ承認、それ以外（空文字や別単語）は拒否扱いです。

### `[thinking]` 表示の抑制

Claude の `thinking_delta` や Ollama の `message.thinking`（`glm-5.1:cloud` 等）は `AgentEvent::Thinking` として REPL に渡り、`[thinking] <text>` 行として描画されます。長尺 reasoning モデルでは大量に流れるため、`[ui] show_thinking` で 3 段階に制御できます：

| 値 | 挙動 |
|----|------|
| `"hidden"` | `[thinking]` 行を一切表示しない |
| `"collapsed"`（既定） | 各 delta を「先頭 80 文字 + `...`」に切り詰めた 1 行で表示 |
| `"expanded"` | 受信 text を全文表示 |

未知値（`"verbose"` など）は `"collapsed"` にフォールバックします。設定変更は `agent-cli` 再起動で反映され、実行時切替は提供されません。詳細は [`doc/config.md`](config.md) の「UI 表示モード」節を参照。

### REPL 出力の `[info]` メッセージ

REPL は `AgentEvent` のうち `Info` バリアントを `[info]` プレフィックスで描画します。`Info` は補助情報・状態通知であり、エラーではありません（エラーは `[error]` プレフィックスで区別）。代表的なメッセージ：

| メッセージ | 発生条件 | 直後の挙動 |
|------------|----------|------------|
| `[info] cancel requested` | `/cancel` を入力 | 進行中の処理に中断要求を送る（即時停止保証なし） |
| `[info] history persisted (N entries)` | `/history` などの履歴保存トリガー | 入力履歴ファイルへの flush 完了 |
| `[info] system prompt updated` | `/reload-persona` で履歴先頭を差し替え | 以降の応答に新しいシステムプロンプトが反映 |
| `[info] history cleared (N message(s) removed)` | `/clear`／`/reset` で会話履歴を初期化 | システムプロンプト（ペルソナ）は保持、User／Assistant／ToolResult を全消去 |
| `[info] max tool-use iterations reached` | tool_use 反復が `[runtime] max_tool_iterations`（既定 24）に到達（FR-04-3／設計書 4.3B） | 当該ターンを `Done` で終了、次のユーザー入力プロンプトを再描画 |

`[info] max tool-use iterations reached` は AI がツール実行を `max_tool_iterations` 反復に渡って繰り返しても結論に達しなかった場合の防護機構です（`agent.rs::process_turn` の `self.config.runtime.max_tool_iterations.max(1)`）。上限値は設定ファイルで変更可能（既定 24、最小 1、最大 `u32::MAX`）。意味と対処は `doc/troubleshooting.md` の「`[info] max tool-use iterations reached` と表示される」節、設定値のチューニングは `doc/config.md` の `[runtime]` 節を参照してください。

## ユースケース

### 1. 単独対話（claude）

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
agent-cli run --provider claude
```

### 2. ローカル LLM（ollama）

```bash
ollama serve &
agent-cli run --provider ollama --model glm-5.1:cloud
```

### 3. 2 プロセス協調（claude × ollama）

```toml
# 双方の設定で registry_dir を共有
[runtime]
registry_dir = "/tmp/agent-cli/team"
```

```bash
# ターミナル A
agent-cli run --provider claude --name alice

# ターミナル B
agent-cli run --provider ollama --model glm-5.1:cloud --name bob
```

A 側：

```text
> /list
agent-01HX...    alice    claude    claude-opus-4-7    汎用アシスタント
agent-01HY...    bob      ollama    glm-5.1:cloud      汎用アシスタント

> /send bob "B 視点で1行レビューして"
delivered to agent-01HY...
```

B 側に `[peer prompt from agent-01HX...]` が表示され、AI が応答します。

### 4. 役割分担（ペルソナ運用）

```bash
cp example/agents/reviewer.md ~/.config/agent-cli/agents/alice.md
cp example/agents/coder.md    ~/.config/agent-cli/agents/bob.md

# それぞれ別端末で
agent-cli run --name alice    # reviewer ペルソナが自動適用
agent-cli run --name bob      # coder ペルソナが自動適用
```

REPL 上で `/persona` を打てば、現在適用中のロールとスキルが確認できます。フロントマターの全キー（`role`／`skills`／`allowed_tools`／`denied_tools`／`model`／`temperature` 等）と運用パターンの詳細は [`doc/personas.md`](personas.md) を参照。

### 5. CLI からのワンショット送信

REPL を立ち上げず、別エージェントへ短いメッセージだけ届けたい場合：

```bash
agent-cli send alice "stand-by"
```

これは IPC クライアントとしてだけ動き、すぐに終了します。受信側エージェントは継続的に応答します。

### 6. 設定切り替え

```bash
agent-cli --config ./project-a.toml run --name proj-a
agent-cli --config ./project-b.toml run --name proj-b
```

`registry_dir` を別にすればまったく独立した世界として動きます。

## 入力履歴

ユーザー入力（`/` で始まらない通常プロンプト）は、`<runtime.log_dir>/history.txt` に 1 行 1 件で永続化されます。次回起動時に読み込まれ、`/history [n]` で確認できます。

- 既定 `runtime.log_dir = "~/.local/share/agent-cli/logs"` の場合、履歴は `~/.local/share/agent-cli/logs/history.txt`。
- 上限は最終 200 件（メモリ上）。ファイル側は append-only。
- 機微情報を入力した際は手動で削除してください。

## 会話履歴のリセット（`/clear`）

LLM へ毎ターン送られる会話文脈（System／User／Assistant／ToolResult）を初期化したい場合は `/clear`（または別名 `/reset`）を実行します。

- システムプロンプト（ペルソナ由来）は保持されます。User／Assistant／ToolResult のみ全消去。
- 直後の Info 出力に削除件数が表示されます：`[info] conversation history cleared (N message(s) removed; persona retained)`。
- ペルソナ自体を差し替えたい場合は `/reload-persona` を併用してください。
- `/clear` は当該プロセス内のメモリ上履歴のみ操作します。`<log_dir>/<agent-id>/<timestamp>.jsonl` の会話ログファイルは削除しません（後から見返せます）。
