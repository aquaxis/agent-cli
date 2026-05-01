# Claude バックエンド

Anthropic Claude API（Messages、SSE）を利用するバックエンドです。`agent-cli` のリファレンス実装で、thinking／tool_use／streaming すべてに対応します。

## 前提条件

- Anthropic コンソールで API キーを発行
- API キーを環境変数として保持（既定 `ANTHROPIC_API_KEY`）

## 設定

```toml
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
model       = "claude-opus-4-7"
base_url    = "https://api.anthropic.com"   # 通常はこのまま
thinking    = true                           # thinking ブロックを有効化
```

## 推奨モデル

| 用途 | モデル |
|------|--------|
| 推論重視・コード生成全般 | `claude-opus-4-7` |
| バランス型 | `claude-sonnet-4-6` |
| 軽量・高速 | `claude-haiku-4-5-20251001` |

実際に利用可能なモデルは Anthropic コンソールで確認してください。

## 対応機能

| 機能 | 対応 | 備考 |
|------|------|------|
| Streaming | ✓ | SSE |
| Tool use | ✓ | Anthropic ネイティブ tool_use ブロックをパース |
| Thinking | ✓ | `[thinking]` ヘッダーで表示。`ui.show_thinking` で制御 |

## 動作確認

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
agent-cli --provider claude doctor
agent-cli --provider claude selftest
```

両方とも終了コード 0 になればバックエンドは健全です。

## プロキシ／互換サーバー

社内プロキシや Anthropic 互換ゲートウェイを使うには `base_url` を上書きしてください。

```toml
[provider.claude]
base_url = "https://proxy.example.com/anthropic"
```

## 既知の制限

- 大量の tool_use を伴う長時間応答時、`reqwest` のタイムアウト（120 秒）を超えると失敗します。長時間ジョブはツール側でタイムアウトを管理してください。
- `thinking_delta` の表示は逐次行で出力されるため、ターミナル幅次第で視認性が落ちます。`ui.show_thinking = "collapsed"` を推奨。

## トラブルシューティング

| 症状 | 原因 | 対処 |
|------|------|------|
| `env var ANTHROPIC_API_KEY not set` | 環境変数未設定 | `export ANTHROPIC_API_KEY=...` |
| `HTTP 401` | キー失効／誤り | 公式コンソールで再発行 |
| `HTTP 429` | レート制限 | 利用ペース調整 |
| 応答が空 | thinking 専用設定でモデルが thinking しか出さない | `thinking=false` で再試行 |
