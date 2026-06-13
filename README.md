<div align="center">

<img src="resources/RHoiScribe.ico" alt="RHoiScribe" width="128" height="128">

<h1 align="center">RHoiScribe</h1>

Local MCP server for Hearts of Iron IV modding agents

[简体中文](docs/README.zh-CN.md) | [Русский](docs/README.ru.md) | [日本語](docs/README.ja.md)

[![GitHub Stars](https://img.shields.io/github/stars/czxieddan/RHoiScribe?style=for-the-badge&label=Stars)](https://github.com/czxieddan/RHoiScribe/stargazers)
[![License](https://img.shields.io/badge/License-AGPL--3.0--or--later-blue?style=for-the-badge)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024-orange?style=for-the-badge)](Cargo.toml)
[![MCP](https://img.shields.io/badge/MCP-stdio-green?style=for-the-badge)](docs/client-setup.md)

If RHoiScribe helps your modding workflow, starring the repository helps other HOI4 mod authors find it.

</div>

RHoiScribe gives Codex, Claude Code, and other MCP-compatible clients a local HOI4 modding reference layer plus tools for generating game-readable files.

The goal is simple: reduce wasted agent work caused by repeated web searches, stale assumptions, unsafe file paths, missing localisation encoding, and Paradox script that looks plausible but does not load in game.

<h2 align="center">Environment</h2>

<table align="center">
  <tr>
    <th align="center">Area</th>
    <th align="center">Value</th>
  </tr>
  <tr>
    <td align="center">Server transport</td>
    <td align="center">MCP over stdio</td>
  </tr>
  <tr>
    <td align="center">Implementation</td>
    <td align="center">Rust 2024</td>
  </tr>
  <tr>
    <td align="center">Build tool</td>
    <td align="center">Cargo</td>
  </tr>
  <tr>
    <td align="center">Primary clients</td>
    <td align="center">Codex, Claude Code, MCP-compatible clients</td>
  </tr>
  <tr>
    <td align="center">Runtime network</td>
    <td align="center">Not required for bundled prompts, resources, and tools</td>
  </tr>
  <tr>
    <td align="center">Modding target</td>
    <td align="center">Hearts of Iron IV local mods</td>
  </tr>
</table>

<h2 align="center">Who It Is For</h2>

- Mod authors who want AI agents to generate HOI4 content with better local context.
- Agent workflows that need prompts, resources, and tools available through one MCP server.
- Offline or low-search development sessions where the agent should read bundled HOI4 guidance before writing files.
- Teams that want generated content to follow predictable mod-root paths and reviewable output shapes.

<h2 align="center">What Agents Get</h2>

<h3 align="center">Prompts</h3>

Agents can use built-in prompts for:

- mod feature planning
- HOI4 script writing
- localisation writing
- GUI, GFX, and scripted GUI work
- generated-content review

Prompt names currently include `hoi4_mod_planner`, `hoi4_script_writer`, `hoi4_localisation_writer`, `hoi4_gui_assistant`, and `hoi4_review`.

<h3 align="center">Resources</h3>

Agents can read local resources instead of starting from a blank prompt:

- `rhoiscribe://hoi4/latest-update`
- `rhoiscribe://hoi4/knowledge/catalog`
- `rhoiscribe://hoi4/knowledge/<topic_id>`

The knowledge catalog is structured for agent use. Topics contain category, file types, tags, syntax examples, relationships to other HOI4 systems, validation guidance, and source references. Current coverage includes script basics, scopes, triggers, effects, modifiers, variables, MTTH variables, unique identifier checks, arrays, localisation, scripted localisation, scripted triggers/effects, GUI, scripted GUI, focuses, events, detailed on_action scope families, decisions, missions, ideas, characters, history, map files, technology, equipment, units, AI, diplomacy, game rules, defines, bookmarks, audio, game path discovery, debug launch checks, error log triage, and common loading errors.

<h3 align="center">Tools</h3>

Agents can call tools for repeatable generation and validation:

- `generate_localisation_batch`
- `generate_focus_batch`
- `generate_event_batch`
- `generate_decision_batch`
- `search_hoi4_knowledge`
- `scan_unique_identifiers`
- `index_hoi4_project`
- `validate_hoi4_project`
- `repair_hoi4_project`
- `edit_hoi4_script_file`
- `generate_gui_gfx_asset`
- `discover_hoi4_environment`
- `validate_hoi4_debug_run`
- `classify_error_log`
- `validate_hoi4_paths`
- `format_paradox_script`

Generation tools support dry-run previews. In write mode they require an `output_root` and write paths relative to the target mod root.
Knowledge search returns matching topic IDs and MCP resource URIs for queries such as `mtth variables`, `decision mission blocks`, or `on_actions FROM.FROM`.
Identifier scanning checks batches of proposed new IDs against structured HOI4 definitions and reports duplicates, existing output files, and `replace_path` risks.
Project indexing builds a structured map of definitions, references, and files for flags, variables, scripted triggers/effects, focuses, events, GUI elements, GFX sprites, texture paths, and localisation keys.
Project validation returns red/yellow/green checks for duplicate definitions, brace balance, missing textures or sprites, missing localisation keys, and `replace_path` risks.
Project repair can dry-run or apply encoding and formatting fixes. It checks UTF-8 BOM rules, script formatting, `sound/` file types, and `music/` OGG metadata. If ffmpeg is needed and missing, dry-run returns installation guidance; after user approval, `dry_run=false` with `install_ffmpeg=true` allows a silent installation attempt.
Script editing modifies existing HOI4 script files by replacing or inserting named blocks with dry-run previews and brace checks.
The experimental GUI/GFX asset tool can generate local procedural PNG assets, `.gfx` sprite registration, and optional `.gui` files without external image models. Writing new assets requires `approved=true`.
Environment discovery finds the HOI4 install through Steam metadata first, then optional folder scanning, and reads `launcher-settings.json` for the document data path, `hoi4.exe` path, `logs/error.log` path, and game version.
Debug-run validation checks the document `map`, `localisation`, and `history` folders, active launcher mod descriptors, the current playset, dependency descriptors, and the workspace mod path before an optional `hoi4.exe -gdpr-compliant -debug_mode` launch.
Error-log classification groups `error.log` lines by likely HOI4 subsystem and links them back to changed paths when the agent has a diff or generated file list.

<h2 align="center">Help Improve RHoiScribe</h2>

HOI4 syntax and modding practice change over time. If you find bundled knowledge that is outdated, incomplete, or wrong, please open an [Issue](https://github.com/czxieddan/RHoiScribe/issues) with the game version, file type, source reference, and a minimal example when possible.

Pull requests are welcome for expanding the knowledge catalog, improving examples, or building more MCP tools for generation, validation, project scanning, and other agent workflows.

<h2 align="center">Quick Start</h2>

Download a prebuilt binary from [GitHub Releases](https://github.com/czxieddan/RHoiScribe/releases):

- Windows: `rhoiscribe-windows-x86_64.exe`
- Linux: `rhoiscribe-linux-x86_64`
- macOS: `rhoiscribe-macos-universal`

For agents that can read a Skill folder, download the matching Skill package:

- Windows: `rhoiscribe-skill-windows-x86_64.zip`
- Linux: `rhoiscribe-skill-linux-x86_64.zip`
- macOS: `rhoiscribe-skill-macos-universal.zip`

Unzip it into a stable folder. The package contains `SKILL.md` and the matching executable, so an agent can use RHoiScribe directly even when you do not want to configure an MCP server.

Keep the downloaded file in a stable folder. On Linux and macOS, run `chmod +x` on the downloaded file if the system asks for executable permission.

Build from source only when you want a local Cargo build:

```powershell
cargo build --release
```

Source builds place the executable under `<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/`.

Print the command path to use in your MCP client:

```powershell
.\rhoiscribe-windows-x86_64.exe --print-command
```

Linux and macOS users can run the same option on their downloaded file:

```bash
./rhoiscribe-linux-x86_64 --print-command
./rhoiscribe-macos-universal --print-command
```

Run it directly only when you want to start the stdio MCP server by hand:

```powershell
.\rhoiscribe-windows-x86_64.exe
```

```bash
./rhoiscribe-linux-x86_64
./rhoiscribe-macos-universal
```

Skill packages can also be called directly for JSON output:

```powershell
.\rhoiscribe-windows-x86_64.exe --skill list-tools
.\rhoiscribe-windows-x86_64.exe --skill list-resources
.\rhoiscribe-windows-x86_64.exe --skill list-prompts
.\rhoiscribe-windows-x86_64.exe --skill read-resource "rhoiscribe://hoi4/latest-update"
```

```bash
./rhoiscribe-linux-x86_64 --skill call-tool "search_hoi4_knowledge" '{"query":"on_actions ROOT FROM"}'
```

For Codex, Claude Code, and generic MCP configuration examples, see [docs/client-setup.md](docs/client-setup.md).

<h2 align="center">MCP Surface</h2>

After the client starts RHoiScribe, the agent can use standard MCP methods:

- `prompts/list`
- `prompts/get`
- `resources/list`
- `resources/read`
- `tools/list`
- `tools/call`

Example resource read:

```text
rhoiscribe://hoi4/knowledge/scripted_gui.dynamic_lists
```

Example environment discovery call:

```json
{
  "scan_fallback": true
}
```

Example debug preflight call:

```json
{
  "game_path": "<HOI4_GAME_PATH>",
  "document_path": "<HOI4_DOCUMENT_PATH>",
  "workspace_mod_path": "<MOD_OUTPUT_ROOT>",
  "launch": false
}
```

Example error log classification call:

```json
{
  "error_log_path": "<HOI4_DOCUMENT_PATH>/logs/error.log",
  "changed_paths": ["common/national_focus/CHI.txt"],
  "limit": 5
}
```

Example project validation call:

```json
{
  "roots": [
    {
      "path": "<MOD_OUTPUT_ROOT>",
      "role": "mod"
    }
  ],
  "include_game_roots": true
}
```

Example repair dry-run call:

```json
{
  "roots": [
    {
      "path": "<MOD_OUTPUT_ROOT>",
      "role": "mod"
    }
  ],
  "dry_run": true,
  "format_scripts": true,
  "check_media": true
}
```

Example experimental GUI/GFX asset dry-run:

```json
{
  "asset_name": "CHI_command_button",
  "sprite_name": "GFX_CHI_command_button",
  "width": 128,
  "height": 64,
  "style": "button",
  "primary_color": "#214a67",
  "secondary_color": "#d5b261",
  "approved": true,
  "dry_run": true
}
```

Example `tools/call` arguments for a localisation dry run:

```json
{
  "language": "l_simp_chinese",
  "file_stem": "common/autonomy/CHI",
  "key_prefix": "CHI",
  "entries": [
    {
      "id": "industrial_recovery",
      "title": "Industrial Recovery",
      "description": "Rebuild the industrial base."
    }
  ],
  "dry_run": true
}
```

Write mode adds a mod output root:

```json
{
  "language": "l_simp_chinese",
  "file_stem": "common/autonomy/CHI",
  "entries": [
    {
      "id": "industrial_recovery",
      "title": "Industrial Recovery"
    }
  ],
  "dry_run": false,
  "output_root": "<MOD_OUTPUT_ROOT>"
}
```

The generated localisation file is written with UTF-8 BOM when write mode is enabled.
Use `file_stem` values such as `common/autonomy/CHI`, or complete mod-relative paths such as `localisation/simp_chinese/common/autonomy/CHI_l_simp_chinese.yml`, when the user's mod already organizes localisation in nested folders.

<h2 align="center">Output Model</h2>

Generation tools return structured file plans:

```json
{
  "dry_run": true,
  "files": [
    {
      "path": "localisation/simp_chinese/common/autonomy/CHI_l_simp_chinese.yml",
      "encoding": "utf-8-bom",
      "summary": "HOI4 localisation file"
    }
  ],
  "messages": ["dry-run only; no files were written"]
}
```
Paths are mod-relative and can use nested HOI4-readable folders when they match the user's workspace. Unsafe paths, drive-prefixed paths, and traversal attempts are rejected before writing.
