# Changelog

[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) 形式、[Semantic Versioning](https://semver.org/lang/ja/) に準拠する。

## [Unreleased]

### Added

- 初版リリースに向けた骨格実装：
  - 単独起動の Rust 製 CLI（`agent-cli`）
  - REPL ＋ tools 機能 ＋ thinking 表示（Claude Code 相当）
  - 4 バックエンド：`claude` / `codex` / `ollama` / `llama.cpp`
    - 各バックエンドのストリームパースを純関数として切り出し、モック入力で単体テスト
    - ペルソナの `model` / `temperature` をリクエスト body に反映
  - 内蔵ツール：`shell` / `fs_read` / `fs_write` / `send_to`
  - エージェント間メッセージング（Unix ドメインソケット、JSON Lines）
  - レジストリ（`<registry_dir>/<agent-id>.{sock,json}`、PID 生存確認、stale 掃除）
  - エージェントペルソナファイル（YAML フロントマター ＋ Markdown 本文）
    - 解決順序：`--persona` → `[runtime] persona_file` → `<agents_dir>/<name>.md` → 組み込み既定
    - REPL コマンド：`/persona` / `/reload-persona` / `/peer <id>` / `/tools`
  - 設定ファイル `~/.config/agent-cli/config.toml`、`--config` / `AGENT_CLI_CONFIG` で個別指定
  - 自己診断 `agent-cli doctor`
  - スモークテスト `agent-cli selftest`（4 ステージ）
    - Stage 1：Provider "OK" 往復
    - Stage 2：シェルツール直接実行
    - Stage 3：IPC ラウンドトリップ
    - Stage 4：子プロセスを起動してレジストリ登録 + Ping/Pong
  - ワンライナー対応 `install.sh`
  - サンプルペルソナ：`example/agents/{coder,reviewer,planner}.md`
  - ドキュメント：`README.md` / `README.en.md` / `doc/` 配下 / `CONTRIBUTING.md` / `CHANGELOG.md` / `LICENSE`
  - GitHub Actions CI（`.github/workflows/ci.yml`）：fmt / clippy / build / test / doc / selftest を自動化
  - 入力履歴の永続化（`<log_dir>/history.txt`、最終 200 件）と REPL `/history [n]` コマンド
  - `agent-cli list` のカラム整列出力
  - 受け入れシナリオ半自動実行スクリプト `scripts/manual_acceptance.sh`
    - 必須 A（claude）／B（ollama）、任意 D1（codex）／D2（llama.cpp）に対応
    - API キー／ローカルサーバー有無で SKIP を自動判定

### Verification

- `cargo build` 警告ゼロ
- `cargo clippy --all-targets -- -D warnings` 通過
- `cargo fmt --all -- --check` 通過
- `cargo test` 全 41 テスト成功（Provider パーサ、Agent ループ E2E、IPC、ペルソナ、ドキュメント整合性、CLI 整合性）
- `cargo doc --no-deps` 警告ゼロ

[Unreleased]: https://github.com/example/agent-cli/compare/HEAD...HEAD
