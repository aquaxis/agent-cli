# ツールリファレンス（`tools.md`）

`agent-cli` 内蔵ツールの引数スキーマ、戻り値、制限、承認フローを示します。

## 共通仕様

- ツールは AI から JSON 形式の入力で呼び出されます。
- 戻り値は `{"ok": bool, "content": string}` の `ToolOutput` で表現され、`content` が AI に渡されます。`shell` のように構造化結果を返すツールは `content` に JSON 文字列を入れます。
- 承認フロー：`auto_approve_tools=false`（既定）では実行前に REPL の入力ループ経由で y/N を取得します。詳細は下記「ツール実行承認」を参照。拒否時は `user denied tool execution` が AI に返ります。
- ペルソナの `allowed_tools`／`denied_tools` で利用可能ツールを制御できます（`doc/config.md` 参照）。

## ツール実行承認

承認 y/N の入出力は **REPL のメイン入力ループと統合** されています（`std::io::stdin().read_line()` の直読みは行いません）。これにより承認入力がユーザーの通常プロンプトと取り違えられる事象を防ぎます。

仕組み：

1. agent タスクが `ApprovalRequest { tool_name, args, response: oneshot::Sender<bool> }` を入力ループへ送信。
2. 入力ループは状態を `AwaitingApproval` に遷移し、`[tool approval] <tool> <args>` バナーと `approve? [y/N]: ` を描画。
3. 入力ループが次の stdin 行を読み、`y`／`yes` のみ承認、それ以外（空入力／別単語）は拒否として `oneshot` に送信。
4. agent タスクは応答に従ってツールを実行、または `user denied tool execution` を返却。

承認スキップ（自動許可）の経路：

| 経路 | 例 | 反映 |
|------|-----|------|
| 設定ファイル | `[runtime] auto_approve_tools = true` | 起動時 |
| CLI フラグ | `agent-cli run --auto-approve-tools` | 起動時のみ上書き |
| REPL コマンド | `/auto on` | 即時。`/auto off` で承認モードへ復帰、`/auto status` で現在値表示 |

実装上、`auto_approve` は `Arc<AtomicBool>` として agent と REPL 間で共有されており、`/auto on`／`/auto off` でセッション中いつでも切り替えできます。

## `shell`

シェルコマンドを実行します。

### 引数

| キー | 型 | 必須 | 既定 | 説明 |
|------|----|------|------|------|
| `cmd` | string | ✓ | — | 実行するコマンド本文（`bash -lc <cmd>` で実行） |
| `cwd` | string | — | プロセスの cwd | 作業ディレクトリ |
| `timeout_secs` | integer | — | `[tools.shell] timeout_secs`（既定 60） | 個別タイムアウト |

### 戻り値（`content` は JSON 文字列）

```json
{
  "exit_code": 0,
  "stdout": "...",
  "stderr": "..."
}
```

`stdout`／`stderr` が `[tools.shell] max_output_kb` を超える場合は `...[truncated]` が付きます。

### 制限

- `bash -lc` 経由のため、`bash` がインストールされている必要があります。
- タイムアウト超過時は `ok=false` で `timed out after <N> seconds: <cmd>` を返します。
- `auto_approve_tools=false`（既定）の場合、対話 y/N が必要です（上記「ツール実行承認」参照）。`/auto on` でセッション中は無効化できます。

### 例

```json
{"name":"shell","arguments":{"cmd":"ls /tmp"}}
```

## `fs_read`

UTF-8 のテキストファイルを読み取ります。

### 引数

| キー | 型 | 必須 | 説明 |
|------|----|------|------|
| `path` | string | ✓ | 読み取り対象。`~`／環境変数を展開 |
| `offset` | integer | — | 読み取り開始バイト位置 |
| `limit` | integer | — | 読み取りバイト数 |

### 戻り値

`content` に UTF-8 テキストを返します。バイナリや非 UTF-8 ファイルの場合 `ok=false` で `binary or non-UTF-8 file: <path>` を返します。

### 例

```json
{"name":"fs_read","arguments":{"path":"./Cargo.toml","limit":1024}}
```

## `fs_write`

UTF-8 のテキストをファイルに書き込みます。

### 引数

| キー | 型 | 必須 | 説明 |
|------|----|------|------|
| `path` | string | ✓ | 書き込み先 |
| `content` | string | ✓ | 書き込む内容 |
| `overwrite` | bool | — | `false`（既定）の場合、既存ファイルがあると `ok=false` |

### 戻り値

`ok=true` の場合、`content` に `wrote <path>` を返します。

### 注意

- 親ディレクトリは自動作成します（`mkdir -p`）。
- 既定で上書き拒否のため、AI が誤って既存ファイルを潰す事故を防げます。
- バイナリ書き込みには対応していません。

## `send_to`

別プロセスのエージェント（ピア）へプロンプトを送信します。

### 引数

| キー | 型 | 必須 | 説明 |
|------|----|------|------|
| `peer` | string | ✓ | 宛先 agent-id または表示名 |
| `text` | string | ✓ | 送信するプロンプト |

### 戻り値

成功時 `content` に `delivered to <agent-id>`。失敗時はエラーメッセージ（`peer not found by id or name: ...` など）。

### 例

```json
{"name":"send_to","arguments":{"peer":"alice","text":"レビューお願いします"}}
```

### 注意

- 宛先解決は `registry_dir` 配下の `<agent-id>.json` を走査して行います。
- 同期型（応答待ち）ではなく、Ack を受領した時点で成功扱いとなります。
- 受信側のエージェントには `[peer prompt from <agent-id>]` プレフィックスが付与されたうえで、ユーザー入力相当として AI に渡ります。

## ツール無効化と権限制御

設定／ペルソナでの優先順位：

```text
[tools] enabled の集合
  ∩ persona.allowed_tools が指定されていればそれ
  ＼ persona.denied_tools が指定されていればそれ
= 当該エージェントで利用可能なツール
```

REPL コマンド `/tools` で現在のセットが確認できます。
