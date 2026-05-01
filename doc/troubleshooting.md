# トラブルシューティング（`troubleshooting.md`）

困ったときに最初に試すコマンドは `agent-cli doctor` と `agent-cli selftest` です。

## API キー関連

### 起動直後に `env var ANTHROPIC_API_KEY not set` などで終了する

- 設定の `api_key_env` で指定された環境変数が未設定です。
- `export ANTHROPIC_API_KEY=...` してから再実行してください。
- `direnv` 利用時は `.envrc` の `direnv allow` 状態を確認。

### `HTTP 401: ...` が応答する

- API キーが失効／無効、または別アカウントのキーを使っている可能性。
- `agent-cli doctor` の `provider conn` ステップで再現します。
- 公式コンソールでキーの有効性を確認してください。

### `HTTP 429: ...` が応答する

- レート制限。短時間で大量のリクエストを発行していないか確認してください。
- `[tools.shell] timeout_secs` を長めにとっておくと、長時間処理中の再試行で過剰なリクエストを抑えられます。

## Ollama / llama.cpp 関連

### `provider conn : FAIL (...)` が `doctor` に出る

- ローカルサーバーが停止している／別ポートで起動している可能性。
- 確認：
  ```bash
  curl -s http://127.0.0.1:11434/api/tags    # ollama
  curl -s http://127.0.0.1:8080/v1/models    # llama.cpp
  ```
- 設定の `base_url` を実環境に合わせてください。

### `glm-5.1:cloud` が見つからない

- Ollama の cloud モデルが手元の環境で利用できない場合があります。
- `ollama list` でローカルにあるモデル名を確認し、`--model <existing>` で起動できます。

## レジストリ／IPC 関連

### `/list` に他プロセスが現れない

- 双方の `[runtime] registry_dir` が異なる可能性が高いです。
- `agent-cli config show` で確認し、`registry_dir` を共有して再起動してください。

### 古いソケット／JSON が残る

- 通常はプロセス終了時に自動削除されますが、`SIGKILL` などで強制終了した場合に残ることがあります。
- `agent-cli list` を実行すると stale エントリを自動掃除します。
- 手動掃除する場合：
  ```bash
  rm /tmp/agent-cli/*.sock /tmp/agent-cli/*.json
  ```

### `bind ... failed: Permission denied`

- `registry_dir` の権限不足。`mkdir -p` で作成した上で `chmod 0700` を確認してください。
- root で動かしたソケットが残っている場合、所有権の問題が起きます。掃除して再作成してください。

## シェルツール関連

### `timed out after 60 seconds: ...`

- 既定タイムアウトを超過。`[tools.shell] timeout_secs` を増やすか、AI に短いコマンドを指示してください。

### 出力末尾に `...[truncated]`

- `max_output_kb`（既定 256KB）を超えた出力は切り詰めています。閾値を上げてください。

### `tool_result ERR: spawn error: ...`

- `bash` が見つからない／実行不可。`agent-cli doctor` の bash チェックでも検出できます。
- Linux 以外では本アプリは未対応です。

## ペルソナ関連

### 起動時に `persona file not found: ...`

- `--persona` または `[runtime] persona_file` で指定したパスが存在しません。
- 既定パス（`<agents_dir>/<name>.md`）の場合は黙って組み込み既定にフォールバックします。

### `role` が必須エラーになる

- ペルソナファイル先頭の YAML フロントマターに `role: ...` が無い／空です。
- サンプル `example/agents/coder.md` を参考にしてください。

### `/reload-persona` が反映されない

- ペルソナファイルを編集していますか？ パスは `/persona` の `source` で確認できます。
- 反映直後の応答からシステムプロンプトが切り替わります。

## 設定ファイル関連

### `error: config file not found: ...`

- `--config` または `AGENT_CLI_CONFIG` で指定した明示パスが存在しないと自動生成しません。
- 既定パス（`~/.config/agent-cli/config.toml`）を使うと自動生成されます。

### `provider error (claude): [provider.claude] missing`

- 設定ファイルに `[provider.claude]` セクションが無い／壊れている可能性。
- `agent-cli config show` で TOML 構造を確認してください。

## ビルド／インストール関連

### `cargo install` が `--locked` で失敗する

- リポジトリに `Cargo.lock` が無い場合に発生します。`install.sh` は自動的に `--locked` 抜きで再試行するので大半は問題ありません。
- 手動で行う場合：
  ```bash
  cargo install --path . --root "$HOME/.local"
  ```

### `install.sh` が `cargo is required but not found.`

- Rust ツールチェーンが入っていません。
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source "$HOME/.cargo/env"
  ```

## それでも解決しない場合

1. `agent-cli doctor` の出力をすべてコピー
2. `cargo --version`／`rustc --version` の情報
3. 設定ファイル（API キーは伏せる）
4. 直前のコマンドと表示メッセージ

を添えて、リポジトリの Issue に報告してください。
