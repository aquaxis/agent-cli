# 実装タスクリスト（AI_PRJ_TASKS）

本ファイルは`agent-cli`（単独起動・1プロセス1エージェント・複数AIバックエンド対応のRust製CLI）の実装タスクを定義する。

ステータス凡例：`[ ]` 未着手 / `[~]` 着手中 / `[x]` 完了

---

## フェーズ0：プロジェクト基盤

### T-000 Cargoプロジェクト初期化 [x]

- [x] `cargo init --name agent-cli`相当でクレート構造を作成
- [x] `Cargo.toml`に基本メタデータ（version、edition=2021、license など）
- [x] `rust-toolchain.toml`で安定版を固定
- [x] `.gitignore`（`target/` 等）
- 検証：`cargo check`がエラーなく通る。**達成。**

### T-001 依存クレート追加 [x]

- [x] `clap`（derive）、`tokio`（full）、`serde`、`serde_json`、`toml`
- [x] `reqwest`（rustls）、`async-trait`、`thiserror`、`anyhow`、`async-stream`、`futures`
- [x] `tracing`、`tracing-subscriber`
- [x] `crossterm`、`chrono`、`ulid`、`serde_yaml`、`shellexpand`、`dirs`、`tempfile`(dev)
- 検証：`cargo build`成功。**達成。**

### T-002 ロギング初期化 [x]

- [x] `tracing_subscriber`の`EnvFilter`ベース初期化を`main`で行う
- [x] `RUST_LOG`で詳細レベル制御
- 検証：`RUST_LOG=debug agent-cli --help`でログレベルが反映される。**達成。**

---

## フェーズ1：CLIと設定

### T-100 CLI引数定義（`cli.rs`）[x]

- [x] `clap` deriveで`Cli`と`Command` enumを定義
- [x] グローバルオプション：`--config <path>`（全サブコマンドで利用）
- [x] サブコマンド：`run`、`list`、`send`、`providers`、`doctor`、`selftest`、`config`
- [x] `run`の引数：`--name`、`--provider`、`--model`、`--persona`、`--auto-approve-tools`
- [x] `config`サブコマンドに`show`／`edit`／`path`を追加
- 検証：`agent-cli --help`、`agent-cli run --help`等が期待通りに表示される。**達成。**

### T-101 設定ファイル（`config.rs`）[x]

- [x] `Config`構造体（`provider`、`provider.<kind>`セクション、`runtime`、`tools`、`ui`）
- [x] 設定ファイル解決：`--config` → `AGENT_CLI_CONFIG` → `~/.config/agent-cli/config.toml`の優先順位
- [x] 明示指定パスが存在しない場合はエラー終了、既定パスかつ未存在のときのみ既定値で生成
- [x] バックエンド別セクション（`claude`／`codex`／`ollama`／`llama.cpp`）の解析
- [x] CLIオプションによる`provider.kind`／モデル名の上書き
- [x] `agent-cli config show`／`agent-cli config edit`／`agent-cli config path`を実装
- [x] `~`、環境変数、相対パスの正規化（`shellexpand`）
- 検証：
  - [x] 明示指定パス未存在で `error: config file not found` で終了。
  - [x] `cargo test`で `parse_default_config`／`override_provider_and_model` 通過。
  - [x] `agent-cli --config X providers` で別ファイルを利用可能。

### T-102 エラー型（`error.rs`）[x]

- [x] `thiserror`で`AppError`を定義し各種variantを整備（`Config`／`Provider`／`Tool`／`Ipc`／`Registry`／`Persona`／`Ui`／`Agent`／`Io`／`Toml`／`Yaml`／`Json`／`Http`／`Other`／`ConfigNotFound`）
- 検証：全モジュールが`Result<_, AppError>`に統一。**達成。**

---

## フェーズ2：AIプロバイダー抽象とバックエンド実装

### T-200 Provider trait（`ai/mod.rs`）[x]

- [x] `trait Provider`：`name`、`capabilities`、`complete_stream`、`model`
- [x] `Capabilities`、`ProviderEvent`、`Message`、`ToolSpec`の共通型
- [x] ファクトリ`ai::build(&Config) -> Box<dyn Provider>`
- 検証：4 実装すべてが trait を実装しビルド成功。**達成。**

### T-201 Claudeバックエンド（`ai/claude.rs`）[x]

- [x] `reqwest`でAnthropic Messages APIへのSSEリクエスト
- [x] thinkingブロック・tool_useブロックのパース（`content_block_delta` の `text_delta`／`thinking_delta`／`input_json_delta`）
- [x] APIキーは`provider.claude.api_key_env`で指定された環境変数から取得
- [x] `temperature` フィールドを request body に反映
- [x] パース部を純関数 `handle_frame` として切り出し、モック SSE 入力で単体テスト 4 件通過（text 連結／thinking／tool_use／error）
- 検証：実APIキーでの実機検証は手動受け入れシナリオT-601-Aで実施。

### T-202 Codex（OpenAI）バックエンド（`ai/codex.rs`）[x]

- [x] OpenAI Chat Completions API（streaming、function calling）に対応
- [x] APIキーは`provider.codex.api_key_env`から取得
- [x] function call/tool call → `ProviderEvent::ToolUse`へ正規化
- [x] thinkingは未対応として`Capabilities::thinking=false`
- [x] `temperature` フィールドを request body に反映
- [x] パース部を純関数 `handle_codex_frame` として切り出し、モック SSE 入力で単体テスト 4 件通過（text 連結／streaming tool_call accumulation／DONE時の flush／不正JSON）

### T-203 Ollamaバックエンド（`ai/ollama.rs`）[x]

- [x] `/api/chat`（NDJSONストリーム）を実装
- [x] tool calls の `ProviderEvent::ToolUse` 正規化
- [x] `base_url`は`http://127.0.0.1:11434`を既定とする
- [x] `options.temperature` を request body に反映
- [x] パース部を純関数 `parse_ndjson_line` として切り出し、モック NDJSON 入力で単体テスト 4 件通過（text／tool_calls／空行／不正JSON）

### T-204 llama.cppバックエンド（`ai/llamacpp.rs`）[x]

- [x] OpenAI互換`/v1/chat/completions`（streaming）を利用
- [x] tool callの正規化はOpenAI互換実装を再利用
- [x] `temperature` フィールドを request body に反映
- [x] パース部を純関数 `handle_llamacpp_frame` として切り出し、モック SSE 入力で単体テスト 3 件通過（text 連結／空コンテンツ無視／不正JSON）

### T-205 Tool橋渡し（`ai/tool_bridge.rs`）[x]

- [x] Anthropic／OpenAI／Ollama の各ツール定義へ変換するヘルパを提供

### T-206 ストリーミング共通処理（`ai/stream.rs`）[x]

- [x] `SseAccumulator`／`bytes_to_lines` を実装
- 検証：`sse_accumulator_extracts_frames`／`sse_accumulator_handles_partial` が通過。**達成。**

### T-207 `agent-cli providers`サブコマンド [x]

- [x] バックエンド一覧、モデル名、APIキー設定状態を表示
- 検証：実機実行で4バックエンド分が表示される。**達成。**

---

## フェーズ3：Tools

### T-300 Tool抽象（`tools/mod.rs`）[x]

- [x] `trait Tool`と`ToolCtx`／`ToolOutput`
- [x] `ToolRegistry::build`が`tools.enabled`／`allowed_tools`／`denied_tools`を反映
- 検証：4ツールを正しく登録・解決できる。**達成。**

### T-301 Shellツール（`tools/shell.rs`）[x]

- [x] `bash -lc <cmd>`を`tokio::process`で実行
- [x] stdout／stderr／exit_codeをJSONで返す
- [x] `timeout_secs`／`max_output_kb`の制限
- 検証：`echo_works`／`timeout_triggers` がtest通過。**達成。**

### T-302 ファイル読み取りツール（`tools/fs_read.rs`）[x]

- [x] `path`／`offset`／`limit`引数
- [x] UTF-8 検証

### T-303 ファイル書き込みツール（`tools/fs_write.rs`）[x]

- [x] `path`／`content`／`overwrite`（既定false）

### T-304 ピア送信ツール（`tools/send_to.rs`）[x]

- [x] レジストリで宛先解決し、IPC クライアントで送信、`Ack`／`Error`を結果に反映

### T-305 ツール実行の対話的承認 [x]

- [x] 既定で標準入力からy/N承認、`auto_approve_tools=true`時にスキップ
- [x] 拒否時は「user denied tool execution」を AI に返す

---

## フェーズ4：IPCとレジストリ

### T-400 IPCメッセージ型（`ipc/mod.rs`）[x]

- [x] `IpcMessage`（`Prompt`／`Ack`／`Error`／`Ping`／`Pong`）
- [x] JSON Linesフォーマット

### T-401 IPCサーバー（`ipc/server.rs`）[x]

- [x] Unixドメインソケットを`<registry_dir>/<agent-id>.sock`にバインド
- [x] パーミッション0600
- [x] 受信メッセージを`mpsc`で会話ループへ流す
- 検証：`server_receives_prompt`テスト通過。**達成。**

### T-402 IPCクライアント（`ipc/client.rs`）[x]

- [x] 指定ソケットへ接続し`IpcMessage`を送受信

### T-403 レジストリ（`ipc/registry.rs`）[x]

- [x] `<registry_dir>/<agent-id>.json`にメタ情報を書き出し、終了時に削除
- [x] レジストリ走査：JSON／ソケット存在確認、`/proc/<pid>` でPID生存確認、staleの掃除
- [x] `agent-cli list`の出力整形（id・name・provider・model・role・skills）

### T-404 `agent-cli send`サブコマンド [x]

- [x] `agent-cli send <peer> <text>`でIPCクライアントを直接呼ぶ

---

## フェーズ5：エージェント本体とREPL

### T-500 単一エージェント会話ループ（`agent.rs`）[x]

- [x] `AgentInput`／`AgentEvent`の処理
- [x] 会話履歴管理、ペルソナ由来のシステムプロンプト
- [x] Provider呼び出しとtool_use循環（最大8反復）
- [x] `Cancel`受信で情報イベントを発行
- 検証：実機検証は手動受け入れシナリオT-601で実施。

### T-503 ペルソナ機構（`persona.rs`）[x]

- [x] YAMLフロントマター＋Markdown本文のパース（`serde_yaml`）
- [x] `PersonaFrontmatter`の検証（必須キー：`role`）
- [x] `Persona::load(path)`／`Persona::builtin_default()`／`to_system_prompt()`／`summary()`
- [x] 解決優先順位：`--persona` → `[runtime] persona_file` → `<agents_dir>/<name>.md` → 組み込み既定
- [x] `[runtime] agents_dir`の解析（既定`~/.config/agent-cli/agents/`）
- [x] `allowed_tools`／`denied_tools`をツールレジストリへ反映
- [x] レジストリメタ（`.json`）に`PersonaSummary`を記録
- [x] REPLヘッダーに`name`／`role`／`skills`を表示
- [x] サンプルペルソナ（`example/agents/coder.md`／`reviewer.md`／`planner.md`）を同梱
- [x] REPLコマンド`/persona`／`/reload-persona`／`/peer <id>`／`/tools`を実装
- [x] `model`／`temperature`をProvider設定に反映（`apply_persona_overrides`、4 バックエンドのリクエスト body へ反映）
- 検証：
  - `parse_persona_file`／`builtin_used_when_nothing_specified` テスト通過
  - `persona_overrides_apply_to_active_provider` テスト通過
  - 実機 REPL で `/persona`／`/tools`／`/list`／`/peer` 出力確認済み

### T-501 REPLフロントエンド（`app.rs`）[x]

- [x] 行入力（`tokio::io::BufReader::lines`）
- [x] `/`コマンドDispatcher（`/list`／`/send`／`/tools`／`/persona`／`/reload-persona`／`/peer`／`/history`／`/cancel`／`/help`／`/quit`）
- [x] 標準入力とIPC受信を別タスクで合流（`mpsc`経由）
- [x] thinking／text／tool_call／tool_resultの差別化された表示
- [x] 入力履歴の永続化（`<log_dir>/history.txt`、最終200件）と `/history [n]` コマンドでの参照
- [ ] `crossterm`矢印キーによるインライン履歴ナビゲーション（ストリーミング表示との競合があるため未対応、優先度低）
- 別ホスト検証（FR-09-2）で発見された `/quit`／`Ctrl+D` 終了不具合は T-504 で修正済み（2026-05-02）。
- 検証：手動でREPL動作を確認。`/history` で永続化した履歴が表示されることを確認済み。

### T-502 ログ出力（`log.rs`）[x]

- [x] `<log_dir>/<agent-id>/<timestamp>.jsonl`へ1イベント1行で書き出し
- [x] user／assistant／thinking／tool_call／tool_result／peer_promptを区別

### T-507 REPL UX 仕上げ：`/exit` エイリアスと改行・プロンプト再描画（FR-13／FR-03-2／設計書 4.2A）[x]

背景：別ホスト実機検証（FR-09-2）で 2 件の要望（`/exit でも終了したい`／`応答後に改行してプロンプト再描画してほしい`）を受領。前者は `/help` への発見性向上、後者は T-505 で実装済の挙動を単体テスト＋実機で再確認したもの。

実装結果（2026-05-02）：`/help` に `/quit (alias: /exit)` と承認スキップ手段（`/auto`／`auto_approve_tools`／`--auto-approve-tools`）を併記、ヘッダーを `/quit, /exit, or ^D to terminate.` に更新。単体テスト `input_loop_terminates_on_exit_command` を追加（PASS）。実機 5 経路（`/quit`／`/exit`／`Ctrl+D`／`SIGINT`／`SIGTERM`）すべて 110ms 以内で正常終了、レジストリ残留なしを確認。視覚レイアウトは T-505 の Pending 状態抑止により既に保証され、`input_loop_waits_for_agent_idle_between_user_prompts`／`stale_idle_signal_is_drained_before_pending` が「Pending 中は stdin が読まれない＝二重プロンプト先行描画なし」を間接的に保証している。

ドキュメント追補（2026-05-02、別ホストでのユーザー質問「ツール実行の許可を解除する設定方法はあるか」への発見性向上）：
- `doc/usage.md`：REPL コマンド表に `/auto [on|off|status]` 追加。「ツール実行承認のスキップ」専用節で 3 経路（設定ファイル／CLI フラグ／`/auto`）を表形式で明示。
- `README.md`：REPL 一行紹介に `/auto`／`/exit` を追加し、「ツール承認をスキップする」節で同じ 3 経路を表形式で明示。

### T-506 ツール承認の入出力統合と `/auto` REPL コマンド（FR-04-1／FR-04-2／設計書 4.3A）[x]

実装結果（2026-05-02）：

- `agent.rs`：`pub struct ApprovalRequest { tool_name, args, response: oneshot::Sender<bool> }` を新規追加。`Agent` に `auto_approve: Arc<AtomicBool>`／`approval_tx: Option<mpsc::Sender<ApprovalRequest>>` を持たせ、`process_turn` の承認判定を `request_approval()` 関数経由（oneshot 待ち）に変更。旧 `approval_prompt`（std::io::stdin 直読み）は完全削除。
- `app.rs`：`PromptState` を `Ready / Pending / AwaitingApproval(oneshot::Sender<bool>)` の 3 値に拡張。`run_input_loop` に `approval_rx` 引数を追加、`tokio::select!` の arm を 4 本に。AwaitingApproval 中の stdin 行を `y/yes` 判定して oneshot に送信、Pending へ遷移。承認チャネル閉鎖時は `Option<Receiver>` を `None` 化して busy loop を回避。shutdown 経路で AwaitingApproval を抜ける際は `resp_tx.send(false)` で安全側倒し。
- REPL コマンド `/auto on|off|status` を `handle_auto_command` として実装（`Arc<AtomicBool>` を ReplState 経由で共有）。`/help` 出力に承認スキップ手段の 3 経路を併記。
- 単体テスト 5 件追加：`approval_channel_grants_tool_execution`／`approval_channel_denial_skips_tool`／`auto_approve_atomic_skips_approval_channel`（agent.rs）、`approval_y_resolves_true_and_blocks_user_prompt`／`shutdown_during_awaiting_approval_replies_false`（app.rs）。
- 検証：`cargo test` 53 件 PASS、`cargo fmt --check` PASS、`cargo clippy --all-targets -- -D warnings` 警告ゼロ。実機で `/auto on/off/status` の表示と `/help` への記載を確認、5 経路の終了挙動に回帰なし。
- 残：実プロバイダ × shell tool での y 承認 → ツール実行成立は実 API キー環境（T-704）で確認予定。

### T-505 REPL プロンプト同期（FR-03-2／設計書 4.2A）[x]

別ホストでのワンライナー導入検証（FR-09-2）において「一度、応答があると次の入力が行えないときがある」事象が報告された。AI 応答ストリームと REPL プロンプト描画が競合し、ユーザーが入力可能な状態を判別できないことが原因と推測される。設計書 4.2A「プロンプト同期」に従って実装し、テストで再発防止する。

- [x] `enum PromptState { Ready, Pending }` を `run_input_loop` に導入
- [x] 会話ループから入力ループへ「AI応答完了」通知（`mpsc::channel::<()>` を `agent_idle_tx`／`_rx` として `display_task` ↔ 入力ループ間で接続）
- [x] `UserPrompt` 送信時に `Pending` へ遷移、`Done` 受領時に `Ready` へ復帰してプロンプト再描画
- [x] `Pending` 中は `tokio::select!` の `if prompt_state == Ready` ガードで標準入力読取を停止
- [x] `PeerPrompt` 経由の AI 応答完了でも `display_task` が同じ idle 通知を発火するため、状態遷移は共通化
- [x] `Pending` 遷移直前に `agent_idle_rx.try_recv()` で過去の通知を drain（peer prompt 等で蓄積した stale 通知を誤って消費しない）
- [x] `display_task` で `Done` だけでなく `Error` も idle として扱う防衛策を追加（Provider 構築直後の HTTP 失敗等で `Done` が省かれるケースのフォールバック）
- [x] `agent.rs::process_turn` で `complete_stream` 失敗時にも `Done` を必ず emit するよう修正
- [x] 単体テスト：`input_loop_waits_for_agent_idle_between_user_prompts` — 2 行入力を投入し、`Pending` が 2 件目を抑止することを assert
- [x] 単体テスト：`stale_idle_signal_is_drained_before_pending` — 古い idle 通知が drain されることを assert
- [x] 既存 3 件の REPL／終了テスト（EOF／/quit／external shutdown）は新シグネチャでも全 PASS
- [x] 実機検証：プロバイダ未到達設定で 2 件の入力を `(printf "first\n"; sleep 1; printf "second\n"; sleep 1; printf "/quit\n")` で投入 → それぞれ独立にエラー応答 → プロンプト再描画 → 次入力受領 →`/quit` 終了が 2.1 秒で完了
- [x] `cargo test` 47 件 PASS、`cargo fmt --check` PASS、`cargo clippy --all-targets -- -D warnings` 警告ゼロ
- [x] 4 経路の終了挙動（`/quit`／`Ctrl+D`／`SIGINT`／`SIGTERM`）に回帰なし（最大 112 ms で終了、レジストリ残留なし）
- [ ] 手動：別ホストで実プロバイダ接続のもと、3 往復以上の対話が滞りなく続けられることを確認（T-704 と連動）

### T-504 終了処理（FR-13／設計書 4.9）[x]

別ホストでのワンライナー導入検証（FR-09-2）において、`/quit` および `Ctrl+D` の双方で `agent-cli` プロセスが終了しない不具合が報告された。設計書 4.9「終了処理（shutdown coordination）」に従って実装し、テストで再発防止する。

- [x] 共通の `tokio::sync::watch::Sender<bool>` shutdown チャネルを起動時に生成し、入力ループ・IPC 転送タスク・signal タスクへ `Receiver` をクローン配布
- [x] `/quit` REPL コマンドのハンドラから shutdown チャネルへ通知し、入力ループを break する（`run_input_loop` 末尾で `shutdown_tx.send(true)`）
- [x] 標準入力 EOF（`Ctrl+D`）：`BufReader::lines().next_line().await` が `Ok(None)` を返した時点で shutdown チャネルへ通知する
- [x] `tokio::signal::ctrl_c()` および `SignalKind::terminate()` を別タスクで待ち受け、受信時に shutdown チャネルへ通知する（`wait_for_termination_signal`）
- [x] IPC 転送タスクは `tokio::select!` で shutdown 監視を併走させ、通知受領時に break する
- [x] `IpcServer` に `Drop` を追加し、accept タスクの abort と Unix socket 削除を保証
- [x] `RegistryHandle` に `Drop` を追加し、レジストリメタ／ソケット削除を panic 時にも保証
- [x] `agent_handle` に `abort_handle()` 経由のタイマー（500ms）を仕込み、in-flight プロバイダ呼び出しが残ってもタイムアウトで abort する
- [x] `main()` で `run().await` の完了後に `std::process::exit(0/1)` を呼び、tokio runtime drop を待たずに即終了（`tokio::io::stdin()` のブロッキングスレッドが残ると EOF まで待たされる問題を回避）
- 検証：
  - [x] 単体テスト 3 件 PASS（`input_loop_terminates_on_eof`／`input_loop_terminates_on_quit_command`／`input_loop_responds_to_external_shutdown`）
  - [x] 単体テスト 1 件 PASS（`ipc::server::tests::drop_removes_socket_file`：IpcServer Drop でソケット削除）
  - [x] 実機 4 経路すべて 1 秒以内に正常終了し、`<registry_dir>` に残存物なし（commit hash: 修正後ビルド、ollama 不到達設定でテスト）
    - `/quit`：112 ms
    - `Ctrl+D`（EOF）：124 ms
    - `SIGINT`：3 ms
    - `SIGTERM`：4 ms
  - [x] `cargo test` 45 件 PASS、`cargo fmt --check` PASS、`cargo clippy --all-targets -- -D warnings` 警告ゼロ

---

## フェーズ6：結合テスト・受け入れ

### T-600 結合テスト [x]

- [x] モックProvider（`ai::testing::MockProvider`）でAgent会話ループのE2Eテスト
  - `agent_emits_text_and_done`：text delta 連結 + Done
  - `agent_completes_tool_use_cycle_with_shell`：tool_use → shellツール実行 → 結果反映 → 続報テキスト
  - `agent_set_system_prompt_replaces_first_message`：`SetSystemPrompt`で履歴先頭を差し替えて Info 通知
- [x] IPC往復の単体テスト（`server_receives_prompt`）
- 検証：`cargo test`が34件全て通過。**達成。**

### T-601 受け入れシナリオ（手動）

完成判定の必須対象は`claude`と`ollama`（モデル：`glm-5.1:cloud`）の2バックエンド。`codex`／`llama.cpp`は任意検証。

#### T-601-A claude単独（必須）

- [ ] `agent-cli run --provider claude`で起動して対話できる
- [ ] `agent-cli doctor`が終了コード0
- [ ] `agent-cli selftest --provider claude`が終了コード0
- [ ] シェルツール経由で`ls`／`echo`等が実行できる

#### T-601-B ollama単独（必須）[x]

- [x] `agent-cli run --provider ollama --model glm-5.1:cloud`で起動して対話できる
- [x] `agent-cli doctor`が終了コード0（実機検証済 2026-05-01）
- [x] `agent-cli selftest --provider ollama`が終了コード0（4 ステージすべて PASS）
- [x] シェルツール経由のコマンド実行（selftest Stage 2 で `echo selftest` 検証 PASS）

検証ログ：
- `.aiprj/AI_LOG/2026-05-01_016.md`：`scripts/manual_acceptance.sh` 経由の初回検証
- `.aiprj/AI_LOG/2026-05-01_018.md`：`doctor` のクラウド対応タイムアウト調整（15s→60s）後に doctor＋selftest の完全 PASS を再確認

#### T-601-C claude × ollama 2プロセス協調（必須）[~]

IPC レイヤー（プロセス起動／登録／Prompt/Ack 配送）は selftest Stage 4 で、AI 応答生成は selftest Stage 5（ollama 環境で自動）で検証可能。
claude を含む 2 バックエンド異種ペアの実機検証は API キー環境で人間が確認。

- [ ] ターミナルAで`--provider claude --name alice`、ターミナルBで`--provider ollama --model glm-5.1:cloud --name bob`を起動（API キー設定環境）
- [x] 両プロセスで同一`registry_dir`を共有（Stage 4／5 で親子プロセスが共有 registry_dir を使う構成で動作確認済み）
- [x] `/list`に2プロセスが表示される（Stage 4 にて子プロセスのレジストリ登録を自動検証）
- [x] Aから`/send`相当の Prompt が IPC で配送され `Ack` が返る（Stage 4 の Prompt/Ack 検証で自動カバー）
- [x] Bがピアプロンプトを受信して AI 応答を返す経路（Stage 5 で ollama×ollama 代替検証 PASS、`peer responded: "HELLO"`）
- [ ] Aから`/send bob "hello"`でBへ送信、Bがollamaで応答（claude+ollama 異種ペアの実機検証、要実環境）
- [ ] Bから`/send alice "..."`でAへ送信、Aがclaudeで応答（claude+ollama 異種ペアの実機検証、要実環境）

#### T-601-D 任意検証（記録のみ）[~]

- [x] `scripts/manual_acceptance.sh` に scenario D1（codex／OpenAI）と D2（llama.cpp）を追加。`OPENAI_API_KEY` 未設定／llama.cpp 未起動時は自動 SKIP
- [ ] `--provider codex`での対話／selftest（API キー設定環境で実施）
- [ ] `--provider llama.cpp`での対話／selftest（llama.cpp サーバー稼働環境で実施）

#### T-601-E 検証結果記録

- [ ] 各シナリオのバックエンド・モデル・コミットハッシュ・日時・合否を作業ログに記録

### T-650 インストールスクリプト（`install.sh`）（FR-11）[~]

- [x] POSIX `sh`互換でリポジトリ直下に`install.sh`を作成
- [x] Linux（x86_64／aarch64）以外は`uname`で検出してエラー終了
- [x] 環境変数 `AGENT_CLI_REPO`／`AGENT_CLI_REF`／`AGENT_CLI_PREFIX`／`AGENT_CLI_INSTALL_FORCE` をサポート
- [x] カレントがリポジトリ内ならローカルソースを使用、それ以外は`git clone`してビルド
- [x] `cargo install --path . --root $AGENT_CLI_PREFIX` でインストール（既定`$HOME/.local`）
- [x] `cargo`／`git` 不在時のヒント表示
- [x] `PATH`点検と警告
- [x] `README.md` のインストール節にワンライナーとオプションを追記
- 検証：
  - [x] `sh -n install.sh` syntax OK
  - [x] `AGENT_CLI_PREFIX=$TMP/prefix sh install.sh` で `agent-cli 0.1.0` バイナリ生成・実行確認
  - [~] 別ホストで `curl ... | sh`（実機検証進行中）
    - インストール自体は成功
    - `/quit` で終了しない不具合を発見 → T-504 で修正、別ホストで「解決した」とユーザー確認済み（2026-05-02）
    - `Ctrl+D`（EOF）で終了しない不具合を発見 → T-504 で修正、別ホストで「解決した」とユーザー確認済み（2026-05-02）
    - 「一度、応答があると次の入力が行えないときがある」事象を発見 → T-505 で実装済（2026-05-02）、別ホストでは「たぶん、解決した」とユーザー暫定確認
    - 「ツール実行の許可を解除する設定方法はあるか」という質問を受領 → T-506 で `/auto` コマンド追加と `/help` 改善、さらに `doc/usage.md`／`README.md` に「ツール承認をスキップする」3 経路を専用節で追補（2026-05-02）。新バイナリで `/help` 実行、または README／usage を見れば直ちに 3 経路が分かる状態。別ホスト側の確認待ち。
    - 「ツール実行してから応答が無い」事象を発見（承認 `y` が次のプロンプトとして取り違えられる）→ T-506 で承認チャネル経由に置き換え、std::io::stdin 直読みを排除。**別ホストで解決確認済（2026-05-02、ユーザーが `[tool approval] ... approve? [y/N]: y` → `[tool-result ok] shell: ...` → 正常な AI 応答の完全ログを添付）**
    - 「`/exit` でも終了したい」要望を受領 → T-507 で `/help` への併記とテスト追加（2026-05-02）。**別ホストで「解決した」とユーザー確認済**
    - 「応答後に改行してプロンプト再描画してほしい」要望を受領 → T-507 で T-505 の Pending 抑止により既に保証されていることを再確認、テストで明示（2026-05-02）。**別ホストで「解決した」とユーザー確認済**
    - 「一度、応答があると次の入力が行えないときがある」事象 → T-505 で実装済。別ホストで「たぶん、解決した」と暫定確認
    - 開発機上での 5 経路（`/quit`／`/exit`／`Ctrl+D`／`SIGINT`／`SIGTERM`）回帰テストはすべて 110ms 以内で正常終了し、レジストリ残留物なしを確認
    - 残作業：別ホストでの `/auto` 動作確認、`agent-cli --help`／`agent-cli doctor`／実プロバイダ対話の確認（T-704）

### T-602 ドキュメント整備（FR-12）

ドキュメントの作成・更新は、機能を追加するPRと同じPR内で行うことを原則とする。

#### T-602-1 README.md [x]

- [x] プロジェクト概要・特徴・対応バックエンド早見表
- [x] インストール（ワンライナー＋ソースビルド）
- [x] クイックスタート（5分手順）
- [x] **設定方法セクション**（最低限編集項目、コピペサンプル、`--config`／`AGENT_CLI_CONFIG`の使い分け、複数プロファイル例）
- [x] 主要コマンド早見表
- [x] 検証手順（`cargo test`／`agent-cli doctor`／`agent-cli selftest`）
- [x] `doc/`配下のリンクをドキュメント目次として追加
- [x] 英語版`README.en.md`を併設（日本語READMEから相互リンク）

#### T-602-2 doc/usage.md [x]

- [x] 各サブコマンド（`run`／`list`／`send`／`providers`／`doctor`／`selftest`／`config`）の詳細
- [x] REPLコマンド（`/list`／`/send`／`/tools`／`/persona`／`/reload-persona`／`/peer`／`/cancel`／`/help`／`/quit`）の解説
- [x] ユースケース：単独対話、ローカルLLM、2プロセス協調、ペルソナ運用、ワンショット送信、プロファイル切替

#### T-602-3 doc/config.md（最重要）[x]

- [x] 設定ファイル解決順序の図解（`--config` → `AGENT_CLI_CONFIG` → 既定パス）
- [x] 全体構造図と各セクションの役割
- [x] 全項目リファレンス（キー／型／既定／必須・任意／説明）
- [x] 完全サンプル3種（最小／推奨／全機能有効）
- [x] APIキー管理（環境変数、`api_key_env`、`.envrc`、`systemd EnvironmentFile`の例）
- [x] 複数プロファイル運用（`registry_dir`分離・共有）
- [x] シェルツールチューニング（`timeout_secs`／`max_output_kb`）
- [x] UI表示モード（`ui.show_thinking`）
- [x] よくある設定ミスと`agent-cli doctor`の読み方
- [x] 設定変更の反映と再起動の要否
- 検証：設計書11.1の章立てを満たし、各項目に具体例がある。**達成。**

#### T-602-4 doc/providers/{claude,codex,ollama,llamacpp}.md [x]

各バックエンド：

- [x] 前提条件（アカウント、APIキー発行、ローカルサーバー導入）
- [x] 認証情報設定方法
- [x] 推奨モデルと用途別の選び方
- [x] `base_url`の指定（プロキシ／互換サーバー）
- [x] 対応機能マトリクス（thinking／tool_use／streaming）
- [x] `agent-cli doctor`／`selftest --provider`での確認手順
- [x] 既知の制限・トラブルシューティング
- 検証：4バックエンドすべてで同等水準の情報が揃っている。**達成。**

#### T-602-5 doc/tools.md [x]

- [x] `shell`／`fs_read`／`fs_write`／`send_to`の引数スキーマ・戻り値・制限・承認フロー
- [x] 拒否時の挙動とAIへの返却形式

#### T-602-6 doc/architecture.md [x]

- [x] システム構成図（プロセス・IPC・レジストリ）
- [x] Provider抽象とtool橋渡しの概要
- [x] データフロー（ユーザー入力／IPC受信から応答描画まで）

#### T-602-7 doc/troubleshooting.md [x]

- [x] APIキー未設定／間違い／レート超過
- [x] Ollama未起動／llama.cppサーバー未起動
- [x] ソケット権限・stale掃除
- [x] レジストリ衝突（同一`registry_dir`での競合）
- [x] シェルツールのタイムアウト・出力サイズ超過

#### T-602-8 CONTRIBUTING.md／CHANGELOG.md／LICENSE [x]

- [x] `CONTRIBUTING.md`：開発環境構築、`cargo fmt`／`clippy`／`test`、新バックエンド／新ツール追加手順、PR作法、ドキュメント同時更新の必須化
- [x] `CHANGELOG.md`：Keep a Changelog形式、SemVer、初回エントリを記載
- [x] `LICENSE`：MIT 全文

#### T-602-9 rustdocコメント [x]

- [x] 公開API（`Provider`／`Capabilities`／`Tool`／`AgentId`／`IpcMessage`）に`///`コメント
- [x] `cargo doc --no-deps`が警告なしで完走
- 検証：生成ドキュメントで主要型の説明が読める。**達成。**

#### T-602-10 ドキュメント整合性チェック [x]

- [x] サンプル`config.toml`（`doc/config.md` 4.1〜4.3 の最小／推奨／全機能有効）がTOMLパーサで通ることを単体テストで保証（`doc_config_md_full_samples_parse`）
- [x] `tools.enabled` 内の名前が実装ツールと一致することを単体テストで保証（`enabled_tool_names_match_implementation`）
- [x] 同梱サンプルペルソナ（`example/agents/*.md`）がすべて `Persona::load` で読み込めることを単体テストで保証（`bundled_example_personas_parse`）
- [x] CLI 主要サブコマンドの定義整合性を単体テストで保証（`cli_help_compiles_and_lists_known_subcommands` ほか 3 件）
- [x] CI（`.github/workflows/ci.yml`）で fmt／clippy／build／test／doc／selftest（Stage 2/3）を自動化

---

## フェーズ7：完成後検証（FR-09）

完成判定はこのフェーズの全タスクが成功することをもって行う。

### T-700 doctorサブコマンド実装 [x]

- [x] 設定ファイルの存在・パース確認、解決済みパス表示
- [x] 選択中バックエンドのAPIキー（環境変数）存在確認
- [x] バックエンド疎通確認（`complete_stream` 起動でストリーム開始までを検証、60秒タイムアウト）
- [x] レジストリディレクトリ・ログディレクトリの書き込み可否
- [x] `bash`存在確認
- [x] OK／FAIL表示と終了コード制御（FAIL時exit≠0）
- [x] クラウドルーティングモデル（例：Ollama `*:cloud`）のコールドスタートに耐えるよう、疎通タイムアウトを 15 秒 → 60 秒に拡張（実機 `glm-5.1:cloud` で PASS 確認）

### T-701 selftestサブコマンド実装 [x]

- [x] **Stage 1**：短いプロンプト（"Reply with the literal text OK."）を送り `OK` 検出、タイムアウト管理（60秒）
- [x] **Stage 2**：`ToolRegistry` から `shell` ツールを取得し、`echo selftest` を実行して標準出力に `selftest` が含まれることを検証
- [x] **Stage 3**：一時ディレクトリにIPCサーバーを起動し、`Ping` → `Pong` のラウンドトリップが成功することを検証
- [x] **Stage 4**：自バイナリを子プロセスとして起動し、レジストリ登録待機 → `Ping`/`Pong` ＋ `Prompt`/`Ack` → 終了処理を検証
- [x] **Stage 5**：実 provider 設定で子プロセスを起動し、`Prompt` 送信 → 子の会話ログから `assistant` 応答出現を確認（最大 90 秒）。Stage 1 失敗時は SKIP
- [x] 各ステージの成否を逐次表示し、いずれかが失敗すると終了コード非0で全体FAIL
- 検証：Stage 2／3／4 はバックエンド外部依存ゼロで通過。Stage 1／5 は実プロバイダ環境で実機検証済（ollama / glm-5.1:cloud で `peer responded: "HELLO"`）。

### T-702 自動テスト網羅性確認 [x]

- [x] `cargo test`が**41テスト**パス
- [x] `cargo clippy --all-targets -- -D warnings`通過
- [x] `cargo build` 成功（警告ゼロ）
- [x] `cargo fmt --all -- --check` 通過
- [x] CI（`.github/workflows/ci.yml`）で fmt／clippy／build／test／doc／selftest を自動化

### T-703 完成検証レポート [~]

- [x] `agent-cli doctor`／`agent-cli selftest`を `ollama (glm-5.1:cloud)` で実行し結果を作業ログに記録（ログ `_016`）
- [ ] `claude` での doctor／selftest 結果記録（API キーが設定可能な環境で実施）
- [x] T-601-B 必須シナリオの合否を作業ログに記録
- [ ] T-601-A／C 必須シナリオの合否を作業ログに記録（API キー設定環境で実施）
- [ ] T-601-D（任意シナリオ）を実施した場合はその結果も併記
- [x] 半自動実行スクリプト `scripts/manual_acceptance.sh` を整備（SKIP / PASS / FAIL を集計）

### T-704 別ホストワンライナー導入検証（FR-09-2）[~]

T-650 の三つ目のチェック項目に対応する独立タスクとして切り出す。T-504／T-505／T-506／T-507 の修正完了後に必須シナリオを再実行する。

- [x] 別ホストでワンライナーインストールが成功することを確認
- [ ] 別ホストで `agent-cli --help` が正常表示されることを確認（T-506／T-507 で `/auto`／`/exit`／`auto_approve_tools` 説明が含まれる前提）
- [ ] 別ホストで `agent-cli doctor` が終了コード 0 で完了することを確認（バックエンドに応じた API キーを設定）
- [ ] 別ホストで引数なしの `agent-cli` 実行で REPL に入れることを確認（FR-01）
- [~] 別ホストで REPL のユーザー入力 → AI 応答 → 次のユーザー入力を 2 往復以上滞りなく繰り返せることを確認（T-505 修正後／FR-03-2、ユーザー暫定「たぶん、解決した」確認）
- [x] 別ホストでユーザー入力直後に `> ` が前置されず、応答終了後に改行＋`> ` が描画されることを目視確認（T-507／FR-03-2、ユーザー「解決した」確認、2026-05-02）
- [x] 別ホストで `/exit` で終了することを確認（T-507／FR-13、ユーザー「解決した」確認、2026-05-02）
- [x] 別ホストでシェルツール承認 `y` 入力が確実に承認として処理されることを確認（T-506 修正後／FR-04-1、ユーザーが完全な動作ログを添付、2026-05-02：「`approve? [y/N]: y` → `[tool-result ok] shell: ...` → 正常な AI 応答」）
- [ ] 別ホストで `/auto on` によるツール承認スキップが動作することを確認（T-506 修正後／FR-04-2）
- [x] 別ホストで `agent-cli run` を起動し `/quit` で終了することを確認（T-504 修正後、ユーザーが `.aiprj/instructions.md` で「解決した」と確認、2026-05-02）
- [x] 別ホストで `agent-cli run` を起動し `Ctrl+D`（EOF）で終了することを確認（T-504 修正後、ユーザーが `.aiprj/instructions.md` で「解決した」と確認、2026-05-02）
- [ ] 終了後に `<registry_dir>/<agent-id>.sock` および `<registry_dir>/<agent-id>.json` が残っていないことを確認
- [ ] 検証結果（コミットハッシュ・対象ホスト・実行日時・各項目の合否）を `.aiprj/AI_LOG/` に記録

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
