# agent-cli

`agent-cli`は、Claude Code相当のツール／thinking機能を備えたAIエージェントをRust製の単独バイナリで提供するCLIです。tmuxに依存せず、1プロセス＝1エージェントとして起動し、別プロセスのエージェントとローカルIPC（Unixドメインソケット）でプロンプトを送受信できます。

> English version: [`README.en.md`](README.en.md)

## 特徴

- 単独起動：tmux不要。`agent-cli`をシェルから直接実行（`agent-cli run`の省略形）
- Claude Code相当のREPL＋tools／thinkingを内蔵（自前実装）
- 4バックエンド対応：`claude` / `codex` / `ollama` / `llama.cpp`
- マルチエージェント協調：別プロセス間で`/send <peer> <text>`相互通信
- ペルソナファイル（YAMLフロントマター＋Markdown）で役割・スキル・ツール権限を定義
- シェル／ファイルR/W／ピア送信ツール内蔵。承認モードは`/auto on`で実行時切替可能
- ストリーミング応答とREPLプロンプトを同期し、応答完了後に必ず次プロンプトを再描画
- `/quit`／`/exit`／`Ctrl+D`／`Ctrl+C`／`SIGTERM`のいずれでも1秒以内に確実終了し、IPCソケット・レジストリメタを自動クリーンアップ
- 自己診断`agent-cli doctor`／スモークテスト`agent-cli selftest`（5ステージ：Provider／shellツール／IPC／子プロセス起動／子プロセスAI応答）

## 対応バックエンド

| kind | API | 既定モデル |
|------|-----|-----------|
| claude | Anthropic Claude | `claude-opus-4-7` |
| codex | OpenAI Chat Completions | `gpt-4.1` |
| ollama | Ollama `/api/chat` | `glm-5.1:cloud` |
| llama.cpp | OpenAI互換`/v1/chat/completions` | `default` |

完成判定の検証必須対象は`claude`と`ollama (glm-5.1:cloud)`の2構成です。

## インストール

### ワンライナー

```bash
curl -fsSL https://raw.githubusercontent.com/aquaxis/agent-cli/main/install.sh | sh
```

### `install.sh`の動作

- 対象：Linux（x86_64／aarch64）
- 既定インストール先：`$HOME/.local/bin/agent-cli`
- カレントが`agent-cli`リポジトリ内であればローカルソースから、そうでなければ`AGENT_CLI_REPO`から`git clone`してビルドします
- 既存バイナリは上書きします。設定ファイル（`~/.config/agent-cli/config.toml`）は触りません

環境変数で挙動をカスタマイズできます。

| 変数 | 既定 | 用途 |
|------|------|------|
| `AGENT_CLI_REPO` | GitHubのソースリポジトリ | クローン元 |
| `AGENT_CLI_REF` | `main` | チェックアウトするref |
| `AGENT_CLI_PREFIX` | `$HOME/.local` | インストールprefix |
| `AGENT_CLI_INSTALL_FORCE` | （未設定） | `1`で上書き告知を抑制 |

### 手動ビルド

```bash
git clone https://github.com/aquaxis/agent-cli.git
cd agent-cli
cargo install --path . --root "$HOME/.local"
```

## クイックスタート

```bash
# 1. 初回起動で既定の設定ファイルが生成される
agent-cli config path
# => ~/.config/agent-cli/config.toml

# 2. APIキーを環境変数に設定（claudeを使う場合）
export ANTHROPIC_API_KEY=sk-...

# 3. REPLを起動（引数なし `agent-cli` も `agent-cli run` と等価）
agent-cli                       # 設定ファイルの provider.kind を使う
# あるいは
agent-cli run --provider claude # 上書き指定

# 4. 別ターミナルでollamaバックエンドの2人目を起動
agent-cli run --provider ollama --model glm-5.1:cloud --name bob

# 5. 1つ目のターミナルから2つ目へプロンプト送信
> /list
> /send bob "hello from claude side"

# 6. REPLを抜ける
> /quit       # または /exit、Ctrl+D、Ctrl+C のいずれでも可
```

## 設定方法

設定ファイルはTOMLで、解決優先順位は以下です。

1. `--config <path>`で明示指定
2. 環境変数`AGENT_CLI_CONFIG`
3. 既定`~/.config/agent-cli/config.toml`

明示指定したパスが存在しない場合はエラー終了します。既定パスのみ未存在時に自動生成されます。

最低限必要な編集は次のとおりです。

```toml
[provider]
kind = "claude"   # "claude" | "codex" | "ollama" | "llama.cpp"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"   # 環境変数名を指定（値そのものではない）
model       = "claude-opus-4-7"

[provider.ollama]
base_url = "http://127.0.0.1:11434"
model    = "glm-5.1:cloud"
```

複数プロファイルを使い分けるには、`--config`で別ファイルを指定して並行起動してください。各エージェントは`[runtime] registry_dir`を共有すれば相互にピア検出が可能です。

実際にどの設定ファイルが採用されているかは `agent-cli config path` で確認できます。プロバイダ HTTP エラー時のメッセージにも `config` 行として解決済みパスが表示されるので、`~/.local/config/...` と `~/.config/...` の混同などを切り分けるときに役立ちます。

詳しくは[`doc/config.md`](doc/config.md)を、よくあるエラーへの対処は[`doc/troubleshooting.md`](doc/troubleshooting.md)を参照してください。

## 主要コマンド

| コマンド | 用途 |
|---------|------|
| `agent-cli run` | REPLを起動（エージェント1つ） |
| `agent-cli list` | 稼働中のピア一覧 |
| `agent-cli send <peer> <text>` | 指定ピアにプロンプト送信 |
| `agent-cli providers` | 利用可能バックエンドの状態表示 |
| `agent-cli doctor` | 設定／APIキー／疎通／レジストリ／bashの一括点検 |
| `agent-cli selftest [--provider <name>]` | スモークテスト |
| `agent-cli config show` | 設定表示 |
| `agent-cli config edit` | エディターで設定を開く |
| `agent-cli config path` | 設定ファイルのパス表示 |

REPL内では以下の`/`コマンドが使えます。

| コマンド | 用途 |
|---------|------|
| `/list` | 稼働中のピア一覧 |
| `/send <peer> <text>` | 指定ピアへプロンプト送信 |
| `/tools` | 有効なツール名一覧 |
| `/persona` | 自身のペルソナ表示 |
| `/reload-persona` | ペルソナ再読込（履歴は維持） |
| `/peer <id_or_name>` | 指定ピアのペルソナ概要 |
| `/history [n]` | 最近の入力履歴 |
| `/clear`、`/reset` | 会話履歴を初期化（ペルソナは維持） |
| `/cancel` | 進行中処理の中断要求 |
| `/auto [on\|off\|status]` | ツール承認スキップの実行時切替 |
| `/help` | 一覧表示 |
| `/quit`、`/exit` | 終了（互いに完全エイリアス） |

詳細は[`doc/usage.md`](doc/usage.md)を参照。

### ツール承認をスキップする

シェル等のツール呼び出しは既定でy/N承認を求めます。承認スキップ（自動許可）に切り替える経路は3つあります。

| 経路 | 例 |
|------|-----|
| 設定ファイル | `[runtime] auto_approve_tools = true` |
| CLIフラグ | `agent-cli run --auto-approve-tools` |
| REPLコマンド | `/auto on`（`/auto off`で承認モードへ復帰、`/auto status`で現在値表示） |

承認モードのまま実行されたツール要求は`[tool approval] <tool> <args>` バナーと`approve? [y/N]: `を表示します。`y`／`yes`のみ承認、それ以外（空入力や別単語）は拒否扱いです。

### `[thinking]` 表示の抑制

`glm-5.1:cloud` のような長尺 reasoning モデルは大量の thinking トークンを返すため、REPL が `[thinking] ...` で埋まることがあります。`[ui] show_thinking` で表示量を 3 段階で制御できます。

```toml
[ui]
show_thinking = "hidden"     # 完全に抑制
# show_thinking = "collapsed"  # 既定。先頭 80 文字 + "..." の 1 行
# show_thinking = "expanded"   # 全文表示
```

| 値 | 挙動 |
|----|------|
| `"hidden"` | `[thinking]` を一切表示しない |
| `"collapsed"`（既定） | 各 thinking delta を「先頭 80 文字 + `...`」に切り詰め、改行があれば 1 行目のみ |
| `"expanded"` | 受信 text を全文表示 |

設定変更は `agent-cli` 再起動で反映されます。詳細は [`doc/config.md`](doc/config.md) の「UI 表示モード」を参照。

### `[info] max tool-use iterations reached` の意味

REPL でこのメッセージが出るのは、AI が 1 回のユーザー入力に対して **ツール実行（tool_use）を上限回連続して繰り返しても結論に到達できなかった** ときです（無限ループ防止のための防護機構）。

- これは **エラーではなく情報通知**（`[info]` プレフィックス）です。`[error]` ではないので、エラーログ／監視警報には残りません。
- 直後にプロンプト `> ` が再描画され、次の入力を受け付けます。会話履歴は維持されます。
- **設定ファイルで変更できますか？** はい。`~/.config/agent-cli/config.toml` の `[runtime] max_tool_iterations` で変更可能（既定 24）。変更は `agent-cli` 再起動で反映。
- **無制限の設定は可能ですか？** 厳密な「無制限」は不可（API 課金・GPU 占有・stdout 占有の暴走防止のため意図的に制限）。型は `u32` のため最大 `u32::MAX = 4,294,967,295` まで設定可能で、実用上はこれで「無制限相当」です。
- 推奨レンジ：単純対話 4-8、design-then-debug 系オーケストレーター 24-48、長尺自律実行 64-256。
- 対処：プロンプトを分割する／意図を具体化する／不要ツールを `denied_tools` で除外する／`/clear` で履歴をリセットして再試行する／`max_tool_iterations` を引き上げる。詳細は [`doc/troubleshooting.md`](doc/troubleshooting.md) ／ [`doc/config.md`](doc/config.md) を参照。

### 終了方法

以下のいずれでも確実に終了し、IPCソケット（`<registry_dir>/<agent-id>.sock`）とレジストリメタ（`<registry_dir>/<agent-id>.json`）を自動削除します。応答ストリーミング中／ツール実行中であっても1秒以内に終了します。

| 経路 | 操作 |
|------|------|
| REPLコマンド | `/quit` または `/exit` |
| EOF | `Ctrl+D`（標準入力の終端） |
| シグナル | `Ctrl+C`（SIGINT）または `kill <pid>`（SIGTERM） |

## 検証

```bash
# 自動テスト（80件）
cargo test

# フォーマット／リント
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

# 自己診断
agent-cli doctor

# スモークテスト（5ステージ：Provider／shellツール／IPC／子プロセス起動／子プロセスAI応答）
agent-cli selftest --provider claude
agent-cli selftest --provider ollama

# 半自動受け入れシナリオ（環境変数の有無で SKIP/PASS/FAIL を集計）
scripts/manual_acceptance.sh
```

## ペルソナ

エージェントの役割（`role`）／スキル／説明／使用ツール（`allowed_tools` / `denied_tools`）／モデル／温度を、Markdown（YAMLフロントマター＋本文）で定義できます。サンプルは`example/agents/`に同梱しています。

```bash
mkdir -p ~/.config/agent-cli/agents
cp example/agents/reviewer.md ~/.config/agent-cli/agents/alice.md
agent-cli run --name alice
# → <agents_dir>/alice.md が自動的に読み込まれる
```

解決優先順位は **`--persona <path>` → `[runtime] persona_file` → `<agents_dir>/<name>.md` → 組み込み既定** の順。最小サンプル：

```markdown
---
name: alice
role: コードレビュアー
skills: [Rust, セキュリティ]
allowed_tools: [shell, fs_read]
denied_tools:  [fs_write]
---

あなたは熟練のレビュアーです。最小差分で修正案を提示してください。
```

詳細な記法・フロントマター全キー・運用シナリオは [`doc/personas.md`](doc/personas.md) を参照。

## ドキュメント目次

- [`doc/usage.md`](doc/usage.md) — CLI／REPL コマンド詳細
- [`doc/config.md`](doc/config.md) — 設定リファレンス（最詳細）
- [`doc/personas.md`](doc/personas.md) — ペルソナリファレンス（フロントマター全キー・運用シナリオ）
- [`doc/tools.md`](doc/tools.md) — 内蔵ツール仕様
- [`doc/architecture.md`](doc/architecture.md) — アーキテクチャ概要
- [`doc/troubleshooting.md`](doc/troubleshooting.md) — 既知の失敗と対処
- [`doc/providers/claude.md`](doc/providers/claude.md)／[`codex.md`](doc/providers/codex.md)／[`ollama.md`](doc/providers/ollama.md)／[`llamacpp.md`](doc/providers/llamacpp.md) — バックエンド別ガイド
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — 開発参加ガイド
- [`CHANGELOG.md`](CHANGELOG.md) — 変更履歴

## ライセンス

MIT License. 詳細は [`LICENSE`](LICENSE) を参照。
