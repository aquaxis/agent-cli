# llama.cpp バックエンド

[llama.cpp](https://github.com/ggerganov/llama.cpp) サーバーが提供する OpenAI 互換 API（`/v1/chat/completions`）を利用するバックエンドです。

## 前提条件

- llama.cpp サーバーをビルド／起動
  ```bash
  ./llama-server --port 8080 -m /path/to/model.gguf --jinja
  ```
- OpenAI 互換のオプション（`--jinja` 等、ツール対応用ビルドフラグ）が有効になっていること

## 設定

設定ファイルでは TOML キー名のドット (`.`) のため `"llama.cpp"` をクオートしてください。

```toml
[provider]
kind = "llama.cpp"

[provider."llama.cpp"]
model    = "default"
base_url = "http://127.0.0.1:8080"
api_key_env = "LLAMACPP_API_KEY"   # 任意。Bearer 認証付きビルド時のみ
```

## 対応機能

| 機能 | 対応 | 備考 |
|------|------|------|
| Streaming | ✓ | SSE（OpenAI 互換） |
| Tool use | △ | サーバービルド／モデル次第。動かない場合は無効化されます |
| Thinking | ✗ | 非対応 |

## 動作確認

```bash
./llama-server --port 8080 -m model.gguf &
agent-cli --provider llama.cpp doctor
agent-cli --provider llama.cpp selftest --provider llama.cpp
```

## 推奨モデル

llama.cpp 上で動作する任意の OpenAI 互換チャットモデル（`llama3`／`qwen2.5`／`gpt-oss` など）。tool calling が必要なら、対応する Jinja テンプレート同梱モデルを選んでください。

## 既知の制限

- 同じ OpenAI 互換 API でも、tool_calls 形式やロール表現が微妙に違うサーバーが存在します。動かない場合は `[tools] enabled` を空にして tool 抜きで試行し、応答だけ確認してください。
- `--jinja` 付きで起動していないとツール呼び出しが正しく動きません。

## トラブルシューティング

| 症状 | 原因 | 対処 |
|------|------|------|
| `connection refused` | サーバー未起動 | `./llama-server ...` |
| 応答が空文字だけ | テンプレート未対応 | `--jinja` 有効でビルド／起動 |
| ツールが呼ばれない | モデルが function calling 非対応 | 別モデル、または tool 抜き運用 |
