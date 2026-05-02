# agent-cli

`agent-cli`は、Claude Code相当のツール／thinking機能を備えたAIエージェントをRust製の単独バイナリで提供するCLIです。tmuxに依存せず、1プロセス＝1エージェントとして起動し、別プロセスのエージェントとローカルIPC（Unixドメインソケット）でプロンプトを送受信できます。

> English version: [`README.en.md`](README.en.md)

## 特徴

- 単独起動：tmux不要。`agent-cli`をシェルから直接実行
- Claude Code相当のREPL＋tools／thinkingを内蔵（自前実装）
- 4バックエンド対応：`claude` / `codex` / `ollama` / `llama.cpp`
- マルチエージェント協調：別プロセス間で`/send <peer> <text>`相互通信
- ペルソナファイル（YAMLフロントマター＋Markdown）で役割・スキル・ツール権限を定義
- シェル／ファイルR/W／ピア送信ツール内蔵
- 自己診断`agent-cli doctor`／スモークテスト`agent-cli selftest`

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

# 3. REPLを起動
agent-cli run --provider claude

# 4. 別ターミナルでollamaバックエンドの2人目を起動
agent-cli run --provider ollama --model glm-5.1:cloud --name bob

# 5. 1つ目のターミナルから2つ目へプロンプト送信
> /list
> /send bob "hello from claude side"
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

詳しくは[`doc/config.md`](doc/config.md)を参照してください。

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

REPL内では`/list` `/send <peer> <text>` `/tools` `/cancel` `/auto [on|off|status]` `/help` `/quit`（エイリアス：`/exit`）などが使えます。

### ツール承認をスキップする

シェル等のツール呼び出しは既定でy/N承認を求めます。承認スキップ（自動許可）に切り替える経路は3つあります。

| 経路 | 例 |
|------|-----|
| 設定ファイル | `[runtime] auto_approve_tools = true` |
| CLIフラグ | `agent-cli run --auto-approve-tools` |
| REPLコマンド | `/auto on`（`/auto off`で承認モードへ復帰、`/auto status`で現在値表示） |

## 検証

```bash
# 自動テスト
cargo test

# 自己診断
agent-cli doctor

# スモークテスト
agent-cli selftest --provider claude
agent-cli selftest --provider ollama
```

## ペルソナ

エージェントの役割・スキル・ツール権限を`~/.config/agent-cli/agents/<name>.md`に定義できます。サンプルは`example/agents/`を参照してください。

```bash
cp example/agents/reviewer.md ~/.config/agent-cli/agents/alice.md
agent-cli run --name alice
```

## ドキュメント目次

- [`doc/usage.md`](doc/usage.md) — CLI／REPL コマンド詳細
- [`doc/config.md`](doc/config.md) — 設定リファレンス（最詳細）
- [`doc/tools.md`](doc/tools.md) — 内蔵ツール仕様
- [`doc/architecture.md`](doc/architecture.md) — アーキテクチャ概要
- [`doc/troubleshooting.md`](doc/troubleshooting.md) — 既知の失敗と対処
- [`doc/providers/claude.md`](doc/providers/claude.md)／[`codex.md`](doc/providers/codex.md)／[`ollama.md`](doc/providers/ollama.md)／[`llamacpp.md`](doc/providers/llamacpp.md) — バックエンド別ガイド
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — 開発参加ガイド
- [`CHANGELOG.md`](CHANGELOG.md) — 変更履歴

## ライセンス

MIT License. 詳細は [`LICENSE`](LICENSE) を参照。
