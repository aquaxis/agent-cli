# コントリビューションガイド

`agent-cli` への貢献を歓迎します。本ドキュメントは開発参加時の基本ルールをまとめます。

## 前提環境

- Linux（x86_64／aarch64）
- Rust 安定版（`rust-toolchain.toml` で固定）
- `cargo`／`git`／`bash`

## 開発フロー

```bash
# 1. クローンして
git clone https://github.com/aquaxis/agent-cli.git
cd agent-cli

# 2. ビルド／テスト
cargo build
cargo test

# 3. lint と整形
cargo fmt --all
cargo clippy --all-targets -- -D warnings

# 4. ローカル動作確認
cargo run --quiet -- providers
cargo run --quiet -- doctor
```

PR を作成する前に、以下が成功していることを確認してください。

- `cargo build`（警告ゼロ）
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo fmt --check`
- `cargo doc --no-deps`（公開 API に rustdoc コメントが付いていること）

実環境（API キーやローカル LLM サーバー）が手元にある場合は、`./scripts/manual_acceptance.sh` を併走させてください。
SKIP / PASS / FAIL を集計し、終了コードで失敗を検出します。

```bash
ANTHROPIC_API_KEY=sk-ant-... \
  OPENAI_API_KEY=sk-...     \
  OLLAMA_URL=http://127.0.0.1:11434 \
  ./scripts/manual_acceptance.sh
```

## ドキュメント運用ルール

機能追加・変更・廃止を伴う PR では、以下のうち関連するものを **同じ PR 内で** 更新してください。

- `README.md`（クイックスタート・主要コマンド・設定方法・対応バックエンド表）
- `doc/usage.md`（コマンド・REPL 仕様）
- `doc/config.md`（設定キーの追加／変更）
- `doc/providers/<kind>.md`（バックエンド固有の挙動）
- `doc/tools.md`（ツール仕様）
- `doc/troubleshooting.md`（既知の失敗・対処）
- `CHANGELOG.md`（`[Unreleased]` セクションへ Added／Changed／Fixed／Removed のいずれか）
- `rustdoc`（公開 API は `///` 必須）

## 仕様 3 ファイルの取り扱い

`/.aiprj/AI_PRJ_REQUIREMENTS.md`／`AI_PRJ_DESIGN.md`／`AI_PRJ_TASKS.md` は、AI による開発を駆動する仕様ドキュメントです。実装に齟齬がある場合は、**実装よりも仕様を先に修正**するか、同じ PR 内で両方を整合させてください。

## 新しいバックエンドの追加手順

1. `src/ai/<kind>.rs` を新設し、`trait Provider` を実装
2. `src/ai/mod.rs::build` に分岐を追加
3. `src/ai/mod.rs::SUPPORTED` に名前を追加
4. `Cargo.toml` の依存を必要に応じて更新
5. `src/config.rs` の `[provider.<kind>]` セクション既定を追加
6. `doc/providers/<kind>.md` を新設し、認証・推奨モデル・対応機能を記載
7. `src/commands.rs::providers` に表示処理を確認
8. テスト：パーサに対する単体テスト（モック HTTP の流れを `SseAccumulator` で検証）
9. `agent-cli doctor` がエラーなく走ること

## 新しいツールの追加手順

1. `src/tools/<name>.rs` を新設し、`trait Tool` を実装
2. `src/tools/mod.rs::ToolRegistry::build` のテーブルに追加
3. 既定設定 `src/config.rs::DEFAULT_CONFIG` の `tools.enabled` に追加するか検討
4. `doc/tools.md` に引数スキーマ・戻り値・制限を記載
5. テスト：`tokio::test` で正常系／異常系を検証
6. ペルソナの `allowed_tools`／`denied_tools` で制御できることを確認

## コミットメッセージ

簡潔な命令形（英語または日本語）で、PR 単位のスコープを意識してください。例：

- `feat(ollama): add tool_calls parsing`
- `fix(ipc): cleanup stale sockets on PID disappearance`
- `docs(config): explain registry_dir sharing`

## ライセンス

このプロジェクトに貢献いただいたコードは MIT License の下で公開されます。
