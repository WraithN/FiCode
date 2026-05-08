<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh_CN.md">中文</a> |
  <a href="README.fr.md">Français</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.de.md">Deutsch</a>
</p>

# fi-code

Rust で構築されたターミナル AI Coding Agent CLI プログラム。REPL または TUI でユーザーと対話し、マルチターン会話、ツール呼び出し、セッション永続化、および MCP プロトコル拡張をサポートします。

## 機能

- **🤖 マルチモデル対応**：OpenAI 互換インターフェースと Anthropic インターフェースを統一し、ストリーミング SSE レスポンスをサポート
- **🔧 ツール呼び出し**：6 つの組み込みツール（`bash`、`read`、`write`、`edit`、`web_fetch`、`grep`）を搭載。モデルの応答に基づいて自動実行し、結果を返却
- **💬 セッション永続化**：JSON Lines 形式でセッションをローカルディスクにインクリメンタル書き込み、中断後の復帰をサポート
- **🖥️ デュアルモード対話**：従来の REPL 対話と `ratatui` ベースのフルターミナル TUI インターフェースをサポート
- **🛡️ 権限検証**：Bash などの高リスク操作に対してリスクグレーディング（Allow / Ask / Deny）を実施し、`sudo`、`rm -rf` および一般的なインジェクション攻撃を阻止
- **⚙️ 柔軟な設定**：`~/.config/fi-code/config.json` または `config.jsonc` をサポート。コメント、環境変数プレースホルダー、およびホットリロードに対応
- **🔗 MCP サポート**：Model Context Protocol を実装し、マルチサーバー管理（stdio/HTTP トランスポート）をサポート
- **📦 Skills システム**：拡張可能な Skill 登録および読み込みメカニズムをサポート

## クイックスタート

### 必要条件

- [Rust](https://rustup.rs/) 1.70+（最新の安定版を推奨）
- 対応する AI Provider API Key

### インストール

```bash
# リポジトリをクローン
git clone <repository-url>
cd fi-code

# ビルド
cargo build --release

# 実行（開発モード）
cargo run -- --help
```

### 設定

#### 方法 1：環境変数（最優先）

**OpenAI 互換：**
```bash
export OPENAI_API_KEY=sk-...
export OPENAI_BASE_URL=https://api.openai.com/v1
export OPENAI_MODEL_NAME=gpt-4o
```

**Anthropic：**
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export ANTHROPIC_BASE_URL=https://api.anthropic.com
export ANTHROPIC_MODEL_NAME=claude-3-7-sonnet-20250219
```

#### 方法 2：設定ファイル

設定ファイルのパス（優先順位で検索）：
- Linux/macOS: `~/.config/fi-code/config.jsonc` または `~/.config/fi-code/config.json`

例：
```json
{
  "model": "my-model",
  "provider": {
    "openai": {
      "name": "My Provider",
      "options": {
        "apiKey": "{env:MY_API_KEY}",
        "baseURL": "https://api.example.com/v1"
      },
      "models": {
        "my-model": {
          "name": "My Model",
          "limit": { "context": 200000, "output": 65536 }
        }
      }
    }
  }
}
```

`//` および `/* */` コメントをサポート。`apiKey` は `{env:VAR_NAME}` プレースホルダー構文をサポート。

### 使い方

```bash
# 対話型 REPL モード
cargo run -- -i

# TUI フルターミナルインターフェースモード
cargo run -- --tui

# 単一コマンドの実行
cargo run -- -c "Rust の Hello World を書いてください"

# 設定済みモデルの表示
cargo run -- --models

# セッション一覧の表示
cargo run -- -s

# 作業ディレクトリの指定
cargo run -- -i -w /path/to/project
```

## プロジェクト構造

```
src/
├── main.rs                 # プログラムのエントリポイント
├── agent/                  # Agent コアループとプロンプト管理
├── provider/               # モデル統合（OpenAI / Anthropic）
├── session/                # セッションとメッセージ管理
├── tools/                  # ツールレジストリと実装
├── config/                 # 設定読み込みとホットリロード
├── permission/             # 権限リスクグレーディング
├── tui/                    # ターミナル UI（ratatui）
├── mcp/                    # MCP プロトコルサポート
├── skills/                 # Skills システム
├── commands/               # スラッシュコマンド
└── utils/                  # 共通ユーティリティ
```

## 組み込みツール

| ツール | 説明 | リスクレベル |
|--------|------|-------------|
| `bash` | シェルコマンドの実行 | Ask（危険なコマンドは Deny） |
| `read` / `read_file` | ファイル内容の読み取り | Allow |
| `write` | ファイルへの書き込み | Ask |
| `edit` | ファイルの編集 | Ask |
| `web_fetch` | Web ページの取得と Markdown 変換 | Ask |
| `grep` | 正規表現によるファイル内容の検索 | Allow |

## セキュリティメカニズム

- **パスエスケープ防止**：すべてのファイル操作は `safe_path` チェックを通過し、作業ディレクトリを超えないことを保証
- **Bash サンドボックス**：環境変数をクリアし、最小限の必要な変数のみを保持。120 秒タイムアウト
- **権限グレーディング**：Deny（危険なコマンドを直接拒否）、Ask（対話確認）、Allow（読み取り専用操作を直接許可）
- **出力切り捨て**：ツールの返却内容は 50,000 文字に制限され、コンテキストの溢れを防止

## TUI ショートカット

TUI モードでは、以下のショートカットが利用可能です：

| ショートカット | 機能 |
|--------------|------|
| `Tab` / `Shift+Tab` | フォーカス領域の切り替え |
| `Ctrl+C` | 生成停止 / プログラム終了 |
| `Ctrl+B` | 左側ファイルドロワーの開閉 |
| `Ctrl+H` | 右側セッション履歴ドロワーの開閉 |
| `Ctrl+M` | モデル選択ドロップダウンの表示 |
| `Ctrl+T` | テーマ切り替え |
| `Ctrl+N` | 新規セッション |
| `Enter` | メッセージ送信 |
| `Shift+Enter` | 入力ボックス内で改行 |
| `Esc` | ドロワー/ドロップダウンを閉じる/メイン領域に戻る |
| `Ctrl+Up` / `PageUp` | チャット領域を上にスクロール |
| `Ctrl+Down` / `PageDown` | チャット領域を下にスクロール |

## 開発

```bash
# テストの実行
cargo test

# コードのフォーマット
cargo fmt

# Clippy 静的チェック
cargo clippy
```

## 技術スタック

| 依存関係 | 用途 |
|----------|------|
| `tokio` | 非同期ランタイム |
| `reqwest` | HTTP クライアント、SSE ストリーミングリクエスト |
| `serde` / `serde_json` | シリアライズとデシリアライズ |
| `anyhow` | エラー処理 |
| `rustyline` | ターミナル行読み取りと履歴 |
| `ratatui` / `crossterm` | TUI レンダリングと端末イベント |
| `colored` | ターミナルカラー出力 |
| `clap` | コマンドライン引数解析 |
| `notify` | 設定ファイルのホットリロード |
| `regex` | 正規表現マッチング |

## セッションストレージ

セッションデータは `.jsonl` 形式でプラットフォーム設定ディレクトリに保存されます：
- **Linux**: `~/.config/fi-code/sessions/`
- **macOS**: `~/Library/Application Support/fi-code/sessions/`
- **Windows**: `%APPDATA%\fi-code\sessions\`

## ライセンス

このプロジェクトは [MIT License](./LICENSE) の下でオープンソースとして提供されています。

Copyright (c) 2025 fi-code contributors.

---

> **注意**：このプロジェクトは初期開発段階にあります。API および設定形式は変更される可能性があります。
