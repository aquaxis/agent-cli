# agent-cli

`agent-cli` は、Claude Code 相当の AI エージェント（ツール/思考/ストリーミング REPL）を 1 つのバイナリにまとめた、Rust 製のスタンドアロン CLI です。tmux に依存しません。各プロセスはちょうど 1 つのエージェントを所有し、他のエージェントとはローカルの Unix ドメインソケット IPC を介して通信します。

> The English version is [`README.md`](README.md). （英語版は [`README.md`](README.md) です。本書はそのメンテナンス対象の日本語訳です。）

## 特長

- スタンドアロン — tmux 不要。`agent-cli` を実行するだけです（引数なしは `agent-cli run` と等価）。
- ゼロから実装した Claude Code 相当の REPL。組み込みツールと思考機能を備えます（`claude` CLI を呼び出しません）。
- 5 つのバックエンド: `claude` / `codex` / `ollama` / `opencode` / `llama.cpp`。
- マルチエージェント連携 — 別々のプロセスが `/send <peer> <text>` でプロンプトを交換します。
- ペルソナファイル（YAML フロントマター + Markdown 本文）でロール、スキル、ツールの許可/拒否リスト、モデル、temperature を定義します。
- 組み込みツール: `shell` / `fs_read` / `fs_write` / `send_to`。承認モードは実行中に `/auto on` で切り替えられます。
- ストリーミング応答は REPL のプロンプトと同期しており、応答完了後は常に新しい `> ` が再描画されます。
- 確実なシャットダウン — `/quit`、`/exit`、`Ctrl+D`、`Ctrl+C`、`SIGTERM` のいずれでも約 1 秒以内に終了し、IPC ソケットとレジストリのメタデータを自動的に後始末します。
- `agent-cli doctor` による自己診断と、`agent-cli selftest` による 5 段階のスモークテスト（Provider OK / shell ツール / IPC / 子プロセス登録 / 子プロセスの AI 応答）。
- `[runtime] max_tool_iterations` でツール使用ループ上限を設定可能（デフォルト 24、最大 `u32::MAX`）。下記「[info] max tool-use iterations reached」を参照。
- Ollama の `message.thinking` フィールドは、`glm-5.1:cloud` のような思考対応モデル向けに `[thinking]` としてデコードされます。
- オプトインのコンテキスト効率化機能（すべてデフォルト OFF）: Claude プロンプトキャッシュ、opencode ローカル永続セッション、ハイブリッド履歴ウィンドウ管理（要約してから破棄）。[`doc/config.md`](doc/config.md) §11 を参照。

## 対応バックエンド

| kind | API | デフォルトモデル |
|------|-----|--------------|
| claude | Anthropic Claude (Messages, SSE) | `claude-opus-4-7` |
| codex | OpenAI Chat Completions (SSE) | `gpt-4.1` |
| ollama | Ollama `/api/chat` (NDJSON) | `glm-5.1:cloud` |
| opencode | OpenCode — デュアルモード（下記参照） | `claude-sonnet-4-5` |
| llama.cpp | OpenAI 互換 `/v1/chat/completions` (SSE) | `default` |

`opencode` は API キーの有無でモードを選択します:

- **キーなし → ローカルモード。** 稼働中の `opencode serve` にネイティブのセッション API
  （`POST /session` → `POST /session/:id/message`、同期 JSON）で接続します。
  デフォルト `base_url` は `http://127.0.0.1:4096`。
- **キーあり → クラウドモード（OpenCode Zen）。** デフォルトのクラウド `base_url` は
  `https://opencode.ai/zen/v1`、キーの環境変数は `OPENCODE_API_KEY`、`Authorization: Bearer`。
  ワイヤ形式は `[provider.opencode] api` で選択できます: `"openai"`（デフォルト）→
  `POST {base_url}/chat/completions`（SSE, `[DONE]`）、`"anthropic"` →
  `POST {base_url}/messages`（Anthropic SSE）。対応する `base_url`（例: "go" エンドポイント
  `https://opencode.ai/zen/go/v1`）と組み合わせてください。
  [`doc/providers/opencode.md`](doc/providers/opencode.md) を参照。

必須の検証対象は `claude` と `ollama`（モデル `glm-5.1:cloud`）です。

| 機能 | claude | codex | ollama | opencode | llama.cpp |
|------------|--------|-------|--------|----------|-----------|
| ストリーミング | ✓ | ✓ | ✓ | ✓（クラウド SSE / ローカルはバッファ） | ✓ |
| ツール使用 | ✓ | ✓（function calling） | ✓（モデル依存） | ✓ クラウド / ✗ ローカル (v1) | ✓（サーバービルド依存） |
| 思考 | ✓ (`thinking_delta`) | ✗ | ✓（モデル依存, `message.thinking`） | ✗ | ✗ |

## インストール

### ワンライナー

```bash
curl -fsSL https://raw.githubusercontent.com/aquaxis/agent-cli/main/install.sh | sh
```

### `install.sh` の動作

- 対象は Linux（x86_64 / aarch64）。他のプラットフォームはエラーで早期終了します。
- デフォルトのインストール先: `$HOME/.local/bin/agent-cli`。
- `agent-cli` リポジトリ内部から実行された場合はローカルソースをビルドし、そうでなければ `AGENT_CLI_REPO` を `git clone` してビルドします。
- 既存のバイナリは上書きされます。`~/.config/agent-cli/config.toml` はそのまま残ります。

| 変数 | デフォルト | 用途 |
|----------|---------|---------|
| `AGENT_CLI_REPO` | GitHub のソースリポジトリ | クローン元 |
| `AGENT_CLI_REF` | `main` | ブランチ / タグ / コミット |
| `AGENT_CLI_PREFIX` | `$HOME/.local` | インストール先プレフィックス |
| `AGENT_CLI_INSTALL_FORCE` | （未設定） | `1` を設定すると上書き通知を抑制 |

### ソースからビルド

```bash
git clone https://github.com/aquaxis/agent-cli.git
cd agent-cli
cargo install --path . --root "$HOME/.local"
```

## クイックスタート

```bash
# 1. 初回起動時にデフォルト設定が作成されます。
agent-cli config path
# => ~/.config/agent-cli/config.toml

# 2. 使用するバックエンドの API キーを設定します（Claude の例）。
export ANTHROPIC_API_KEY=sk-ant-...

# 3. REPL を起動します（引数なしは `agent-cli run` と等価）。
agent-cli                       # 設定の provider.kind を使用
# または
agent-cli run --provider claude # コマンドラインで上書き

# 4. 別のターミナルで Ollama を使う 2 つ目のエージェントを起動します。
agent-cli run --provider ollama --model glm-5.1:cloud --name bob

# 5. 1 つ目のセッションからプロンプトを送ります。
> /list
> /send bob "hello from claude side"

# 6. REPL を終了します。
> /quit       # /exit, Ctrl+D, Ctrl+C でも可
```

## 設定

設定ファイルは TOML です。解決順序:

1. `--config <path>`（明示指定）
2. `AGENT_CLI_CONFIG` 環境変数
3. デフォルト `~/.config/agent-cli/config.toml`

明示指定したパスは存在している必要があります（自動生成しません）。デフォルトパスは初回起動時に適切なテンプレートを自動生成します。

`[provider] kind` でアクティブなバックエンドを選択します。埋める必要があるのはそのバックエンドの `[provider.*]` テーブルだけですが、複数のテーブルを 1 つのファイルに残しておき、`kind`（または `--provider`）で切り替えることもできます。

### バックエンド別の設定例

**claude** — Anthropic Claude (Messages, SSE):

```toml
[provider]
kind = "claude"

[provider.claude]
api_key_env  = "ANTHROPIC_API_KEY"  # シークレットを保持する環境変数名
model        = "claude-opus-4-7"
base_url     = "https://api.anthropic.com"   # 通常はそのまま
thinking     = true                          # 思考ブロックを有効化
# prompt_cache = true                         # オプトイン: Anthropic プロンプトキャッシュ
```

**codex** — OpenAI Chat Completions (SSE, function calling)。`kind = "codex"` は内部名であり、OpenAI のレガシー Codex モデルを指すものではありません。`base_url` は OpenAI 互換ゲートウェイ / Azure OpenAI でも動作します:

```toml
[provider]
kind = "codex"

[provider.codex]
api_key_env = "OPENAI_API_KEY"
model       = "gpt-4.1"
base_url    = "https://api.openai.com/v1"
```

**ollama** — ローカルまたはクラウドの Ollama `/api/chat` (NDJSON)。API キー不要:

```toml
[provider]
kind = "ollama"

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"
```

**opencode** — ローカルモードは稼働中の `opencode serve` に接続（キー不要）。API キーが解決されると自動的にクラウドモード（OpenCode Zen）に切り替わります:

```toml
[provider]
kind = "opencode"

# ローカルモード（デフォルト）: 稼働中の `opencode serve`、キー不要。
[provider.opencode]
base_url = "http://127.0.0.1:4096"
model    = "claude-sonnet-4-5"
# persistent_session = true   # オプトイン（ローカルのみ）: 1 つのサーバーセッションを再利用

# クラウドモード（OpenCode Zen）: api_key_env を設定すると、その存在でクラウドに切り替わります。
# base_url    = "https://opencode.ai/zen/v1"
# api_key_env = "OPENCODE_API_KEY"
# api         = "anthropic"   # クラウドのワイヤ形式: "openai"（デフォルト）| "anthropic"
#                             # 対応する base_url（例: .../zen/go/v1）と組み合わせる
```

**llama.cpp** — `llama-server` の OpenAI 互換 `/v1/chat/completions`。TOML キーにドットを含むため `"llama.cpp"` を引用符で囲みます。サンプリングのパラメータは `llama-cli` のフラグに対応しており、すべて省略可能です（省略すると → サーバー自身のデフォルト）:

```toml
[provider]
kind = "llama.cpp"

[provider."llama.cpp"]
model    = "default"
base_url = "http://127.0.0.1:8080"
# api_key_env = "LLAMACPP_API_KEY"   # 任意; Bearer 認証ビルドのみ
# max_tokens     = 1024   # -n / --n-predict : 生成する最大トークン数
# temperature    = 0.2    # --temp
# top_k          = 80     # --top-k
# top_p          = 0.95   # --top-p
# min_p          = 0.05   # --min-p
# repeat_penalty = 1.05   # --repeat-penalty
# repeat_last_n  = 64     # --repeat-last-n
# seed           = 0      # --seed
```

### オプトインのコンテキスト効率化機能

すべてデフォルト OFF。すべてのフラグが OFF のとき、リクエストボディと履歴処理はバイト単位で従来どおりです。[`doc/config.md`](doc/config.md) §11 を参照。

```toml
[provider.claude]
prompt_cache = true              # Anthropic プロンプトキャッシュ（system + tools + 末尾）

[provider.opencode]
persistent_session = true        # ローカル OpenCode セッションをターン間で再利用

[history]
enabled            = true        # 予算超過時に古いターンを要約してから破棄
max_context_tokens = 24000
keep_recent_turns  = 6
```

複数のプロファイルを並行実行するには、各インスタンスを個別の `--config` ファイルに向けてください。互いをピアとして検出させたい場合は `[runtime] registry_dir` を共有します。

`agent-cli config path` は、現在有効な解決済みの設定ファイルを表示します。プロバイダーの HTTP エラーメッセージにも解決済みの `config` 行が含まれるため、`~/.local/config/...` と `~/.config/...` の取り違えを即座に判別できます。

完全なリファレンスは [`doc/config.md`](doc/config.md)、よくある失敗モードは [`doc/troubleshooting.md`](doc/troubleshooting.md) を参照してください。

## サブコマンド

| コマンド | 用途 |
|---------|---------|
| `agent-cli run` | REPL を起動（1 プロセス 1 エージェント） |
| `agent-cli list` | 稼働中のピアを一覧表示 |
| `agent-cli send <peer> <text>` | ピアにワンショットのプロンプトを送信 |
| `agent-cli providers` | バックエンドの状態を表示 |
| `agent-cli doctor` | 設定 / API キー / 接続性 / レジストリ / `bash` を健全性チェック |
| `agent-cli selftest [--provider <kind>]` | 5 段階のスモークテスト |
| `agent-cli config show` | 現在の設定を表示 |
| `agent-cli config edit` | `$EDITOR` で設定を開く |
| `agent-cli config path` | 解決済みの設定パスを表示 |

`agent-cli run` 内の REPL コマンド:

| コマンド | 用途 |
|---------|---------|
| `/list` | 稼働中のピアを一覧表示 |
| `/send <peer> <text>` | ピアにプロンプトを送信 |
| `/tools` | このエージェントで有効なツールを一覧表示 |
| `/persona` | このエージェントのペルソナを表示（ロール / スキル / ソースパス） |
| `/reload-persona` | ペルソナファイルを再解決して再読み込み（履歴は保持） |
| `/peer <id_or_name>` | ピアのペルソナ概要を表示 |
| `/history [n]` | 直近 n 件（デフォルト 20）のユーザー入力を表示 |
| `/clear`, `/reset` | 会話履歴をクリア（ペルソナ / システムプロンプトは保持） |
| `/cancel` | 実行中の AI 応答またはツール呼び出しのキャンセルを要求 |
| `/auto [on\|off\|status]` | 実行中にツール承認スキップを切り替え |
| `/help` | ヘルプを表示 |
| `/quit`, `/exit` | 終了（完全なエイリアス） |

ユーザープロンプトは `<runtime.log_dir>/history.txt`（直近 200 件）に永続化され、次回起動時に再読み込みされます。詳細は [`doc/usage.md`](doc/usage.md) を参照してください。

### ツール承認のスキップ

ツール呼び出し（shell, fs_*, send_to）はデフォルトで y/N の承認を求めます。承認をスキップする方法は 3 つあります:

| 方法 | 例 |
|--------|---------|
| 設定ファイル | `[runtime] auto_approve_tools = true` |
| CLI フラグ | `agent-cli run --auto-approve-tools` |
| REPL コマンド | `/auto on`（`/auto off` で承認モードに戻る、`/auto status` で現在値を表示） |

承認モードでは、各ツール要求が `[tool approval] <tool> <args>` と `approve? [y/N]:` を表示します。受理されるのは `y` / `yes` のみで、それ以外（空入力や他の語）は拒否として扱われます。

### `[thinking]` 出力の抑制

`glm-5.1:cloud` のような長考モデルは大量の思考テキストを出力し、REPL が `[thinking] ...` 行で埋め尽くされることがあります。表示量は `[ui] show_thinking` で制御します:

```toml
[ui]
show_thinking = "hidden"     # 完全に抑制
# show_thinking = "collapsed"  # デフォルト: 先頭 80 文字 + "..." を 1 行で
# show_thinking = "expanded"   # 全文
```

| 値 | 動作 |
|-------|----------|
| `"hidden"` | `[thinking]` を一切表示しない |
| `"collapsed"`（デフォルト） | 各思考デルタを「先頭 80 文字 + `...`」に切り詰め、複数行なら先頭行のみ表示 |
| `"expanded"` | 全文をそのまま表示 |

変更は次回の `agent-cli` 起動時に反映されます。詳細は [`doc/config.md`](doc/config.md) の「UI display modes」を参照してください。

### `[info] max tool-use iterations reached`

このメッセージは、AI が毎ラウンド `tool_use` 要求を出し続け、最終的なテキスト回答を生成しないままターンごとの反復上限に達したときに REPL に表示されます。暴走ループに対するガードです。

- **エラーではありません** — `[error]` ではなく `[info]` 接頭辞です。エラーログには書き込まれず、監視アラートも発生しません。
- 次の `> ` プロンプトは即座に再描画され、会話履歴は保持されます。
- **設定で変更できる？** はい。`~/.config/agent-cli/config.toml` の `[runtime] max_tool_iterations` を編集して `agent-cli` を再起動してください（デフォルト `24`）。
- **「無制限」にできる？** 厳密に不可（暴走による課金 / GPU / 標準出力を防ぐため、真の無制限モードは意図的に提供していません）。型は `u32` なので実用上の最大は `u32::MAX = 4,294,967,295`（実質的に無制限）です。
- 推奨レンジ: 単純なチャット 4–8、設計してからデバッグするオーケストレーター 24–48、長時間の自律実験 64–256。
- 回避策: プロンプトを分割する、より具体的なゴールを与える、ペルソナの `denied_tools` で無関係なツールを除く、`/clear` してやり直す、`max_tool_iterations` を上げる。[`doc/troubleshooting.md`](doc/troubleshooting.md) と [`doc/config.md`](doc/config.md) を参照。

### 終了

以下のいずれでもプロセスは約 1 秒以内に終了し、IPC ソケット（`<registry_dir>/<agent-id>.sock`）とレジストリのメタデータ（`<registry_dir>/<agent-id>.json`）を削除します。ストリーミング中やツール実行中でも動作します。

| 方法 | アクション |
|--------|--------|
| REPL コマンド | `/quit` または `/exit` |
| EOF | `Ctrl+D`（stdin クローズ） |
| シグナル | `Ctrl+C`（SIGINT）または `kill <pid>`（SIGTERM） |

## 検証

```bash
# 自動テストスイート
cargo test

# フォーマット / Lint
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

# 自己診断
agent-cli doctor

# スモークテスト（5 段階: provider OK / shell / IPC / 子プロセス / 子プロセスの AI 応答）
agent-cli selftest --provider claude
agent-cli selftest --provider ollama

# 半自動の受け入れシナリオ（環境変数の有無で PASS / SKIP / FAIL を集計）
scripts/manual_acceptance.sh
```

`selftest` のステージ 1 は稼働中のバックエンドが必要です。ステージ 2–4（shell ツール、IPC 往復、子プロセス IPC）は外部依存なしで実行されます。ステージ 5 は動作するプロバイダーに加えて子プロセスの起動が必要です。

## ペルソナ

ペルソナファイル（YAML フロントマター付き Markdown）は、エージェントのロール、スキル、説明、許可/拒否ツール、モデル、temperature を定義します。例は [`example/agents/`](example/agents/) に同梱されています。

```bash
mkdir -p ~/.config/agent-cli/agents
cp example/agents/reviewer.md ~/.config/agent-cli/agents/alice.md
agent-cli run --name alice
# → <agents_dir>/alice.md が自動読み込みされます
```

解決順序: **`--persona <path>` → `[runtime] persona_file` → `<agents_dir>/<name>.md` → 組み込みデフォルト。**

最小例:

```markdown
---
name: alice
role: code reviewer
skills: [Rust, security]
allowed_tools: [shell, fs_read]
denied_tools:  [fs_write]
---

You are a senior reviewer. Always propose minimal-diff fixes.
```

フロントマターの完全なリファレンス、検証ルール、運用シナリオは [`doc/personas.md`](doc/personas.md) を参照してください。

## ドキュメント

- [`doc/usage.md`](doc/usage.md) — CLI と REPL コマンドのリファレンス
- [`doc/config.md`](doc/config.md) — 設定の完全リファレンス（最も詳細）
- [`doc/personas.md`](doc/personas.md) — ペルソナのリファレンス（全フロントマターキー、運用シナリオ）
- [`doc/tools.md`](doc/tools.md) — 組み込みツールの仕様
- [`doc/architecture.md`](doc/architecture.md) — アーキテクチャ概要
- [`doc/troubleshooting.md`](doc/troubleshooting.md) — 既知の不具合と対処
- [`doc/providers/claude.md`](doc/providers/claude.md) / [`codex.md`](doc/providers/codex.md) / [`ollama.md`](doc/providers/ollama.md) / [`opencode.md`](doc/providers/opencode.md) / [`llamacpp.md`](doc/providers/llamacpp.md) — バックエンド別ガイド
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — 開発ガイド
- [`CHANGELOG.md`](CHANGELOG.md) — リリースノート

## ライセンス

MIT License. [`LICENSE`](LICENSE) を参照してください。
