# MCP Client Setup

RHoiScribe is a local MCP server launched through stdio. It is intended for Codex, Claude Code, and other MCP-compatible clients that can start a local stdio command.

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

Use placeholders in docs and committed examples. Replace them only in your private client configuration:

- `<RHOISCRIBE_COMMAND>`: absolute path printed by `--print-command`.
- `<ABSOLUTE_PATH_TO_RHOISCRIBE>`: absolute path to this repository on the user's machine.
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

## Runtime Behavior

- Transport: stdio.
- Network: no runtime network access is required.
- Prompts: available through `prompts/list` and `prompts/get`.
- Resources: available through `resources/list` and `resources/read`.
- Tools: available through `tools/list` and `tools/call`.
- Write mode: generation tools require `dry_run = false` and `output_root = "<MOD_OUTPUT_ROOT>"`.
- Project index: `index_hoi4_project` returns structured definitions, references, and files for a mod root and optional game roots.
- Project validation: `validate_hoi4_project` returns red/yellow/green static checks for duplicate IDs, brace balance, missing GUI/GFX/localisation links, and `replace_path` risks.
- Repair checks: `repair_hoi4_project` can dry-run or apply UTF-8 BOM rules, Paradox script formatting, and audio checks. If ffmpeg is missing, ask the user before installing it; RHoiScribe returns guidance and does not install it silently.
- Existing-file edits: `edit_hoi4_script_file` replaces or inserts named blocks in an existing HOI4 script file with dry-run preview and brace checks.
- Experimental assets: `generate_gui_gfx_asset` can create local procedural PNG files, `.gfx` sprite registration, and optional `.gui` files without external image models. Writing requires `approved=true`.
- Environment discovery: `discover_hoi4_environment` can find `<HOI4_GAME_PATH>`, `game_executable_path`, `<HOI4_DOCUMENT_PATH>`, `error_log_path`, and game version when local HOI4 is installed.
- Debug preflight: `validate_hoi4_debug_run` checks launcher descriptors, playset state, clean document folders, and can optionally launch `hoi4.exe -gdpr-compliant -debug_mode`.
- Log triage: `classify_error_log` groups `error.log` entries by likely HOI4 subsystem and can correlate entries with changed mod-relative paths.

## Smoke Test

After adding the server to a client, ask the client to list MCP resources and read:

```text
rhoiscribe://hoi4/knowledge/catalog
```

Then call `generate_localisation_batch` with `dry_run = true` before allowing write mode. The returned file path should stay under a valid `localisation/<language>/` tree, including nested subdirectories when they match the user's mod, and the encoding should be `utf-8-bom`.

For project-level checks, call `index_hoi4_project`, then `validate_hoi4_project`, then `repair_hoi4_project` with `dry_run = true`. Only use repair apply mode after reviewing the returned changes.

For experimental asset generation, call `generate_gui_gfx_asset` with `dry_run = true` first. Set `approved=true` only after the user agrees to create new procedural GUI/GFX assets instead of reusing existing project art.

For local game validation, call `discover_hoi4_environment` first, then pass its returned paths into `validate_hoi4_debug_run` with `launch = false`. Only set `launch = true` after the preflight result is green and the user wants RHoiScribe to start the game.
