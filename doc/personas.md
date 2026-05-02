# ペルソナリファレンス（`personas.md`）

`agent-cli` は起動するエージェントごとに「ペルソナ」を割り当てます。ペルソナは Markdown ファイル（YAML フロントマター＋本文）として記述し、エージェントの **役割（role）／スキル／説明／使用ツール／モデル／温度** を一括で定義できます。本ドキュメントは設定方法・記述方法・実装上の挙動をまとめたリファレンスです。

関連ドキュメント：

- [`doc/config.md`](config.md) — `agents_dir`／`persona_file` などの設定キー
- [`doc/architecture.md`](architecture.md) — ペルソナ機構の全体像（§6）
- [`doc/usage.md`](usage.md) — REPL コマンド `/persona`／`/reload-persona`／`/peer`

---

## 1. 概要

ペルソナは以下に影響します。

| 影響先 | 内容 |
|--------|------|
| システムプロンプト | `role`／`skills`／`description`／本文を所定の見出しで連結し、AI への先頭指示として注入 |
| ツールレジストリ | `allowed_tools` をホワイトリスト、`denied_tools` をブラックリストとして適用 |
| Provider 設定 | `model`／`temperature` が指定されていれば、起動時に当該プロバイダのリクエスト body へ上書き |
| REPL ヘッダー | `name`／`role`／`skills` が起動時バナーに表示 |
| `agent-cli list` 出力 | `role`／`skills` が一覧の列に追加 |
| レジストリメタファイル | `<registry_dir>/<agent-id>.json` の `persona` フィールドに `role`／`skills`／`description`／`source_path` が記録され、ピアから `/peer <id>` で参照可能 |

---

## 2. 設定方法（解決順序）

ペルソナファイルの解決優先順位は次のとおり（上から順に試行、最初にヒットしたものを使用）。

```text
1. CLI オプション      `--persona <path>`
2. 設定ファイル        `[runtime] persona_file = "<path>"`
3. ファイル名規約      <agents_dir>/<name>.md  （--name に対応）
4. 組み込み既定        「汎用アシスタント」
```

### 2.1 CLI オプションで明示指定（最優先）

```bash
agent-cli run --persona ./reviewer.md
```

- パスは絶対／相対いずれも可
- ファイルが存在しない場合は **エラー終了**（既定パス未存在時のフォールバックなし）

### 2.2 設定ファイルで指定

```toml
# ~/.config/agent-cli/config.toml
[runtime]
persona_file = "~/.config/agent-cli/agents/alice.md"
```

- `~`／環境変数／相対パス展開あり（`shellexpand` ベース）
- ファイル未存在は同じくエラー終了

### 2.3 名前規約（`<agents_dir>/<name>.md`）

最も運用しやすい方式。`agent-cli run --name <name>` を実行すると `<agents_dir>/<name>.md` を自動的に探します。

```bash
mkdir -p ~/.config/agent-cli/agents
cp example/agents/reviewer.md ~/.config/agent-cli/agents/alice.md

agent-cli run --name alice
# → ~/.config/agent-cli/agents/alice.md が読み込まれる
```

`agents_dir` の既定は `~/.config/agent-cli/agents`。`[runtime] agents_dir` で変更可能：

```toml
[runtime]
agents_dir = "~/projects/agent-cli/personas"
```

このパターンでは **未存在時は黙って組み込み既定にフォールバック** します（明示指定経路と異なる）。

### 2.4 組み込み既定

`--persona` も `persona_file` も `<agents_dir>/<name>.md` もヒットしなかった場合、以下のペルソナで起動します。

```yaml
name: default
role: 汎用アシスタント
skills: [対話, ツール実行]
description: 組み込みの既定ペルソナ
```

本文は「あなたは agent-cli 上で動作する汎用 AI アシスタントです」で始まる短い指示。設定不要で動かしたい人向けの安全側既定です。

---

## 3. ペルソナファイルの形式

### 3.1 ファイル全体構造

```markdown
---
<YAML フロントマター>
---

<Markdown 本文（自由記述）>
```

要件：

- 先頭は `---` で始まる必要があります（BOM は無視されます）。
- 終端 `---` が次行以降に必要。
- フロントマターは YAML でパース。`role` のみ必須キー。
- 本文は trim されてからシステムプロンプトに連結されます。空でも構いません。

### 3.2 フロントマターのキー一覧

| キー | 型 | 必須 | 既定 | 説明 |
|------|----|------|------|------|
| `name` | string | — | （`--name` 引数 or 表示用 `(unnamed)`） | 表示用エージェント名。CLI の `--name` が優先 |
| `role` | string | **✓** | — | 役割（システムプロンプトの「# 役割」セクションに入る） |
| `skills` | string[] | — | `[]` | スキル一覧。「# スキル」セクションで箇条書き |
| `description` | string | — | — | 1〜2 行の補足説明。`/peer` 出力にも表示 |
| `model` | string | — | — | このペルソナを起動した際にプロバイダのモデル名を上書き |
| `temperature` | number | — | — | 同上、サンプリング温度（0.0〜2.0 程度の浮動小数点） |
| `allowed_tools` | string[] | — | — | 利用可能ツールのホワイトリスト。指定するとこのリストのみ有効 |
| `denied_tools` | string[] | — | — | 拒否ツールのブラックリスト。`tools.enabled` から差し引く |

#### 検証エラー

- `role` が未指定／空文字列のとき：`error: \`role\` is required in persona frontmatter`
- フロントマター冒頭の `---` がない／終端がない：`persona file must begin with YAML frontmatter (\`---\`)` または `missing closing \`---\` for YAML frontmatter`
- YAML パースエラー：`invalid YAML frontmatter: <serde_yaml message>`

### 3.3 本文

本文は Markdown として書きますが、`agent-cli` は内容を解釈せず、そのままシステムプロンプトの末尾に挿入します。書き方の自由度が高いぶん、AI に確実に守らせたい指示は箇条書きで簡潔に書くのがおすすめです。

例：

```markdown
あなたは熟練のレビュアーです。以下のルールを必ず守ってください。

- 所有権・ライフタイム上の問題を最優先で指摘する
- パフォーマンスへの影響を定量的に述べる
- 修正案を提示する際は最小差分を心がける
```

---

## 4. システムプロンプトの合成

`Persona::to_system_prompt()` は次の順で 1 つの文字列に組み立て、AI へのシステムメッセージとして送信されます。

```text
<組み込み既定の前置き：
  あなたは agent-cli 上で動作する汎用 AI アシスタントです。
  ユーザーからの依頼に対し、必要に応じてツールを使い、簡潔かつ正確に応答してください。>

# 役割
<frontmatter.role>

# スキル
- <skills[0]>
- <skills[1]>
...

# 説明
<frontmatter.description>

# 詳細
<本文>
```

- `skills` が空配列のとき「# スキル」セクションは出力されません。
- `description` が空文字／未指定のとき「# 説明」は出力されません。
- 本文が空のとき「# 詳細」は出力されません。

会話履歴の先頭 `Message::System` として保持され、`/reload-persona` で差し替え可能です。

---

## 5. ツール権限制御

`allowed_tools`／`denied_tools` は `ToolRegistry::build` で次の順に適用されます。

```text
[tools] enabled の集合
  ↓ allowed_tools が指定されていれば、そのリストとの積集合を取る（ホワイトリスト）
  ↓ 残りに対して denied_tools が指定されていれば、その要素を除外（ブラックリスト）
= 当該エージェントで実際に有効なツール
```

### 5.1 例

設定ファイル：

```toml
[tools]
enabled = ["shell", "fs_read", "fs_write", "send_to"]
```

| ペルソナ指定 | 有効ツール |
|-------------|----------|
| 何も指定しない | `shell, fs_read, fs_write, send_to` |
| `allowed_tools: [shell, fs_read]` | `shell, fs_read` |
| `denied_tools: [fs_write]` | `shell, fs_read, send_to` |
| `allowed_tools: [shell, fs_write]` ＋ `denied_tools: [fs_write]` | `shell` |
| `denied_tools: [send_to]` | このエージェントは他ピアへ `send_to` で送信できない（ただし受信は可） |

REPL で確認：

```text
> /tools
tools: shell, fs_read
```

### 5.2 セキュリティ運用のヒント

- 「読み取り専用」ロール（コードレビュアー等）には `denied_tools: [fs_write]` を付ける
- 「ピアへ依頼するだけ」のディスパッチャ役には `allowed_tools: [send_to]` のみ
- どのペルソナでも `auto_approve_tools=false`（既定）なら、ツール実行ごとに y/N 承認が REPL 入力ループから求められます（`doc/tools.md` 参照）

---

## 6. モデル／温度の上書き

ペルソナの `model`／`temperature` は、起動時に **アクティブなプロバイダ（`provider.kind`）の設定** にだけ上書きが適用されます。

```yaml
---
name: alice
role: 厳密なコードレビュアー
model: claude-opus-4-7
temperature: 0.1
---
```

- 上記ペルソナで `agent-cli run --provider claude --name alice` を起動すると、`provider.claude.model` が `claude-opus-4-7`、`provider.claude.temperature` が `0.1` に上書きされる。
- `--provider ollama` で起動した場合は `provider.ollama.model`／`provider.ollama.temperature` に同じ値が入る（モデル名がプロバイダ間で互換でない可能性に注意）。
- CLI の `--model` を併用すると、CLI 上書き → ペルソナ上書きの順で適用されます（最終的にはペルソナが勝ちます）。

> 温度は Provider 実装側で適切な範囲にクランプ／無視されます。Anthropic Claude は `0.0..=1.0`、OpenAI／Ollama は `0.0..=2.0` 程度を想定。

---

## 7. 完全サンプル

リポジトリ同梱の `example/agents/` には 3 つのサンプルがあり、`bundled_example_personas_parse` 単体テストで常に最新パーサで読めることを保証しています。

### 7.1 `coder.md`（実装担当）

```markdown
---
name: coder
role: Rust ソフトウェアエンジニア
skills:
  - Rust
  - 非同期プログラミング (tokio)
  - CLI 設計
description: 安全で読みやすいコードを書くことに重点を置くエンジニア
allowed_tools:
  - shell
  - fs_read
  - fs_write
  - send_to
---

あなたは agent-cli のコードを書くエンジニアです。
- まず計画を立て、必要に応じて`shell`と`fs_read`でリポジトリを調査してください。
- ファイルを編集する際は、最小差分・既存スタイル尊重を心がけてください。
- 不明な仕様は推測せず、`send_to`で他エージェントに確認してください。
```

### 7.2 `reviewer.md`（読み取り専用レビュアー）

```markdown
---
name: reviewer
role: コードレビュアー
skills:
  - Rust
  - 静的解析
  - セキュリティレビュー
description: 安全性とパフォーマンスを重視するレビュアー
allowed_tools:
  - shell
  - fs_read
denied_tools:
  - fs_write
---

あなたは熟練のコードレビュアーです。常に以下を意識してレビューしてください。

- 所有権・ライフタイム上の問題を最優先で指摘する
- パフォーマンスへの影響を定量的に述べる
- 修正案を提示する際は最小差分を心がける
```

### 7.3 `planner.md`（指示出し役）

```markdown
---
name: planner
role: プランナー
skills:
  - 計画立案
  - 要件分析
  - サブタスク分解
description: 大規模タスクをサブタスクに分解して進捗を管理する
---

あなたはプロジェクトのプランナーです。
- ユーザの要望を箇条書きで分解し、優先順位を付けて整理してください。
- 必要に応じて`send_to`で実装担当エージェントへタスクを割り振ってください。
```

サンプルをそのままコピーして開始するのが最速：

```bash
mkdir -p ~/.config/agent-cli/agents
cp example/agents/{coder,reviewer,planner}.md ~/.config/agent-cli/agents/
```

---

## 8. 運用シナリオ

### 8.1 単独エージェント

```bash
cp example/agents/coder.md ~/.config/agent-cli/agents/me.md
agent-cli run --name me
```

REPL ヘッダーに `role: Rust ソフトウェアエンジニア` などが表示されれば反映 OK。

### 8.2 マルチエージェント協調（planner ＋ coder ＋ reviewer）

```bash
# 共有 registry を使う設定（3 ターミナルで共通）
cat > /tmp/team.toml <<EOF
[provider]
kind = "claude"
[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
[runtime]
registry_dir = "/tmp/agent-cli/team"
agents_dir = "$HOME/.config/agent-cli/agents"
EOF

# ターミナル A
agent-cli --config /tmp/team.toml run --name planner

# ターミナル B
agent-cli --config /tmp/team.toml run --name coder

# ターミナル C
agent-cli --config /tmp/team.toml run --name reviewer
```

A から `/list` を打つと B／C が `role`／`skills` 付きで見え、`/peer coder` で coder の概要、`/send coder "<task>"` で実装依頼が飛びます。

### 8.3 同名でロール違いを切り替えたい

`--persona` で都度上書きすればファイル名規約と独立に運用できます。

```bash
# 平日：レビュアー
agent-cli --persona ~/personas/strict_reviewer.md

# 週末：自由記述
agent-cli --persona ~/personas/casual_chat.md
```

### 8.4 ホワイトリストで安全運用

CI 用ワーカーは破壊的操作を一切させたくない、というケース：

```yaml
---
name: ci-worker
role: CI ヘルパー
allowed_tools:
  - fs_read
---
```

`tools.enabled` がもっと多くても、このエージェントには `fs_read` だけが見えます。

---

## 9. REPL での操作

| コマンド | 用途 |
|---------|------|
| `/persona` | 自身のペルソナ詳細（`name`／`role`／`skills`／`description`／`temperature`／`allowed_tools`／`denied_tools`／`source`） |
| `/reload-persona` | 同じ解決経路で再読込し、システムプロンプトを差し替え（会話履歴は維持） |
| `/peer <id_or_name>` | ピアのペルソナ概要（`role`／`skills`／`description`） |
| `/tools` | 当該エージェントで実際に有効なツール一覧（ペルソナ反映後） |

`/reload-persona` の挙動：

1. 起動時と同じ優先順位で再解決
2. システムプロンプト（`Message::System`）を新しい内容で差し替え
3. 会話履歴の `User`／`Assistant`／`ToolResult` メッセージは保持
4. ツールレジストリは現状再構築されません（`allowed_tools`／`denied_tools` の変更を反映するには再起動）
5. `model`／`temperature` の上書きも再起動時のみ反映

---

## 10. レジストリへの反映

`<registry_dir>/<agent-id>.json` の `persona` フィールド：

```json
{
  "id": "agent-01HX...",
  "name": "alice",
  "provider": "claude",
  "model": "claude-opus-4-7",
  "socket": "/tmp/agent-cli/agent-01HX....sock",
  "persona": {
    "role": "コードレビュアー",
    "skills": ["Rust", "静的解析", "セキュリティレビュー"],
    "description": "安全性とパフォーマンスを重視するレビュアー",
    "source_path": "/home/.../agents/alice.md"
  }
}
```

これにより別プロセスから `/peer alice` を打つだけで、相手の役割・スキル・説明が確認できます。`/send` の宛先選定にも有用です。

---

## 11. トラブルシューティング

### `persona file not found: <path>`

- `--persona` または `[runtime] persona_file` で指定したパスが存在しません。
- 既定パス（`<agents_dir>/<name>.md`）の場合は組み込み既定にフォールバックするため、このメッセージは出ません。
- 解決：パスを `agent-cli config show` で確認、または `--persona` を相対パス → 絶対パスに直す。

### `\`role\` is required in persona frontmatter`

- フロントマターに `role:` が無い、または空文字列。
- 解決：必ず `role: <文字列>` を設定する。最低限の例：
  ```yaml
  ---
  role: 汎用アシスタント
  ---
  ```

### `persona file must begin with YAML frontmatter (\`---\`)`

- ファイルが `---` で始まっていない。本文だけのファイルは扱えません。
- 解決：先頭に `---\nrole: ...\n---\n` を追加。

### `invalid YAML frontmatter: ...`

- フロントマターが YAML として不正（インデント崩れ、未クォートのコロン、リストの `- ` 不足など）。
- 解決：YAML として `python -c 'import yaml,sys;yaml.safe_load(sys.stdin)'` 等で先に検証。

### `/reload-persona` してもツール権限が変わらない

- `allowed_tools`／`denied_tools` の差し替えは現状 `ToolRegistry` を再構築しないため、`agent-cli` を再起動してください。
- システムプロンプトのみは即時反映されます。

### REPL ヘッダーに別の `role` が出る

- 名前規約（`<agents_dir>/<name>.md`）が CLI の `--persona` で上書きされている可能性。`/persona` の `source` 行で実際に読み込まれたパスを確認できます。

### サンプルペルソナをカスタマイズしたい

- `example/agents/*.md` をコピーして `~/.config/agent-cli/agents/` に配置すると、`--name` で読み込まれるようになります。元のサンプルは将来のアップデートで上書きされる可能性があるため、必ず別名コピーを推奨。

---

## 12. 仕様サマリ（チートシート）

```text
解決順序：       --persona > [runtime] persona_file > <agents_dir>/<name>.md > builtin
必須キー：       role
任意キー：       name / skills / description / model / temperature / allowed_tools / denied_tools
ツール選定：     enabled ∩ allowed_tools \ denied_tools
モデル上書き：   起動時のみ。/reload-persona では反映されない
温度上書き：     起動時のみ。同上
REPL：          /persona, /reload-persona, /peer <id>, /tools
レジストリ：    <registry_dir>/<agent-id>.json の persona フィールドに反映
バリデーション： cargo test bundled_example_personas_parse で example/agents/*.md を常時検証
```
