# agent-cli

`agent-cli` は、Claude Code 相当の AI エージェント（ツール/思考/ストリーミング REPL）を 1 つのバイナリにまとめた、Rust 製のスタンドアロン CLI です。tmux に依存しません。各プロセスはちょうど 1 つのエージェントを所有し、他のエージェントとはローカルの Unix ドメインソケット IPC を介して通信します。

> The English version is [`README.md`](README.md). （英語版は [`README.md`](README.md) です。本書はそのメンテナンス対象の日本語訳です。）

## 特長

- スタンドアロン — tmux 不要。`agent-cli` を実行するだけです（引数なしは `agent-cli run` と等価）。
- ゼロから実装した Claude Code 相当の REPL。組み込みツールと思考機能を備えます（REPL とツール自体は `claude` CLI を呼び出しません）。なお `claude` CLI を駆動する方法は、後述の `claude-code` バックエンドとして別途選択できます。
- 7 つのバックエンド: `claude` / `claude-code` / `codex` / `ollama` / `opencode` / `opencode-go` / `llama.cpp`。
- マルチエージェント連携 — 別々のプロセスが `/send <peer> <text>` でプロンプトを交換します。
- デタッチドエージェント — `agent-cli spawn`（または `/spawn`）は、起動元プロセスに依存せず独自セッションで動くヘッドレスなピアを作成します。`agent-cli stop <peer>`（または `/stop`）で停止します。
- グループ — 起動したコホートを `--group <id>` でタグ付け（デタッチドな子が継承）。`agent-cli list --group <id>` で絞り込み、`agent-cli groups` で稼働中のコホートを検出します。
- ペルソナファイル（YAML フロントマター + Markdown 本文）でロール、スキル、ツールの許可/拒否リスト、モデル、temperature を定義します。
- 組み込みツール: `bash` / `read` / `write` / `send_to` / `edit` / `glob` / `grep` / `monitor` / `websearch` / `webfetch`。承認モードは実行中に `/auto on` で切り替えられます。
- カスタムスラッシュコマンド — `.agent-cli/commands/` に Markdown ファイルを置くだけで `/<name>` として使えます。`$ARGUMENTS` / `$1`…`$N` / `@file` の展開と、前方一致による自動実行に対応します。
- プロンプトの行編集 — `↑` / `↓` での履歴参照、`Ctrl+A` / `Ctrl+E`、`Esc` でのクリア、`/` コマンド入力中の候補表示（プロンプトの 1 行上）、`Tab` によるコマンド補完。
- `Esc` による実行中ターンの停止 — 応答のストリーミング中、ツール実行中、承認待ちのいずれでも `Esc` を押せば、モデルやツールの完了を待たずに即座にプロンプトへ戻ります。会話はそのまま継続できます。
- 実行中の進捗表示 — 実行内容を 1 行で表示し（収まらない分は `…` で省略）、その下の行にスピナーと経過時間を表示します。ターンが終わると `✔ <経過時間>` に変わります。ツール結果は画面上 5 行に省略し（`… +N more lines`、モデルとログには全文が渡ります）、さらにその下へ thinking（推論）の内容を最新 10 行までライブ表示し、超過分は `… +N more` で省略します。ブロックをマウスでクリックすると画面に収まる範囲の全文表示に切り替わり、もう一度クリックで 10 行表示に戻ります。
- プロンプトからのシェル実行 — Claude Code と同様に `!<コマンド>` と入力すると、モデルを経由せず即座にシェルで実行します（承認プロンプトはありません）。出力は実行しながら逐次表示され、長いコマンドは `Esc` で中断できます。実行したコマンドとその出力はモデルへコンテキストとして渡されるので、続けて「この結果は？」と聞けます。タイムアウト・コンテキスト上限・無効化は `[shell]` で設定します。
- マウスホイールでのスクロールバック — ホイールを回すと、**プロンプト行はその位置に表示したまま**これまでのログを上下にスクロールします。入力中の文字とカーソルはそのまま残り、編集も続けられます。ターン実行中もスクロールでき、画面下端にはスピナーと経過時間が固定表示されます。最下部まで戻すか `Esc` で元の表示に戻ります。キーボードの操作は変更していません（`↑` / `↓` は従来どおり入力履歴の移動です）。`[ui] mouse_scroll` と `[ui] scrollback_lines` で制御できます。
- 配色付きの表示 — プロンプトとツール名はシアン、引数はグレー、ツール結果と経過時間は暗めの黄、回答マーカーと起動バナーはマゼンタ、`✔` は緑、`✗` とエラーは赤、承認プロンプトは黄で表示します。回答本文は最も長く読む部分なので着色しません。色は端末に接続されたストリームにのみ出力し、`NO_COLOR` を尊重します。`[ui] color` で常時有効化・無効化もできます。
- スクリプトから利用可能 — `agent-cli run` に質問をパイプで流し込む、あるいは稼働中のエージェントに `agent-cli ask <peer> <text>` で問い合わせて応答だけを標準出力で受け取れます。
- ストリーミング応答は REPL のプロンプトと同期しており、応答完了後は常に新しい `> ` が再描画されます。
- 確実なシャットダウン — `/quit`、`/exit`、`Ctrl+D`、`Ctrl+C`、`SIGTERM` のいずれでも約 1 秒以内に終了し、IPC ソケットとレジストリのメタデータを自動的に後始末します。
- `agent-cli doctor` による自己診断と、`agent-cli selftest` による 5 段階のスモークテスト（Provider OK / bash ツール / IPC / 子プロセス登録 / 子プロセスの AI 応答）。
- `agent-cli update` による自己アップデート — インストール先へソースからビルドし直します。既定は `main`、`--ref` で任意のタグ/ブランチを指定できます。`--check` は変更を加えずに最新リリースとの比較だけを報告します。
- MCP クライアント — 外部の Model Context Protocol サーバーを `[[mcp.servers]]` に宣言すると、起動時に **stdio**（サブプロセス）または **http**（URL への Streamable HTTP）でツールが検出され、`mcp__<server>__<tool>` としてエージェントに提供されます。`agent-cli mcp list` で確認できます。
- `[runtime] max_tool_iterations` でツール使用ループ上限を設定可能（デフォルト 24、最大 `u32::MAX`）。下記「[info] max tool-use iterations reached」を参照。
- Ollama の `message.thinking` フィールドは、`glm-5.1:cloud` のような思考対応モデル向けに `[thinking]` としてデコードされます。
- オプトインのコンテキスト効率化機能（すべてデフォルト OFF）: Claude プロンプトキャッシュ、opencode ローカル永続セッション、ハイブリッド履歴ウィンドウ管理（要約してから破棄）。[`doc/config.md`](doc/config.md) §11 を参照。

## 対応バックエンド

| kind | API | デフォルトモデル |
|------|-----|--------------|
| claude | Anthropic Claude (Messages, SSE) | `claude-opus-4-7` |
| claude-code | ローカルの Claude Code CLI を子プロセスとして駆動（API キー不要） | Claude Code 自身のデフォルト |
| codex | OpenAI Chat Completions (SSE) | `gpt-4.1` |
| ollama | Ollama `/api/chat` (NDJSON) | `glm-5.1:cloud` |
| opencode | OpenCode — デュアルモード（下記参照） | `claude-sonnet-4-5` |
| opencode-go | OpenCode Go クラウド（自動設定ショートカット） | `claude-sonnet-4-5` |
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

**`opencode-go`** は、Go 固有のデフォルト値が自動設定される OpenCode の便利なエイリアスです。
`kind = "opencode-go"` と `api_key_env` だけを設定すれば、`base_url`
（`https://opencode.ai/zen/go/v1`）、`api`（`"anthropic"`）、`model`
（`claude-sonnet-4-5`）が自動的に埋められます。`[provider.opencode]`
で個別に上書きすることもできます。下記の設定例を参照してください。

**`claude-code`** は、マシンにインストール済みの Claude Code CLI を子プロセスとして
起動し、プロバイダー抽象に適合させます。`agent-cli` の REPL・ペルソナ・
エージェント間 IPC・ログ・カスタムスラッシュコマンドが、そのまま Claude Code を
包む形になります。API キーは不要で、Claude Code 自身の認証をそのまま使います:

```toml
[provider]
kind = "claude-code"

[provider.claude-code]
mode      = "delegation"   # Claude Code 自身がツールを実行（デフォルト）
transport = "stream"       # トークン単位のストリーミング（デフォルト）
```

先に把握しておくべき点が 2 つあります。デフォルトの `delegation` モードでは
Claude Code 側がツールを実行するため、agent-cli のツールレジストリと承認プロンプトは
関与しません。また `gateway` モード（`--tools ""`）ではツールが一切実行されません。
`claude -p` は外部のツール定義を受け付けないためです。
[`doc/providers/claude-code.md`](doc/providers/claude-code.md) を参照してください。

必須の検証対象は `claude` と `ollama`（モデル `glm-5.1:cloud`）です。

| 機能 | claude | claude-code | codex | ollama | opencode | llama.cpp |
|------------|--------|-------------|-------|--------|----------|-----------|
| ストリーミング | ✓ | ✓（`transport = "stream"`） | ✓ | ✓ | ✓（クラウド SSE / ローカルはバッファ） | ✓ |
| ツール使用 | ✓ | ✓ delegation モード（Claude Code **内部**で実行） | ✓（function calling） | ✓（モデル依存） | ✓ クラウド / ✗ ローカル (v1) | ✓（サーバービルド依存） |
| 思考 | ✓ (`thinking_delta`) | ✓（`transport = "stream"`） | ✗ | ✓（モデル依存, `message.thinking`） | ✗ | ✗ |

`opencode-go` の機能は `opencode`（クラウドモード）と同じです。設定のショートカットであり、別のバックエンドではありません。

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

対話セッションは必須ではありません。コマンドラインだけで質問と応答を完結させられます:

```bash
# 質問をパイプで渡すと、応答が表示され EOF でプロセスが終了します。
echo "Rust の所有権を 3 行で説明して" | agent-cli run

# 無人実行でツールも使わせる場合。
echo "src 配下の .rs ファイル数を bash で数えて" | agent-cli run --auto-approve-tools

# 稼働中のエージェントに問い合わせ、応答テキストだけを受け取る場合。
agent-cli ask bob "現在の設計上のリスクをまとめて"
```

適用されるルール（1 行 1 プロンプト、承認の扱い、出力の構成）は [`doc/usage.md`](doc/usage.md) の "Non-interactive / Scripted Use" を参照してください。

## 設定

設定ファイルは TOML です。解決順序:

1. `--config <path>`（明示指定）
2. `AGENT_CLI_CONFIG` 環境変数
3. プロジェクトローカル `./.agent-cli/config.toml`（存在する場合のみ使用）
4. デフォルト `~/.config/agent-cli/config.toml`

明示指定したパスは存在している必要があります（自動生成しません）。カレントディレクトリに `.agent-cli/config.toml` があれば自動的に使用されます（自動生成はされず、親ディレクトリは辿らずカレントディレクトリのみを確認します）。デフォルトパスは初回起動時に適切なテンプレートを自動生成します。全セクションにコメントを付けた雛形として [`example/config.example.toml`](example/config.example.toml) も利用できます。

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

**claude-code** — ローカルにインストール済みの Claude Code CLI を子プロセスとして駆動します。API キーは不要で、Claude Code 自身の認証を使います。以下のキーはすべて任意です:

```toml
[provider]
kind = "claude-code"

[provider.claude-code]
bin       = "claude"       # 実行ファイル名（PATH から解決）またはフルパス
model     = "sonnet"       # --model。省略時は Claude Code 自身のデフォルト
mode      = "delegation"   # "delegation"（Claude Code 側のツール）| "gateway"（チャットのみ）
transport = "stream"       # "stream"（トークンストリーミング）| "oneshot"
session   = "persistent"   # "persistent" | "ephemeral"（delegation のみ）
turn_timeout_secs = 900
# tools           = ["Bash", "Read"]
# permission_mode = "auto"
# max_budget_usd  = 1.0
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

**opencode-go** — OpenCode Go クラウドの便利な kind。`base_url`、`api`、`model` が自動設定されます:

```toml
[provider]
kind = "opencode-go"

[provider.opencode]
api_key_env = "OPENCODE_API_KEY"   # 必須フィールドはこれだけ
# base_url、api、model は自動設定; 必要に応じて [provider.opencode] で上書き可能
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
| `agent-cli spawn [...]` | 起動元より長く生きるデタッチドなヘッドレスピアを作成（`run` と同じオプション） |
| `agent-cli stop <peer>` | 稼働中のピア（id または名前）を停止。IPC 経由の穏当なシャットダウン、失敗時は `SIGTERM` にフォールバック |
| `agent-cli serve [...]` | ヘッドレス実行（登録 + ピアへの応答のみ、REPL なし）。`spawn` が起動する対象 |
| `agent-cli list [--group <id>]` | 稼働中のピアを一覧表示（`GROUP` 列付き。`--group` で 1 グループに絞り込み） |
| `agent-cli groups` | 稼働中のグループを検出して一覧表示（メンバー数付き） |
| `agent-cli send <peer> <text>` | ピアにワンショットのプロンプトを送信（応答は待ちません） |
| `agent-cli ask <peer> <text> [--timeout <secs>]` | ピアにプロンプトを送り、応答を待って表示（デフォルト 120 秒） |
| `agent-cli providers` | バックエンドの状態を表示 |
| `agent-cli doctor` | 設定 / API キー / 接続性 / レジストリ / `bash` を健全性チェック |
| `agent-cli update [--check] [--force] [--yes] [--ref <ref>]` | `main`（または `--ref`）から再ビルドして置き換え（`cargo` でソースビルド） |
| `agent-cli selftest [--provider <kind>]` | 5 段階のスモークテスト |
| `agent-cli config show` | 現在の設定を表示 |
| `agent-cli config edit` | `$EDITOR` で設定を開く |
| `agent-cli config path` | 解決済みの設定パスを表示 |
| `agent-cli mcp list` | 設定済みの MCP サーバーへ接続し、ツール一覧を表示 |

`agent-cli run` 内の REPL コマンド:

| コマンド | 用途 |
|---------|---------|
| `/list` | 稼働中のピアを一覧表示 |
| `/send <peer> <text>` | ピアにプロンプトを送信 |
| `/spawn [name] [provider]` | このセッションより長く生きるデタッチドなピアを作成 |
| `/stop <peer>` | 稼働中のピア（id または名前）を停止。デタッチドエージェントも対象 |
| `/tools` | このエージェントで有効なツールを一覧表示 |
| `/persona` | このエージェントのペルソナを表示（ロール / スキル / ソースパス） |
| `/reload-persona` | ペルソナファイルを再解決して再読み込み（履歴は保持） |
| `/peer <id_or_name>` | ピアのペルソナ概要を表示 |
| `/history [n]` | 直近 n 件（デフォルト 20）のユーザー入力を表示 |
| `/clear`, `/reset` | 会話履歴をクリア（ペルソナ / システムプロンプトは保持） |
| `/cancel` | 実行中の AI 応答またはツール呼び出しを停止（実行中の `Esc` と同じ信号） |
| `/auto [on\|off\|status]` | 実行中にツール承認スキップを切り替え |
| `/commands` | カスタムスラッシュコマンドを一覧表示（名前 / 先頭行 / ファイルパス） |
| `/reload-commands` | カスタムコマンドのディレクトリを再スキャン |
| `/help` | ヘルプを表示 |
| `/quit`, `/exit` | 終了（完全なエイリアス） |

ユーザープロンプトと実行したスラッシュコマンドは `<runtime.log_dir>/history.txt`（直近 200 件）に永続化され、次回起動時に再読み込みされます（`/quit` と `/exit` は除外）。詳細は [`doc/usage.md`](doc/usage.md) を参照してください。

### デタッチドエージェント

`agent-cli spawn` は、2 つ目のターミナルを開かずにピアを作成します。子プロセスは
独自セッションでヘッドレスに動作し、起動元に依存しません。起動元の終了・`Ctrl+C`・
端末切断のいずれでも生き残ります:

```bash
agent-cli spawn --name worker      # デタッチドなヘッドレスピアを起動
agent-cli list                     # `worker` は通常のピアとして登録される
agent-cli ask worker "..."         # 通常どおり送受信（send / ask / send_to ツール）
agent-cli stop worker              # 穏当にシャットダウンを要求
```

子プロセスは起動元の設定ファイル（したがって同じ `[runtime] registry_dir`）を
引き継ぐため、直ちに検出可能です。y/N 承認を答えるコンソールを持たないため、
ヘッドレスエージェントはツール実行を自動承認します。REPL 内では同じ操作が
`/spawn [name] [provider]` と `/stop <peer>` として利用できます。詳細は
[`doc/usage.md`](doc/usage.md) の "Detached agents" を参照してください。

### グループ

**グループ**は、まとめて起動したエージェント群が共有する識別子で、コホート全体を
ひと目で認識できるようにします。`--group <id>`（または `[runtime] group` 設定キー）
で付与します。デタッチドな子は起動元のグループを自動的に引き継ぐため、id はルートで
一度だけ指定すれば済みます:

```bash
agent-cli spawn --group team --name lead     # グループ付きのデタッチドピア
agent-cli spawn --group team --name helper    # 何も指定し直さず同じグループを継承
agent-cli list --group team                   # team のメンバーだけ（GROUP 列）
agent-cli groups                              # team  2  lead, helper
```

`agent-cli groups` はライブなレジストリを走査し、稼働中の各グループをメンバー数
付きで一覧表示します（グループなしのエージェントは `-` バケットにまとめられます）。
グループは起動時に固定されます。詳細は [`doc/usage.md`](doc/usage.md) の "Groups"
を参照してください。

### アップデート

`agent-cli update` は、インストール済みの agent-cli をリポジトリから再ビルドします。
agent-cli はビルド済みバイナリを配布しておらず（インストールはソースビルド）、
アップデートも同様です: `cargo install --git … --branch main` を実行中バイナリの
インストール先へ実行し、新しい `--version` を検証します。
Rust ツールチェーン（`cargo`）が必要で、Linux 専用です。

```bash
agent-cli update --check       # 現在と最新リリースを報告。変更しない
agent-cli update               # 確認のうえ main から再ビルドして置き換え
agent-cli update --yes         # 確認プロンプトをスキップ
agent-cli update --ref v0.11.0 # 特定のタグ（または別ブランチ）からビルド
```

`--ref` を指定しない場合は **`main`** からビルドします。オプションなしの
`agent-cli update` は `agent-cli update --ref main` と同等です。

詳細は [`doc/usage.md`](doc/usage.md) の "Updating" を参照してください。

### MCP サーバー

agent-cli は **Model Context Protocol (MCP) クライアント**として動作できます:
外部サーバーを `[[mcp.servers]]` に宣言すると、`run` / `serve` 時にそのツールが
検出され、（組み込みツールと並んで、同じ承認ゲートを通して）
`mcp__<server>__<tool>` としてエージェントに提供されます。サーバーへは **stdio**
（サブプロセス）または **http**（URL への Streamable HTTP）で接続します。消費する
のは MCP ツールのみで、Linux 専用です。

```toml
[[mcp.servers]]                 # stdio
name    = "filesystem"
command = "npx"
args    = ["-y", "@modelcontextprotocol/server-filesystem", "/home/user"]

[[mcp.servers]]                 # http (Streamable HTTP)
name      = "remote"
transport = "http"
url       = "https://example.com/mcp"
```

```bash
agent-cli mcp list   # 各サーバーへ接続してツールを一覧表示
```

起動やハンドシェイクに失敗したサーバーはログに記録されてスキップされ、起動を
中断させることはありません。詳細は [`doc/config.md`](doc/config.md) の "[mcp]" と
[`doc/usage.md`](doc/usage.md) の "MCP servers" を参照してください。

### ツール承認のスキップ

ツール呼び出し（bash, read, write, send_to, monitor, edit, glob, grep, websearch, webfetch）はデフォルトで y/N の承認を求めます。承認をスキップする方法は 3 つあります:

| 方法 | 例 |
|--------|---------|
| 設定ファイル | `[runtime] auto_approve_tools = true` |
| CLI フラグ | `agent-cli run --auto-approve-tools` |
| REPL コマンド | `/auto on`（`/auto off` で承認モードに戻る、`/auto status` で現在値を表示） |

承認モードでは、各ツール要求が `[tool approval] <tool> <args>` と `approve? [y/N]:` を表示します。受理されるのは `y` / `yes` のみで、それ以外（空入力や他の語）は拒否として扱われます。

これは `agent-cli` 自身が実行するツールに対する仕組みです。`kind = "claude-code"`
を `delegation` モードで使う場合、ツールは Claude Code 内部で実行されるため、上記
3 つの方法はいずれも適用されません。そのバックエンドの `permission_mode` /
`tools` / `allowed_tools` / `disallowed_tools` で制御してください。

### カスタムスラッシュコマンド

`.agent-cli/commands/` にある `*.md` ファイルは、ファイル名（拡張子を除く）のスラッシュコマンドになります。`.agent-cli/commands/review.md` なら `/review` です。実行するとファイルの内容が展開され、ユーザープロンプトとしてエージェントに送信されます。

```markdown
<!-- .agent-cli/commands/review.md -->
以下のファイルをレビューし、深刻な問題を 3 つ挙げてください。

対象: $1
観点: $ARGUMENTS

@doc/tools.md
```

```text
> /review src/agent.rs security
```

| プレースホルダー | 展開結果 |
|-------------|-----------|
| `$ARGUMENTS` | コマンド名の後ろに入力した引数文字列全体 |
| `$1`, `$2`, … | 空白区切りの N 番目の引数。存在しない場合は空文字列 |
| `@<path>` | 対象ファイルの内容（読めない場合は `[error: cannot read @<path>]`） |

- 組み込みコマンドが優先されます。`help.md` を置いても `/help` は上書きされません。
- 前方一致するカスタムコマンドが 1 つだけならそのまま実行され、`[auto] /<入力> → /<解決後>` が表示されます。複数一致する場合は候補が一覧表示されます。
- `/commands` で読み込み済みのコマンドを一覧表示、`/reload-commands` で再起動せずに再スキャンできます。
- ディレクトリは `[runtime] commands_dir`（デフォルト `.agent-cli/commands`）で変更できます。存在しなくてもエラーにはなりません。

完全なリファレンスは [`doc/usage.md`](doc/usage.md) の "Custom Slash Commands" を参照してください。

### REPL の行編集

端末に接続されている場合、プロンプトはその場での行編集と履歴参照に対応します:

| キー | 動作 |
|-----|--------|
| `↑` / `↓` | 履歴を参照（最新より先に戻ると入力途中の内容が復元されます） |
| `Ctrl+A` / `Home`、`Ctrl+E` / `End` | 行頭 / 行末へ移動 |
| `Esc` | エージェントの実行中はターンを停止して即座にプロンプトへ戻る。待機中のプロンプトでは履歴参照を抜ける、または行をクリア |
| `Ctrl+C` | エージェントの実行中は `Esc` と同じ。待機中のプロンプトでは行をクリアし、空行なら終了 |
| `Ctrl+D` | 空行で終了 |

行が `/` で始まり空白を含まない間は、最も一致するコマンド名がインラインで表示されます。raw モードは TTY が必要で、パイプ入力では行単位の読み込みにフォールバックします（ツール、カスタムコマンド、ピア通信はそのまま動作します）。

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

# スモークテスト（5 段階: provider OK / bash / IPC / 子プロセス / 子プロセスの AI 応答）
agent-cli selftest --provider claude
agent-cli selftest --provider ollama

# 半自動の受け入れシナリオ（環境変数の有無で PASS / SKIP / FAIL を集計）
scripts/manual_acceptance.sh
```

`selftest` のステージ 1 は稼働中のバックエンドが必要です。ステージ 2–4（bash ツール、IPC 往復、子プロセス IPC）は外部依存なしで実行されます。ステージ 5 は動作するプロバイダーに加えて子プロセスの起動が必要です。

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
allowed_tools: [bash, read]
denied_tools:  [write]
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
- [`doc/providers/claude.md`](doc/providers/claude.md) / [`claude-code.md`](doc/providers/claude-code.md) / [`codex.md`](doc/providers/codex.md) / [`ollama.md`](doc/providers/ollama.md) / [`opencode.md`](doc/providers/opencode.md) / [`llamacpp.md`](doc/providers/llamacpp.md) — バックエンド別ガイド
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — 開発ガイド
- [`CHANGELOG.md`](CHANGELOG.md) — リリースノート

## ライセンス

MIT License. [`LICENSE.md`](LICENSE.md) を参照してください。
