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
| Thinking | ✗ | 非対応 |

`Capabilities` は静的に `tool_use=true` を返しますが、サーバー／モデルが対応していない場合はツール呼び出しが行われない（または無視される）ことがあります。

## 動作確認

```bash
ollama serve &
agent-cli --provider ollama doctor
agent-cli --provider ollama selftest --provider ollama
```

`doctor` の `provider conn` ステップで `OK (stream initiated)` になれば疎通は良好です。

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
