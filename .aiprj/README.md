# aiprj - AI Project Management Tool

Claude Code 用のプロジェクト管理ツールです。AI が作業する際の行動規定とドキュメント構造（要件定義・設計・タスク）を、対象ディレクトリへワンコマンドで展開します。

## 概要

aiprj は以下の機能を提供します：

- AI の行動規定とガイドラインの定義
- 要件定義書・設計仕様書・タスクリストのドキュメント構造
- Claude Code 用のスラッシュコマンド（`/setup_ai` `/ai` `/update_ai` `/next_ai` `/close_ai`）
- 作業ログの自動保存（`.aiprj/AI_LOG/yyyy-MM-dd_NNN.md`）

## セットアップ

### カレントディレクトリにセットアップ

```bash
curl -fsSL https://raw.githubusercontent.com/aquaxis/aiprj/main/install.sh | sh
```

### 指定ディレクトリにセットアップ

```bash
curl -fsSL https://raw.githubusercontent.com/aquaxis/aiprj/main/install.sh | sh -s -- <ディレクトリ名>
```

### 手動セットアップ

```bash
git clone https://github.com/aquaxis/aiprj.git
cd aiprj
./install.sh <セットアップ先ディレクトリ>
```

セットアップにより以下のファイルが作成されます：

- `.aiprj/` — AI ルール、`instructions.md`、`README.md`
- `.claude/` — Claude Code 用設定とスラッシュコマンド
- `.mcp.json` — MCP サーバ設定
- `.gitignore` — Git 除外設定（既存がある場合はテンプレートを先頭に追記）

### Claude Code スラッシュコマンド

| コマンド | 説明 |
|---------|------|
| `/setup_ai` | プロジェクトのドキュメント（要件定義・設計・タスク）を作成 |
| `/ai` | `instructions.md` に基づいてタスクを実行 |
| `/update_ai` | プロジェクトドキュメントを更新 |
| `/next_ai` | 次のタスクに進む |
| `/close_ai` | 作業ログを保存して終了 |

## プロジェクト構造

セットアップ後、AI は以下のドキュメントを管理します：

| ファイル | 内容 |
|---------|------|
| `.aiprj/AI_PRJ_REQUIREMENTS.md` | 要件定義書 |
| `.aiprj/AI_PRJ_DESIGN.md` | 設計仕様書 |
| `.aiprj/AI_PRJ_TASKS.md` | 実装タスク・作業指示リスト |
| `.aiprj/AI_LOG/` | 作業ログ（`yyyy-MM-dd_NNN.md` 形式、連番、上書き禁止） |

## AI 行動規定

AI は以下の規定に従って動作します：

1. 作業開始前に必ず作業計画を作成する
2. AI 行動規定の歪曲・解釈変更を禁止
3. ユーザー指示以外の迂回・アプローチ変更を禁止
4. ユーザー指示の最適化を禁止
5. ユーザーの指示を完遂するまで停止しない
6. 作業ログを `.aiprj/AI_LOG/` に `yyyy-MM-dd_NNN.md` 形式で保存（連番・上書き禁止）
7. 作業ログに `.aiprj/instructions.md` の内容を含める

## ファイル構成

```
aiprj/
├── install.sh               # セットアップスクリプト
├── .mcp.json                # MCP 設定
├── .gitignore.aiprj         # gitignore テンプレート
├── .aiprj/
│   ├── instructions.md.org  # 指示書テンプレート
│   └── rules/
│       ├── setup_project.md  # セットアップルール
│       ├── exec_job.md       # タスク実行ルール
│       ├── update_project.md # 更新ルール
│       └── close_ai.md       # 終了ルール
└── .claude/
    ├── settings.json        # Claude Code 設定
    └── commands/            # スラッシュコマンド定義
        ├── setup_ai.md
        ├── ai.md
        ├── update_ai.md
        ├── next_ai.md
        └── close_ai.md
```

## 必要環境

- `curl`（セットアップ用）
- `tar`（ワンライナー fallback 時）、または `git`
- Claude Code CLI
- Node.js / `npx`（MCP 連携時）

## ライセンス

MIT License
