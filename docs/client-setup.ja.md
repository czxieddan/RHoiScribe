# MCP セットアップガイド

[English](client-setup.md) | [简体中文](client-setup.zh-CN.md) | [Русский](client-setup.ru.md)

RHoiScribe は stdio で起動するローカル MCP server です。Codex、Claude Code、そしてローカルコマンドを起動できる MCP-compatible client で使えます。

設定後の具体的な機能や作業の流れは、[機能ガイド](features.ja.md) を参照してください。

## ダウンロード

[GitHub Releases](https://github.com/czxieddan/RHoiScribe/releases) から prebuilt binary をダウンロードします。

- Windows: `rhoiscribe-windows-x86_64.exe`
- Linux: `rhoiscribe-linux-x86_64`
- macOS: `rhoiscribe-macos-universal`

ローカル Skill folder を読める agent には Skill package も用意されています。

- Windows: `rhoiscribe-skill-windows-x86_64.zip`
- Linux: `rhoiscribe-skill-linux-x86_64.zip`
- macOS: `rhoiscribe-skill-macos-universal.zip`

Skill package には `SKILL.md` と対応する executable が入っています。MCP server を追加せずに、agent から RHoiScribe の prompts、resources、tools を直接使いたい場合に向いています。

ダウンロードしたファイルは、移動しない安定したフォルダーに置いてください。Linux と macOS で実行権限を求められた場合は、対象ファイルに `chmod +x` を実行します。

ローカル Cargo build が必要な場合だけ source からビルドします。

```powershell
cargo build --release
```

## パス

ドキュメントやコミットする設定例では placeholder を使い、実際のパスは自分の private client configuration だけで置き換えてください。

- `<RHOISCRIBE_COMMAND>`: `--print-command` が表示する絶対パス。
- `<ABSOLUTE_PATH_TO_RHOISCRIBE>`: この repository または release folder の絶対パス。
- `<MOD_OUTPUT_ROOT>`: 書き込み tool が使う HOI4 mod folder の絶対パス。

command path を表示します。

```powershell
.\rhoiscribe-windows-x86_64.exe --print-command
```

Linux:

```bash
./rhoiscribe-linux-x86_64 --print-command
```

macOS:

```bash
./rhoiscribe-macos-universal --print-command
```

よく使う binary path:

- Prebuilt Windows: `<ABSOLUTE_PATH_TO_RHOISCRIBE>\rhoiscribe-windows-x86_64.exe`
- Prebuilt Linux: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/rhoiscribe-linux-x86_64`
- Prebuilt macOS: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/rhoiscribe-macos-universal`
- Windows のローカル Cargo build: `<ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe`
- Linux または macOS のローカル Cargo build: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe`

## Codex

Codex の MCP server 設定に RHoiScribe を追加し、release binary を `command` に指定します。

```toml
[mcp_servers.rhoiscribe]
command = "<RHOISCRIBE_COMMAND>"
args = []
```

Windows の TOML string では backslash を escape するか、client が受け付ける path 表記を使ってください。

```toml
[mcp_servers.rhoiscribe]
command = "<RHOISCRIBE_COMMAND>"
args = []
```

Codex の画面や設定場所が異なる場合でも、server name、command path、空の `args` という形は同じです。

## Claude Code

Claude Code は MCP configuration または CLI から local stdio MCP server を登録できます。ここでも release binary を command として使います。

```json
{
  "mcpServers": {
    "rhoiscribe": {
      "command": "<RHOISCRIBE_COMMAND>",
      "args": []
    }
  }
}
```

Windows の JSON string では backslash を escape します。

```json
{
  "mcpServers": {
    "rhoiscribe": {
      "command": "<RHOISCRIBE_COMMAND>",
      "args": []
    }
  }
}
```

CLI で登録する場合も同じ command path を使えます。

```powershell
claude mcp add rhoiscribe -- <RHOISCRIBE_COMMAND>
```

## 汎用 MCP JSON

多くの MCP client は `command` と `args` を持つ server map を受け付けます。

```json
{
  "mcpServers": {
    "rhoiscribe": {
      "command": "<RHOISCRIBE_COMMAND>",
      "args": []
    }
  }
}
```

Windows client では通常 `.exe` path が必要で、JSON 内の backslash も escape します。

```json
{
  "mcpServers": {
    "rhoiscribe": {
      "command": "<RHOISCRIBE_COMMAND>",
      "args": []
    }
  }
}
```

## 直接 Skill コマンド

直接 Skill コマンドは JSON を返し、MCP server と同じ prompts、resources、tools を公開します。

```powershell
.\rhoiscribe-windows-x86_64.exe --skill list-tools
.\rhoiscribe-windows-x86_64.exe --skill list-resources
.\rhoiscribe-windows-x86_64.exe --skill list-prompts
.\rhoiscribe-windows-x86_64.exe --skill read-resource "rhoiscribe://hoi4/latest-update"
.\rhoiscribe-windows-x86_64.exe --skill call-tool "search_hoi4_knowledge" '{ "query": "on_actions ROOT FROM" }'
```

```bash
./rhoiscribe-linux-x86_64 --skill list-tools
./rhoiscribe-linux-x86_64 --skill list-resources
./rhoiscribe-linux-x86_64 --skill list-prompts
./rhoiscribe-linux-x86_64 --skill read-resource "rhoiscribe://hoi4/latest-update"
./rhoiscribe-linux-x86_64 --skill call-tool "search_hoi4_knowledge" '{"query":"on_actions ROOT FROM"}'
```

MCP server mode では、CWT language workspace を同じ process memory の中で温めておけます。直接 `--skill` でも同じ機能を呼び出せますが、各 command は短命な process なので、温めた CWT state は次の command には残りません。

## 実行時の基本

- Transport: stdio。
- Runtime network: 不要です。
- Prompts: `prompts/list` と `prompts/get` で利用できます。
- Resources: `resources/list` と `resources/read` で利用できます。
- Tools: `tools/list` と `tools/call` で利用できます。
- 機能の詳細と推奨 workflow は [機能ガイド](features.ja.md) を参照してください。

## スモークテスト

client に server を追加したら、MCP resources を一覧し、次の resource を読ませてください。

```text
rhoiscribe://hoi4/knowledge/catalog
```

その後、書き込み可能な tool を使う前に、`search_hoi4_knowledge` のような read-only tool を一度呼び出します。

project validation、生成、CWT language support、asset、debug workflow については [機能ガイド](features.ja.md) を参照してください。
