# Codex（OpenAI）バックエンド

OpenAI Chat Completions API（streaming、function calling）を利用するバックエンドです。「codex」という名前は agent-cli 内部の `kind` であり、OpenAI のレガシー Codex モデルを意味するものではありません。

## 前提条件

- OpenAI のアカウントで API キーを発行
- 環境変数として保持（既定 `OPENAI_API_KEY`）

## 設定

```toml
[provider]
kind = "codex"

[provider.codex]
api_key_env = "OPENAI_API_KEY"
model       = "gpt-4.1"
base_url    = "https://api.openai.com/v1"
```

## 推奨モデル

OpenAI 公式のモデル名一覧から、以下を目安に選んでください。

| 用途 | 例 |
|------|----|
| 推論重視・コード | `gpt-4.1` 系 |
| 汎用対話 | `gpt-4o` 系 |
| 軽量 | `gpt-4o-mini` 系 |

`base_url` を変えれば、OpenAI 互換のゲートウェイ／企業内プロキシ／Azure OpenAI 互換エンドポイントなどでも動作します。

## 対応機能

| 機能 | 対応 | 備考 |
|------|------|------|
| Streaming | ✓ | SSE |
| Tool use | ✓ | function calling を `ProviderEvent::ToolUse` に正規化 |
| Thinking | ✗ | 非対応（`Capabilities::thinking=false`） |

## 動作確認

```bash
export OPENAI_API_KEY="sk-..."
agent-cli --provider codex doctor
agent-cli --provider codex selftest
```

## 既知の制限

- function calling 非対応モデルでは tool 呼び出しが行われない可能性があります。`gpt-4.1`／`gpt-4o` 系を推奨。
- 完了レスポンスの `[DONE]` 直前で残った tool_call をフラッシュする実装になっています。途中切断の影響を受ける可能性があります。

## トラブルシューティング

| 症状 | 原因 | 対処 |
|------|------|------|
| `env var OPENAI_API_KEY not set` | 未設定 | `export OPENAI_API_KEY=...` |
| `HTTP 401` | キー失効／組織制限 | 別の API キーを試す |
| 不完全な応答 | モデルが function calling を使えない | モデルを変更 |
