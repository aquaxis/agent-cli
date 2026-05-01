# 実装タスクリスト（AI_PRJ_TASKS）

本ファイルは`agent-cli`（単独起動・1プロセス1エージェント・複数AIバックエンド対応のRust製CLI）の実装タスクを定義する。

ステータス凡例：`[ ]` 未着手 / `[~]` 着手中 / `[x]` 完了

---

## フェーズ0：プロジェクト基盤

### T-000 Cargoプロジェクト初期化

- [ ] `cargo init --name agent-cli`相当でクレート構造を作成
- [ ] `Cargo.toml`に基本メタデータ（version、edition=2021、license など）
- [ ] `rust-toolchain.toml`で安定版を固定
- [ ] `.gitignore`（`target/` 等）
- 検証：`cargo check`がエラーなく通る。

### T-001 依存クレート追加

- [ ] `clap`（derive）、`tokio`（full）、`serde`、`serde_json`、`toml`
- [ ] `reqwest`（rustls）、`async-trait`、`thiserror`、`anyhow`
- [ ] `tracing`、`tracing-subscriber`
- [ ] `crossterm`、`chrono`、`ulid`
- 検証：`cargo build`成功。

### T-002 ロギング初期化

- [ ] `tracing_subscriber`の`EnvFilter`ベース初期化を`main`で行う
- [ ] `RUST_LOG`で詳細レベル制御
- 検証：`RUST_LOG=debug agent-cli --help`でログレベルが反映される。

---

## フェーズ1：CLIと設定

### T-100 CLI引数定義（`cli.rs`）

- [ ] `clap` deriveで`Cli`と`Command` enumを定義
- [ ] グローバルオプション：`--config <path>`（全サブコマンドで利用）
- [ ] サブコマンド：`run`、`list`、`send`、`providers`、`doctor`、`selftest`、`config`
- [ ] `run`の引数：`--name`、`--provider`、`--model`、`--persona`
- [ ] `config`サブコマンドに`show`／`edit`／`path`を追加
- 検証：`agent-cli --help`、`agent-cli run --help`、`agent-cli --config /tmp/x.toml run --help`が期待通りに表示される。

### T-101 設定ファイル（`config.rs`）

- [ ] `Config`構造体（`provider`、`provider.<kind>`セクション、`runtime`、`tools`、`ui`）
- [ ] 設定ファイル解決：`--config` → `AGENT_CLI_CONFIG` → `~/.config/agent-cli/config.toml`の優先順位
- [ ] 明示指定パスが存在しない場合はエラー終了、既定パスかつ未存在のときのみ既定値で生成
- [ ] バックエンド別セクション（`claude`／`codex`／`ollama`／`llama.cpp`）の解析
- [ ] CLIオプションによる`provider.kind`／モデル名の上書き
- [ ] `agent-cli config show`／`agent-cli config edit`／`agent-cli config path`を実装
- [ ] `~`、環境変数、相対パスの正規化
- 検証：
  - 既定パス未存在時に初回実行で自動生成される。
  - `--config /tmp/missing.toml`はエラー終了する。
  - 異なる`--config`で2プロセスを起動でき、それぞれ独立して動作する。
  - `agent-cli config path`が解決済み絶対パスを表示する。

### T-102 エラー型（`error.rs`）

- [ ] `thiserror`で`AppError`を定義し、`Config`／`Provider`／`Tool`／`Ipc`／`Registry`／`Ui`等のvariantを整備
- 検証：各モジュールが`Result<_, AppError>`を返す形に統一できる。

---

## フェーズ2：AIプロバイダー抽象とバックエンド実装

### T-200 Provider trait（`ai/mod.rs`）

- [ ] `trait Provider`：`name`、`capabilities`、`complete_stream`
- [ ] `Capabilities`、`ProviderEvent`、`Message`、`ToolSpec`の共通型
- [ ] ファクトリ`ai::build(&Config) -> Box<dyn Provider>`
- 検証：ダミー実装でユニットテストが通る。

### T-201 Claudeバックエンド（`ai/claude.rs`）

- [ ] `reqwest`でAnthropic Messages APIへのSSEリクエスト
- [ ] thinkingブロック・tool_useブロックのパース
- [ ] APIキーは`provider.claude.api_key_env`で指定された環境変数から取得
- 検証：モックHTTPでSSE応答を流し、`ProviderEvent`列に変換できる。

### T-202 Codex（OpenAI）バックエンド（`ai/codex.rs`）

- [ ] OpenAI Chat Completions API（streaming、function calling）に対応
- [ ] APIキーは`provider.codex.api_key_env`から取得
- [ ] function call/tool call → `ProviderEvent::ToolUse`へ正規化
- [ ] thinkingは未対応として`Capabilities::thinking=false`
- 検証：モックHTTPで応答を再現し、tool callの正規化を検証。

### T-203 Ollamaバックエンド（`ai/ollama.rs`）

- [ ] `/api/chat`（NDJSONストリーム）を実装
- [ ] `tools`フィールド対応モデル時のみ`Capabilities::tool_use=true`
- [ ] `base_url`は`http://127.0.0.1:11434`を既定とする
- 検証：モックHTTPでNDJSONを流し、`Text`／`ToolUse`が得られる。

### T-204 llama.cppバックエンド（`ai/llamacpp.rs`）

- [ ] OpenAI互換`/v1/chat/completions`（streaming）を利用
- [ ] tool callはサーバー対応時のみ正規化、未対応時は`Capabilities::tool_use=false`
- 検証：モックHTTPで応答を再現できる。

### T-205 Tool橋渡し（`ai/tool_bridge.rs`）

- [ ] Claude content block ↔ OpenAI tool_calls の双方向変換
- [ ] tool結果（tool_result／tool message）の正規化
- 検証：両形式の入出力で同一の`ProviderEvent`列が得られる。

### T-206 ストリーミング共通処理（`ai/stream.rs`）

- [ ] SSEパーサとNDJSONパーサのユーティリティ
- [ ] バックプレッシャ対応（`tokio::stream`）
- 検証：境界条件（部分受信、空行、不正JSON）の単体テスト。

### T-207 `agent-cli providers`サブコマンド

- [ ] 利用可能なバックエンド一覧と、現行設定での疎通可否を表示
- 検証：APIキー未設定／ローカルサーバー停止時に分かりやすく表示される。

---

## フェーズ3：Tools

### T-300 Tool抽象（`tools/mod.rs`）

- [ ] `trait Tool`と`ToolCtx`／`ToolOutput`
- [ ] レジストリ（名前 → `Box<dyn Tool>`）
- 検証：ダミーツールを登録／解決できる。

### T-301 Shellツール（`tools/shell.rs`）

- [ ] `bash -lc <cmd>`を`tokio::process`で実行
- [ ] stdout／stderr／exit_codeを返す
- [ ] `timeout_secs`／`max_output_kb`の制限
- [ ] 既定でユーザー承認（y/N）。`auto_approve_tools=true`時はスキップ
- 検証：`echo hello`が成功し、タイムアウト・出力サイズ超過のテストが通る。

### T-302 ファイル読み取りツール（`tools/fs_read.rs`）

- [ ] 引数：`path`、任意の`offset`／`limit`
- [ ] バイナリ判定とエラー応答
- 検証：テンポラリディレクトリで単体テスト。

### T-303 ファイル書き込みツール（`tools/fs_write.rs`）

- [ ] 引数：`path`、`content`、`overwrite`（既定false）
- 検証：上書き可否の挙動を単体テストで確認。

### T-304 ピア送信ツール（`tools/send_to.rs`）

- [ ] 引数：`peer`（agent-idまたは表示名）、`text`
- [ ] レジストリで宛先解決し、IPCクライアントで送信
- 検証：2プロセス起動で実機テスト。

### T-305 ツール実行の対話的承認

- [ ] `auto_approve_tools=false`時にy/Nプロンプト
- [ ] 拒否時は「ユーザーが拒否」をAIへ返す
- 検証：拒否経路の結合テスト。

---

## フェーズ4：IPCとレジストリ

### T-400 IPCメッセージ型（`ipc/mod.rs`）

- [ ] `IpcMessage`（`Prompt`／`Ack`／`Ping`／`Pong`）
- [ ] JSON Linesフォーマット
- 検証：シリアライズ往復の単体テスト。

### T-401 IPCサーバー（`ipc/server.rs`）

- [ ] Unixドメインソケットを`<registry_dir>/<agent-id>.sock`にバインド
- [ ] パーミッション0600
- [ ] 受信メッセージを`mpsc`で会話ループへ流す
- 検証：単体テストで`IpcMessage::Prompt`を送受信できる。

### T-402 IPCクライアント（`ipc/client.rs`）

- [ ] 指定ソケットへ接続し`IpcMessage`を送信
- 検証：往復テスト。

### T-403 レジストリ（`ipc/registry.rs`）

- [ ] `<registry_dir>/<agent-id>.json`にメタ情報を書き出し、終了時に削除
- [ ] レジストリ走査：JSONとソケットの存在確認、PID生存確認、staleの掃除
- [ ] `agent-cli list`の出力整形（id・name・provider・model・PID・起動時刻）
- 検証：2プロセス起動時に双方が一覧に現れる。

### T-404 `agent-cli send`サブコマンド

- [ ] `agent-cli send <peer> <text>`でIPCクライアントを直接呼ぶ
- 検証：別プロセスのエージェントへプロンプトが届き、AI応答が観測できる。

---

## フェーズ5：エージェント本体とREPL

### T-500 単一エージェント会話ループ（`agent.rs`）

- [ ] `AgentInput`／`AgentEvent`の処理
- [ ] 会話履歴管理、system promptの設定可
- [ ] Provider呼び出しとtool_use循環
- [ ] `Cancel`での中断
- 検証：モックProviderで往復が成立する。

### T-503 ペルソナ機構（`persona.rs`）

- [ ] YAMLフロントマター＋Markdown本文のパース（`gray_matter`または`serde_yaml`）
- [ ] `PersonaFrontmatter`の検証（必須キー：`role`、`skills`）
- [ ] `Persona::load(path)`／`Persona::builtin_default()`／`to_system_prompt()`／`summary()`
- [ ] 解決優先順位：`--persona` → `[runtime] persona_file` → `<agents_dir>/<name>.md` → 組み込み既定
- [ ] `[runtime] agents_dir`の解析（既定`~/.config/agent-cli/agents/`）
- [ ] `allowed_tools`／`denied_tools`をツールレジストリへ反映
- [ ] `model`／`temperature`をProvider設定に反映
- [ ] レジストリメタ（`.json`）に`PersonaSummary`を記録
- [ ] REPLヘッダーに`name`／`role`／`skills`を表示
- [ ] REPLコマンド`:persona`／`:reload-persona`／`:peer <id>`
- [ ] サンプルペルソナ（`example/agents/coder.md`／`reviewer.md`／`planner.md`）を同梱
- 検証：
  - サンプルペルソナで起動し、システムプロンプトに反映される。
  - `:reload-persona`で会話履歴を保ったまま再読込される。
  - `:peer`で他プロセスのペルソナ概要が確認できる。
  - `allowed_tools`／`denied_tools`がツール実行可否に反映される。

### T-501 REPLフロントエンド（`app.rs`）

- [ ] `crossterm`での行入力（履歴、複数行貼り付け対応）
- [ ] `:`コマンドDispatcher（`:list`／`:send`／`:tools`／`:cancel`／`:help`／`:quit`）
- [ ] 標準入力とIPC受信を`tokio::select!`で合流
- [ ] thinking／text／tool_call／tool_resultの差別化された表示
- 検証：手動でREPL動作を確認。

### T-502 ログ出力（`log.rs`）

- [ ] `<log_dir>/<agent-id>/<timestamp>.jsonl`へ1イベント1行で書き出し
- [ ] user／assistant／thinking／tool_call／tool_result／peer_promptを区別
- 検証：実行後にログ内容が確認できる。

---

## フェーズ6：結合テスト・受け入れ

### T-600 結合テスト

- [ ] モックProvider＋テンポラリ`registry_dir`でE2Eテスト
- [ ] 2プロセス相当のテストハーネスでIPC往復を検証
- 検証：`cargo test`が全て通る。

### T-601 受け入れシナリオ（手動）

完成判定の必須対象は`claude`と`ollama`（モデル：`glm-5.1:cloud`）の2バックエンド。`codex`／`llama.cpp`は任意検証。

#### T-601-A claude単独（必須）

- [ ] `agent-cli run --provider claude`で起動して対話できる
- [ ] `agent-cli doctor`が終了コード0
- [ ] `agent-cli selftest --provider claude`が終了コード0
- [ ] シェルツール経由で`ls`／`echo`等が実行できる

#### T-601-B ollama単独（必須）

- [ ] `agent-cli run --provider ollama --model glm-5.1:cloud`で起動して対話できる
- [ ] `agent-cli doctor`が終了コード0
- [ ] `agent-cli selftest --provider ollama`が終了コード0
- [ ] シェルツール経由のコマンド実行（tool_use非対応時の代替挙動も記録）

#### T-601-C claude × ollama 2プロセス協調（必須）

- [ ] ターミナルAで`--provider claude --name alice`、ターミナルBで`--provider ollama --model glm-5.1:cloud --name bob`を起動
- [ ] 両プロセスで同一`registry_dir`を共有
- [ ] `:list`に2プロセスが表示される
- [ ] Aから`:send bob "hello"`でBへ送信、Bがollamaで応答
- [ ] Bから`:send alice "..."`でAへ送信、Aがclaudeで応答

#### T-601-D 任意検証（記録のみ）

- [ ] `--provider codex`での対話／selftest（任意）
- [ ] `--provider llama.cpp`での対話／selftest（任意）

#### T-601-E 検証結果記録

- [ ] 各シナリオのバックエンド・モデル・コミットハッシュ・日時・合否を作業ログに記録

### T-602 ドキュメント整備（FR-10）

ドキュメントの作成・更新は、機能を追加するPRと同じPR内で行うことを原則とする。

#### T-602-1 README.md

- [ ] プロジェクト概要・特徴・対応バックエンド早見表
- [ ] インストール（バイナリ取得／ソースビルド）
- [ ] クイックスタート（5分手順）
- [ ] **設定方法セクション**（自動生成ファイル、最低限編集項目、コピペサンプル、`--config`／`AGENT_CLI_CONFIG`の使い分け、複数プロファイル例）
- [ ] 主要コマンド早見表
- [ ] 検証手順（`cargo test`／`agent-cli doctor`／`agent-cli selftest`）
- [ ] ドキュメント目次（`doc/`配下リンク）
- [ ] 英語版`README.en.md`を併設
- 検証：未経験者がREADMEのみでセットアップから検証完了まで到達できる。

#### T-602-2 doc/usage.md

- [ ] 各サブコマンド（`run`／`list`／`send`／`providers`／`doctor`／`selftest`／`config`）の詳細
- [ ] REPLコマンド（`:list`／`:send`／`:tools`／`:cancel`／`:help`／`:quit`）の解説
- [ ] ユースケース：単独対話、2プロセス協調、ローカルLLM接続
- 検証：すべてのコマンドのオプションと出力例が網羅されている。

#### T-602-3 doc/config.md（最重要）

- [ ] 設定ファイル解決順序の図解（`--config` → `AGENT_CLI_CONFIG` → 既定パス）
- [ ] 全体構造図と各セクションの役割
- [ ] 全項目リファレンス（キー／型／既定／許容値／必須・任意／例／注意）の表
- [ ] 完全サンプル3種（最小／推奨／全機能有効）
- [ ] APIキー管理（環境変数、`api_key_env`、`.envrc`、`systemd EnvironmentFile`の例）
- [ ] 複数プロファイル運用（`registry_dir`分離・共有）
- [ ] シェルツールチューニング（`timeout_secs`／`max_output_kb`）
- [ ] UI表示モード（`ui.show_thinking`）
- [ ] よくある設定ミスと`agent-cli doctor`の読み方
- [ ] 設定変更の反映と再起動の要否
- 検証：設計書11.1の章立てを満たし、各項目に具体例がある。

#### T-602-4 doc/providers/{claude,codex,ollama,llamacpp}.md

各バックエンドにつき：

- [ ] 前提条件（アカウント、APIキー発行、ローカルサーバー導入）
- [ ] 認証情報設定方法
- [ ] 推奨モデルと用途別の選び方
- [ ] `base_url`の指定（プロキシ／互換サーバー）
- [ ] 対応機能マトリクス（thinking／tool_use／streaming）
- [ ] `agent-cli doctor`／`selftest --provider`での確認手順
- [ ] 既知の制限・トラブルシューティング
- 検証：4バックエンドすべてで同等水準の情報が揃っている。

#### T-602-5 doc/tools.md

- [ ] `shell`／`fs_read`／`fs_write`／`send_to`の引数スキーマ・戻り値・制限・承認フロー
- [ ] 拒否時の挙動とAIへの返却形式
- 検証：各ツールの入出力JSONサンプルが掲載されている。

#### T-602-6 doc/architecture.md

- [ ] システム構成図（プロセス・IPC・レジストリ）
- [ ] Provider抽象とtool橋渡しの概要
- [ ] データフロー（ユーザー入力／IPC受信から応答描画まで）
- 検証：`AI_PRJ_DESIGN.md`を読まなくても全体像が把握できる粒度。

#### T-602-7 doc/troubleshooting.md

- [ ] APIキー未設定／間違い／レート超過
- [ ] Ollama未起動／llama.cppサーバー未起動
- [ ] ソケット権限・stale掃除
- [ ] レジストリ衝突（同一`registry_dir`での競合）
- [ ] シェルツールのタイムアウト・出力サイズ超過
- 検証：症状から原因と対処までたどれる構成。

#### T-602-8 CONTRIBUTING.md／CHANGELOG.md／LICENSE

- [ ] `CONTRIBUTING.md`：開発環境構築、`cargo fmt`／`clippy`／`test`、新バックエンド／新ツール追加手順、PR作法、ドキュメント同時更新の必須化
- [ ] `CHANGELOG.md`：Keep a Changelog形式、SemVer、初回エントリを記載
- [ ] `LICENSE`：採用ライセンス全文（既定はMIT）
- 検証：3ファイルがリポジトリルートに揃っている。

#### T-602-9 rustdocコメント

- [ ] 公開API（`Provider`／`Tool`／`Config`／`AgentId`／IPCメッセージ型）に`///`コメント
- [ ] `cargo doc --no-deps`が警告なしで完走
- 検証：生成ドキュメントで主要型の説明が読める。

#### T-602-10 ドキュメント整合性チェック

- [ ] サンプル`config.toml`がTOMLパーサで通る（テストで担保）
- [ ] サンプルコマンドの`--help`出力との不整合がない
- [ ] 全ドキュメントのリンク切れチェック
- 検証：CIまたはローカルスクリプトでチェックが自動化されている。

---

## フェーズ7：完成後検証（FR-09）

完成判定はこのフェーズの全タスクが成功することをもって行う。

### T-700 doctorサブコマンド実装

- [ ] 設定ファイルの存在・パース確認
- [ ] 選択中バックエンドのAPIキー（環境変数）存在確認
- [ ] バックエンド疎通確認（HTTPで軽量リクエスト）
- [ ] レジストリディレクトリ・ログディレクトリの書き込み可否
- [ ] `bash`存在確認とシェルツールの起動可否
- [ ] 各項目のOK／NG表示と対処ヒント、終了コード制御
- 検証：意図的にAPIキー未設定／ローカルサーバー停止等を起こし、NGとヒントが表示される。

### T-701 selftestサブコマンド実装

- [ ] 一時的な隔離`registry_dir`／`log_dir`を準備
- [ ] 短いプロンプトを送り応答内容を検証
- [ ] シェルツール実行プロンプトを送り、tool_use一巡が成功することを検証
- [ ] サブプロセスを起動し、IPC経由のプロンプト授受の成功を検証
- [ ] 終了コードで合否を返す
- 検証：成功時exit=0、いずれかが失敗するとexit≠0で原因が表示される。

### T-702 自動テスト網羅性確認

- [ ] `cargo test`がすべてパス
- [ ] `cargo fmt --check`通過
- [ ] `cargo clippy -- -D warnings`通過
- 検証：CI設定（任意）またはローカルで連続実行が安定すること。

### T-703 完成検証レポート

- [ ] `agent-cli doctor`／`agent-cli selftest`を`claude`と`ollama (glm-5.1:cloud)`の双方で実行し結果を作業ログに記録
- [ ] T-601-A／B／C（必須シナリオ）の合否を作業ログに記録
- [ ] T-601-D（任意シナリオ）を実施した場合はその結果も併記
- [ ] 失敗があった場合は再修正のうえ再実行し、最終的に必須項目すべて成功のログを残す
- 検証：作業ログに「必須項目（claude／ollama）全成功」が記録されていること。

---

## マイルストーン

| マイルストーン | 含まれるフェーズ | 目標 |
|----------------|------------------|------|
| M1（最小動作） | フェーズ0〜2のうちClaudeのみ | 単独プロセスでClaude対話できる |
| M2（ツール） | フェーズ3 | shell／fs／send_toツールが動作する |
| M3（協調） | フェーズ4〜5 | 2プロセス間でプロンプト授受ができる |
| M4（マルチバックエンド） | フェーズ2のCodex／Ollama／llama.cpp | 4バックエンドを切替可能 |
| M5（リリース） | フェーズ6 | 受け入れシナリオ通過とドキュメント整備 |
| M6（完成検証） | フェーズ7 | `doctor`／`selftest`／自動テストがすべて成功 |

## 備考

- 実装の各ステップは`/ai`コマンドで開始される。本ファイルは進捗に応じて`/update_ai`で更新する。
- 書き込み制約に従い、本セッションではRustソースファイル等の生成は行わない。
