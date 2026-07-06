# MCP Client Setup

[简体中文](client-setup.zh-CN.md) | [Русский](client-setup.ru.md) | [日本語](client-setup.ja.md)

RHoiScribe is a local MCP server launched through stdio. It is intended for Codex, Claude Code, and other MCP-compatible clients that can start a local command.

For a feature-level overview after setup, see [features.md](features.md).

## Download

Download a prebuilt binary from [GitHub Releases](https://github.com/czxieddan/RHoiScribe/releases):

- Windows: `rhoiscribe-windows-x86_64.exe`
- Linux: `rhoiscribe-linux-x86_64`
- macOS: `rhoiscribe-macos-universal`

Skill packages are available for agents that can read a local Skill folder:

- Windows: `rhoiscribe-skill-windows-x86_64.zip`
- Linux: `rhoiscribe-skill-linux-x86_64.zip`
- macOS: `rhoiscribe-skill-macos-universal.zip`

Each Skill package contains `SKILL.md` and the matching executable. Use it when you want direct agent access to RHoiScribe prompts, resources, and tools without adding an MCP server entry.

Keep the downloaded file in a stable folder. On Linux and macOS, run `chmod +x` on the downloaded file if the system asks for executable permission.

Build from source only when you want a local Cargo build:

```powershell
cargo build --release
```

## Paths

Use placeholders in docs and committed examples. Replace them only in your private client configuration:

- `<RHOISCRIBE_COMMAND>`: absolute path printed by `--print-command`.
- `<ABSOLUTE_PATH_TO_RHOISCRIBE>`: absolute path to this repository or release folder on the user's machine.
- `<MOD_OUTPUT_ROOT>`: absolute path to a HOI4 mod folder when a generation tool writes files.

Print the command path:

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

Expected binary paths:

- Prebuilt Windows: `<ABSOLUTE_PATH_TO_RHOISCRIBE>\rhoiscribe-windows-x86_64.exe`
- Prebuilt Linux: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/rhoiscribe-linux-x86_64`
- Prebuilt macOS: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/rhoiscribe-macos-universal`
- Local Cargo build on Windows: `<ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe`
- Local Cargo build on Linux or macOS: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe`

## Codex

Add RHoiScribe to the Codex MCP server configuration using the release binary as the command.

```toml
[mcp_servers.rhoiscribe]
command = "<RHOISCRIBE_COMMAND>"
args = []
```

For Windows TOML strings, escape backslashes or use a path style accepted by your client:

```toml
[mcp_servers.rhoiscribe]
command = "<RHOISCRIBE_COMMAND>"
args = []
```

If your Codex surface uses a different config location, keep the same server name, command path, and empty `args` shape.

## Claude Code

Claude Code can register local stdio MCP servers from its MCP configuration or CLI. Use the release binary as the command.

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

For Windows JSON strings, escape backslashes:

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

CLI-style registration can use the same command path:

```powershell
claude mcp add rhoiscribe -- <RHOISCRIBE_COMMAND>
```

## Generic MCP JSON

Many MCP clients accept a server map with `command` and `args` fields:

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

Windows clients usually need the `.exe` path and escaped backslashes in JSON:

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

## Direct Skill Commands

Direct Skill commands return JSON and expose the same prompts, resources, and tools as the MCP server:

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

MCP server mode keeps CWT language workspaces warm in process memory across tool calls. Direct `--skill` calls expose the same tools and resources, but each command is a short-lived process, so warm CWT state is rebuilt per command instead of reused.

## Runtime Basics

- Transport: stdio.
- Network: no runtime network access is required.
- Prompts: available through `prompts/list` and `prompts/get`.
- Resources: available through `resources/list` and `resources/read`.
- Tools: available through `tools/list` and `tools/call`.
- Feature details and recommended workflows: [features.md](features.md).

## Smoke Test

After adding the server to a client, ask the client to list MCP resources and read:

```text
rhoiscribe://hoi4/knowledge/catalog
```

Then call a read-only tool such as `search_hoi4_knowledge` before using any write-capable tool.

For broader validation, generation, CWT language support, assets, or debug workflows, use [features.md](features.md).
