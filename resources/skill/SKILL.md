---
name: rhoiscribe-hoi4
description: Use when an agent needs local Hearts of Iron IV modding prompts, resources, or tools from a downloaded RHoiScribe Skill package without configuring an MCP server.
---

# RHoiScribe HOI4

Use the RHoiScribe executable in the same directory as this `SKILL.md` when HOI4 modding work needs local prompts, bundled resources, or batch tools.

## Find The Binary

Use the executable shipped beside this file:

- Windows: `rhoiscribe-windows-x86_64.exe`
- Linux: `rhoiscribe-linux-x86_64`
- macOS: `rhoiscribe-macos-universal`

On Linux or macOS, run `chmod +x ./rhoiscribe-linux-x86_64` or `chmod +x ./rhoiscribe-macos-universal` if the shell reports a permission error.

## Direct Commands

These commands return JSON and use the same prompt, resource, and tool catalogs as the MCP server:

```bash
./rhoiscribe-linux-x86_64 --skill list-tools
./rhoiscribe-linux-x86_64 --skill list-resources
./rhoiscribe-linux-x86_64 --skill list-prompts
./rhoiscribe-linux-x86_64 --skill read-resource "rhoiscribe://hoi4/latest-update"
./rhoiscribe-linux-x86_64 --skill get-prompt "hoi4_mod_planner" '{"request":"plan an industrial focus branch"}'
./rhoiscribe-linux-x86_64 --skill call-tool "search_hoi4_knowledge" '{"query":"on_actions ROOT FROM"}'
```

Use the platform executable name for the current system. On Windows, quote JSON for PowerShell:

```powershell
.\rhoiscribe-windows-x86_64.exe --skill call-tool "format_paradox_script" '{ "script": "focus={id=TAG_focus cost=10}" }'
```

## Agent Rules

- Read RHoiScribe resources before searching the web.
- Use `scan_unique_identifiers` before creating new IDs, flags, variables, tags, ideas, focuses, decisions, characters, scripted triggers, or scripted effects.
- Use `index_hoi4_project` and `validate_hoi4_project` before broad edits so references, missing assets, localisation keys, and duplicate definitions are checked across the project.
- Use `repair_hoi4_project` in dry-run mode before applying encoding, formatting, or audio fixes. If ffmpeg is missing, ask for user approval before installing it.
- Use `edit_hoi4_script_file` for targeted changes to existing files instead of regenerating whole files.
- Use `generate_gui_gfx_asset` only when the user approves new experimental procedural GUI/GFX assets. Pass `approved=true`; otherwise reuse existing project art.
- Prefer existing workspace paths and naming conventions before official fallback conventions.
- Keep file names, folder names, and HOI4 token identifiers ASCII-only unless they are player-facing localisation text.
- Deliver complete game-readable files, not sketches, TODO placeholders, or partial drafts.
