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

REPL 上で `/persona` を打てば、現在適用中のロールとスキルが確認できます。

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
