# 設定リファレンス（`config.md`）

`agent-cli` の設定方法を網羅的に解説します。クイックリファレンスは `README.md` を、起動オプションの詳細は `doc/usage.md` を参照してください。

## 目次

1. [設定ファイルの場所と解決順序](#1-設定ファイルの場所と解決順序)
2. [全体構造とセクションの役割](#2-全体構造とセクションの役割)
3. [全項目リファレンス](#3-全項目リファレンス)
4. [完全サンプル](#4-完全サンプル)
5. [APIキー・秘密情報の管理](#5-apiキー秘密情報の管理)
6. [複数プロファイル運用](#6-複数プロファイル運用)
7. [シェルツールのチューニング](#7-シェルツールのチューニング)
8. [UI 表示モード](#8-ui-表示モード)
9. [よくある設定ミスと診断](#9-よくある設定ミスと診断)
10. [設定変更の反映と再起動](#10-設定変更の反映と再起動)

## 1. 設定ファイルの場所と解決順序

`agent-cli` は以下の優先順位で設定ファイルパスを解決します。

```text
1. --config <path>             ←最優先（明示指定）
2. 環境変数 AGENT_CLI_CONFIG   ←次点
3. ~/.config/agent-cli/config.toml ←既定
```

挙動：

- 1 または 2 で指定されたファイルが**存在しない場合はエラー終了**します。自動生成は行いません。
- 3 が使われる場合、ファイルが存在しなければ既定値で **自動生成** します。
- 解決済みパスは `agent-cli config path` で確認できます。

```bash
agent-cli config path
# 例: /home/alice/.config/agent-cli/config.toml

agent-cli --config ./project-a.toml config path
# 例: /home/alice/work/project-a.toml
```

## 2. 全体構造とセクションの役割

```toml
[provider]                  # どのバックエンドを使うか
[provider.claude]           # claude バックエンド固有設定
[provider.codex]            # codex (OpenAI) バックエンド固有設定
[provider.ollama]           # ollama バックエンド固有設定
[provider."llama.cpp"]      # llama.cpp サーバー固有設定（キーはクオート必須）

[runtime]                   # 実行時の挙動・パス
[tools]                     # ツール全体設定
[tools.shell]               # shell ツールのチューニング

[ui]                        # 表示モード
```

## 3. 全項目リファレンス

### `[provider]`

| キー | 型 | 既定 | 必須 | 説明 |
|------|----|------|------|------|
| `kind` | string | `"claude"` | ✓ | 使用バックエンド：`"claude"`／`"codex"`／`"ollama"`／`"llama.cpp"` |

### `[provider.claude]`／`[provider.codex]`／`[provider.ollama]`／`[provider."llama.cpp"]`

| キー | 型 | 既定 | 必須 | 説明 |
|------|----|------|------|------|
| `model` | string | バックエンド毎の既定 | ◯ | 使用するモデル名 |
| `api_key_env` | string | 各バックエンドの既定 | ◯ | API キーを保持する環境変数名（値そのものではない） |
| `base_url` | string | 各バックエンドの既定 | △ | エンドポイント。プロキシや互換サーバー利用時に上書き |
| `thinking` | bool | `true`（claude のみ意味あり） | △ | thinking ブロックを有効化（`claude` のみ） |

バックエンド毎の既定：

| kind | model 既定 | base_url 既定 | api_key_env 既定 |
|------|-----------|---------------|-------------------|
| claude | `claude-opus-4-7` | `https://api.anthropic.com` | `ANTHROPIC_API_KEY` |
| codex | `gpt-4.1` | `https://api.openai.com/v1` | `OPENAI_API_KEY` |
| ollama | `glm-5.1:cloud` | `http://127.0.0.1:11434` | （不要） |
| llama.cpp | `default` | `http://127.0.0.1:8080` | （任意） |

### `[runtime]`

| キー | 型 | 既定 | 説明 |
|------|----|------|------|
| `auto_approve_tools` | bool | `false` | `true` ならツール実行時の y/N 承認をスキップ。実行時は REPL コマンド `/auto on`／`/auto off`／`/auto status` で同じスイッチを切替可能 |
| `log_dir` | string | `~/.local/share/agent-cli/logs` | 会話ログの保存先 |
| `registry_dir` | string | 空 | エージェントレジストリの場所。空時は `$XDG_RUNTIME_DIR/agent-cli` または `/tmp/agent-cli` を使用 |
| `agents_dir` | string | `~/.config/agent-cli/agents` | ペルソナファイルの探索先（`<agents_dir>/<name>.md`）。詳細は [`doc/personas.md`](personas.md) |
| `persona_file` | string | 空 | 明示指定するペルソナファイル。空時は `<agents_dir>/<name>.md` または組み込みへフォールバック。詳細は [`doc/personas.md`](personas.md) |
| `max_tool_iterations` | u32 | `24` | 1 ターン内の tool_use 反復上限。最小 1（`0`／負値は内部で `1` へ丸め込み）、最大 `u32::MAX = 4,294,967,295`。無限ループ防止の防護機構。詳細は下記「`max_tool_iterations` のチューニング」を参照 |

#### `max_tool_iterations` のチューニング

AI が 1 回のユーザー入力に対して `tool_use → ツール結果 → tool_use → …` を繰り返すループの上限です。上限到達時は REPL に `[info] max tool-use iterations reached` を表示し、当該ターンを `Done` で終了します（エラーではなく情報通知）。

**Q&A：**

| 質問 | 回答 |
|------|------|
| 設定ファイルで変更できますか？ | はい。`[runtime] max_tool_iterations` を編集して `agent-cli` を再起動。実行中の REPL では動的には変わりません。 |
| 無制限の設定は可能ですか？ | いいえ（厳密には）。型 `u32` のため最大 `u32::MAX = 4,294,967,295` 回まで設定可能（実用上は無制限相当）。「真の無上限ループ」は API 課金・GPU 占有・stdout 占有の暴走防止のため意図的に提供しません。事実上の無制限が必要なら `max_tool_iterations = 4294967295` を指定してください。 |

**境界値挙動：**

- `0` または負値：実装側で `.max(1)` により `1` 反復として動作。
- `1` 〜 `u32::MAX`：そのまま採用。
- `u32::MAX` 超：TOML パース時にオーバーフローエラーで起動失敗。

**推奨レンジ（用途別）：**

| 用途 | 推奨値 | 根拠 |
|------|--------|------|
| 単純対話・教育用 | `4-8` | ループ暴走時に早く打ち切れる |
| 既定（design-then-debug 等） | `24`（既定値） | 設計成果物生成 → 検証 → lint 修正 → fs_write の典型ワークフローが収まる |
| 多段オーケストレーター | `32-48` | 複数ツールを順次呼ぶ場合 |
| 長尺自律実行（実験用途） | `64-256` | 大規模タスクを段階的に分解する場合 |
| それ以上 | 非推奨 | 「ループに陥っていないか」を疑うべき範囲。`/cancel`／`Ctrl+C` で介入できる前提で運用 |

設定例：

```toml
[runtime]
max_tool_iterations = 48   # 多段オーケストレーター用途
```

### `[tools]`

| キー | 型 | 既定 | 説明 |
|------|----|------|------|
| `enabled` | string[] | `["shell","fs_read","fs_write","send_to"]` | 有効化するツール |

ペルソナの `allowed_tools` / `denied_tools` がある場合、本リストとの **積／差** が最終的なツールセットになります。

### `[tools.shell]`

| キー | 型 | 既定 | 説明 |
|------|----|------|------|
| `timeout_secs` | int | `60` | 1 コマンド当たりのタイムアウト（秒） |
| `max_output_kb` | int | `256` | stdout／stderr の最大保持サイズ（KB） |

### `[ui]`

| キー | 型 | 既定 | 説明 |
|------|----|------|------|
| `show_thinking` | string | `"collapsed"` | thinking 表示モード：`"collapsed"`（先頭 80 文字 + 1 行目のみに切り詰め）／`"expanded"`（全文）／`"hidden"`（非表示）。詳細は下記「UI 表示モード」 |

## 4. 完全サンプル

### 4.1 最小構成（claude）

```toml
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
```

### 4.2 推奨構成（claude を主、ollama を副に検証用で確保）

```toml
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
model       = "claude-opus-4-7"
thinking    = true

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"

[runtime]
auto_approve_tools = false
log_dir            = "~/.local/share/agent-cli/logs"

[tools]
enabled = ["shell", "fs_read", "fs_write", "send_to"]

[tools.shell]
timeout_secs  = 120
max_output_kb = 512

[ui]
show_thinking = "collapsed"
```

### 4.3 全機能有効構成

```toml
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
model       = "claude-opus-4-7"
base_url    = "https://api.anthropic.com"
thinking    = true

[provider.codex]
api_key_env = "OPENAI_API_KEY"
model       = "gpt-4.1"
base_url    = "https://api.openai.com/v1"

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"

[provider."llama.cpp"]
model    = "default"
base_url = "http://127.0.0.1:8080"

[runtime]
auto_approve_tools  = false
log_dir             = "~/.local/share/agent-cli/logs"
registry_dir        = "/tmp/agent-cli"
agents_dir          = "~/.config/agent-cli/agents"
persona_file        = ""
max_tool_iterations = 48                            # 多段オーケストレーター想定

[tools]
enabled = ["shell", "fs_read", "fs_write", "send_to"]

[tools.shell]
timeout_secs  = 60
max_output_kb = 256

[ui]
show_thinking = "expanded"
```

## 5. APIキー・秘密情報の管理

`agent-cli` は **API キーの値を設定ファイルに書きません**。`api_key_env` で **環境変数名** を指定し、実際の値はその環境変数から取得します。

### 5.1 シェルでの設定

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
agent-cli run
```

### 5.2 `direnv` の `.envrc`

プロジェクトディレクトリ専用の値を切り替える例：

```bash
# .envrc
export ANTHROPIC_API_KEY="$(pass anthropic/api_key)"
export AGENT_CLI_CONFIG="$PWD/agent-cli.toml"
```

### 5.3 `systemd EnvironmentFile`

```ini
# ~/.config/systemd/user/agent-cli@.service
[Service]
Type=simple
EnvironmentFile=%h/.config/agent-cli/secrets.env
ExecStart=%h/.local/bin/agent-cli run --name %i
```

`secrets.env` を `chmod 600` にして API キーを保管します。

### 5.4 注意事項

- 平文でリポジトリに **コミットしない**。`.gitignore` で `.env`／`.envrc`／`secrets.*` を除外してください。
- `agent-cli config show` は値を環境変数名（`api_key_env`）として出力するため、API キー自体は流出しません。

## 6. 複数プロファイル運用

`--config` を使ってプロジェクト毎・用途毎に設定を切り替えられます。

```bash
# claude プロファイル
agent-cli --config ~/profiles/claude.toml run --name alice

# ollama プロファイル
agent-cli --config ~/profiles/ollama.toml run --name bob
```

### 6.1 別エージェントとして独立させる

`registry_dir` を別々に設定すれば、互いに `/list` で見えない独立した名前空間になります。

```toml
# claude.toml
[runtime]
registry_dir = "/tmp/agent-cli/claude"

# ollama.toml
[runtime]
registry_dir = "/tmp/agent-cli/ollama"
```

### 6.2 ピアとして相互通信させる

`registry_dir` を共有すると、別プロファイルでも互いを `/send` で呼び出せます。

```toml
# どちらの設定にも以下を入れる
[runtime]
registry_dir = "/tmp/agent-cli/team"
```

## 7. シェルツールのチューニング

長時間ジョブを扱う／大量出力するコマンドを許可するには `[tools.shell]` を調整します。

```toml
[tools.shell]
timeout_secs  = 600   # 10 分
max_output_kb = 4096  # 4 MB
```

注意：

- `timeout_secs` を超えたプロセスは強制終了され、ツール結果は失敗扱いになります。
- `max_output_kb` を超えた stdout/stderr は末尾に `...[truncated]` を付けて切り詰められます。
- AI が誤って巨大コマンドを呼ばないよう、対話承認（`auto_approve_tools=false`）の併用を推奨します。

## 8. UI 表示モード

`ui.show_thinking` で thinking ブロック（Claude の `thinking_delta`／Ollama の `message.thinking`）の表示量を制御します。`agent-cli` 起動時に解釈され、未知値（`"verbose"` 等）は既定の `"collapsed"` にフォールバックします。

| 値 | 挙動 |
|----|------|
| `"collapsed"`（既定） | 各 thinking delta を「先頭 80 文字 + `...`」に切り詰め、改行があれば 1 行目のみ。`[thinking] <truncated>...` の形式で 1 行表示 |
| `"expanded"` | 受信した thinking text を全文逐次表示（`[thinking] <text>`） |
| `"hidden"` | thinking 行を一切表示しない（`AgentEvent::Thinking` を REPL 側で捨てる） |

設定変更は `agent-cli` 再起動で反映されます。実行時切替は未対応。

## 9. よくある設定ミスと診断

### 症状：起動直後にプロセスが終了する

- 原因：`api_key_env` で指定した環境変数が未設定。
- 診断：`agent-cli doctor` を実行。`api key env : ANTHROPIC_API_KEY ... NOT set` と表示されます。
- 対処：環境変数を `export` するか、別の `provider.kind` に切り替え。

### 症状：`agent-cli list` に他プロセスが現れない

- 原因：`registry_dir` がプロセス間で異なる、またはソケットが stale。
- 診断：双方の `agent-cli config show` の `registry_dir` を比較。`ls /tmp/agent-cli/` で `.sock`／`.json` を確認。
- 対処：`registry_dir` を共有する設定にして再起動。

### 症状：`provider conn : FAIL` が `doctor` に出る

- 原因：API キーが間違っている／ローカルサーバーが停止している／`base_url` が違う。
- 診断：手動で `curl -s $base_url/health` などを試す。
- 対処：URL／キー／サーバー稼働を確認。

### 症状：シェルツールが「timed out」

- 原因：`timeout_secs` を超過。
- 対処：`[tools.shell] timeout_secs` を増やすか、AI に短いコマンドを使うよう指示。

### 症状：`config file not found` で終了する

- 原因：`--config` または `AGENT_CLI_CONFIG` で存在しないパスを指定している（明示パスは自動生成しない）。
- 対処：パスを確認、または既定パス（自動生成対象）を使う。

## 10. 設定変更の反映と再起動

- ほとんどの設定は **プロセス起動時に読み込まれる** ため、変更後は `agent-cli` を再起動してください。
- 例外として、以下は実行中の REPL から動的に変更できます：
  - **ペルソナファイル**：REPL 内 `/reload-persona` で再読込（システムプロンプトのみ更新、会話履歴は保持）
  - **ツール承認スキップ**：REPL 内 `/auto on`／`/auto off`／`/auto status`（`auto_approve_tools` をその場で上書き）
- `--provider`／`--model`／`--persona`／`--auto-approve-tools` は CLI オプションで上書き可能です（プロセス単位）。
