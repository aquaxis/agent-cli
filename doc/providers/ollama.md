# Ollama バックエンド

[Ollama](https://ollama.com) のローカル／クラウドサーバーを利用するバックエンドです。`agent-cli` の **必須検証対象**（`claude` と並ぶ完成判定の必須構成）です。

## 前提条件

- Ollama のインストール（公式 README 参照）
- ローカルなら `ollama serve` でサーバーを起動
- クラウドモデルを使う場合は対応バックエンドを起動

## 設定

```toml
[provider]
kind = "ollama"

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"
```

API キーは不要です（`api_key_env` を指定しても無視されます）。

## 推奨モデル

完成判定の必須対象は **`glm-5.1:cloud`** です。それ以外でも `ollama list` で見えるモデルなら同じ書式で指定可能。

```bash
ollama pull glm-5.1:cloud      # クラウド版
ollama pull llama3.1:8b        # ローカル版例
```

## 対応機能

| 機能 | 対応 | 備考 |
|------|------|------|
| Streaming | ✓ | NDJSON |
| Tool use | ✓ (モデル依存) | `tools` 対応モデルでのみ動作 |
| Thinking | ✓ (モデル依存) | NDJSON `message.thinking` を `[thinking]` として表示。`glm-5.1:cloud` 等で動作 |

`Capabilities` は静的に `tool_use=true` ／ `thinking=true` を返しますが、サーバー／モデルが対応していない場合はツール呼び出し／thinking 出力が行われない（または無視される）ことがあります。

### Thinking 表示の制御

`glm-5.1:cloud` 等の thinking 対応モデルが `message.thinking` フィールドを返すと、agent-cli は `ProviderEvent::Thinking` として REPL に `[thinking]` プレフィックス付きで表示します（emit 順は `Thinking` → `Text` → `ToolUse`、Anthropic 仕様と整合）。表示モードは `[ui] show_thinking` で制御できます：

| 設定 | 挙動 |
|------|------|
| `"collapsed"`（既定） | 各 thinking delta を「先頭 80 文字 + `...`」に切り詰めた 1 行で表示 |
| `"expanded"` | thinking 全文を逐次表示 |
| `"hidden"` | thinking を一切表示しない |

ノイズが多い場合は `[ui] show_thinking = "hidden"` を推奨します。詳細は [`doc/config.md`](../config.md) の「UI 表示モード」節を参照。

## 動作確認

```bash
ollama serve &
# doctor は config の provider.kind を使う
agent-cli --config ./ollama.toml doctor
# selftest は --provider で上書き可能
agent-cli selftest --provider ollama
```

`doctor` の `provider conn` ステップで `OK (stream initiated)` になれば疎通は良好です。クラウドルーティングモデル（`*:cloud` タグ）はコールドスタート遅延に備え、疎通タイムアウトは 60 秒に設定されています。

## プロキシ・別ホスト

別ホストで Ollama を動かしている場合：

```toml
[provider.ollama]
base_url = "http://gpu-server.local:11434"
```

## 既知の制限

- `tool_calls` 周りはモデルにより JSON フォーマットが揺れることがあり、エラー時はツール無しで再試行を推奨。
- 大規模モデルではタイムアウト（180 秒）を超える可能性があります。長文生成は `[tools.shell] timeout_secs` も含めて見直してください。

## トラブルシューティング

| 症状 | 原因 | 対処 |
|------|------|------|
| `connection refused` | サーバー未起動 | `ollama serve` |
| `model 'X' not found` | モデル未取得 | `ollama pull X` |
| ツールが呼ばれない | モデルが tools 非対応 | tools 対応モデルへ変更 |
| 応答が遅い | モデルが大きい／GPU 非利用 | より軽量なモデル、または GPU 環境へ移行 |
