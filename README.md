<div align="center">

# RHoiScribe

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

## Environment

| Area | Value |
| --- | --- |
| Server transport | MCP over stdio |
| Implementation | Rust 2024 |
| Build tool | Cargo |
| Primary clients | Codex, Claude Code, MCP-compatible clients |
| Runtime network | Not required for bundled prompts, resources, and tools |
| Modding target | Hearts of Iron IV local mods |

## Who It Is For

- Mod authors who want AI agents to generate HOI4 content with better local context.
- Agent workflows that need prompts, resources, and tools available through one MCP server.
- Offline or low-search development sessions where the agent should read bundled HOI4 guidance before writing files.
- Teams that want generated content to follow predictable mod-root paths and reviewable output shapes.

## What Agents Get

### Prompts

Agents can use built-in prompts for:

- mod feature planning
- HOI4 script writing
- localisation writing
- GUI, GFX, and scripted GUI work
- generated-content review

Prompt names currently include `hoi4_mod_planner`, `hoi4_script_writer`, `hoi4_localisation_writer`, `hoi4_gui_assistant`, and `hoi4_review`.

### Resources

Agents can read local resources instead of starting from a blank prompt:

- `rhoiscribe://hoi4/latest-update`
- `rhoiscribe://hoi4/knowledge/catalog`
- `rhoiscribe://hoi4/knowledge/<topic_id>`

The knowledge catalog is structured for agent use. Topics contain category, file types, tags, syntax examples, relationships to other HOI4 systems, validation guidance, and source references. Current coverage includes script basics, scopes, triggers, effects, modifiers, variables, arrays, localisation, scripted localisation, scripted triggers/effects, GUI, scripted GUI, focuses, events, decisions, ideas, characters, history, map files, technology, equipment, units, AI, diplomacy, game rules, defines, bookmarks, audio, and common loading errors.

### Tools

Agents can call tools for repeatable generation and validation:

- `generate_localisation_batch`
- `generate_focus_batch`
- `generate_event_batch`
- `generate_decision_batch`
- `validate_hoi4_paths`
- `format_paradox_script`

Generation tools support dry-run previews. In write mode they require an `output_root` and write paths relative to the target mod root.

## Quick Start

Build the server:

```powershell
cargo build --release
```

Use the release binary in your MCP client:

```text
<ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe
```

Linux and macOS clients should use:

```text
<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe
```

Run it directly only when you want to start the stdio MCP server by hand:

```powershell
.\target\release\rhoiscribe.exe
```

For Codex, Claude Code, and generic MCP configuration examples, see [docs/client-setup.md](docs/client-setup.md).

## MCP Surface

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

## Output Model

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
