# RHoiScribe Feature Guide

[简体中文](features.zh-CN.md) | [Русский](features.ru.md) | [日本語](features.ja.md)

This document describes RHoiScribe capabilities after the MCP server or Skill package is configured. For installation and client configuration, see [client-setup.md](client-setup.md).

## Runtime Model

- Transport: local stdio MCP.
- Runtime network: not required.
- Prompts: available through `prompts/list` and `prompts/get`.
- Resources: available through `resources/list` and `resources/read`.
- Tools: available through `tools/list` and `tools/call`.
- MCP mode keeps resident CWT language workspaces warm in process memory across tool calls.
- Direct `--skill` commands expose the same prompts, resources, and tools, but each command is a short-lived process, so warm CWT state is rebuilt per command.

## CWT Language Support

- CWT resources: `rhoiscribe://hoi4/cwt/catalog` and `rhoiscribe://hoi4/cwt/metadata` describe the pinned HOI4 CWT rules crate, upstream revision, hash, virtual source prefix, and no-runtime-disk policy.
- CWT memory policy: embedded CWT rules are loaded from the compiled Cargo git dependency's static source table into process memory.
- RHoiScribe does not extract rule files, create CWT caches, create CWT lock files, or store CWT language state in RNMDB.
- CWT language tools skip RHoiScribe tool-call logging so CWT diagnostics and workspace language state are not written to the `.rhoiscribe` log store.
- Workspace warm-up: when a mod workspace is available, call `discover_hoi4_environment` if the HOI4 game root is not already known, pass `game_path` as `vanilla_root` to `open_hoi4_language_workspace` when vanilla-aware CWT context is useful, then poll `get_hoi4_language_status` until the workspace is warm.
- Conversation-only analysis: if the user only pasted HOI4 script in chat and no workspace or saved file is involved, call `validate_hoi4_file` with `content`. A real path is not required; RHoiScribe uses an in-memory virtual HOI4 path when `path` is omitted.
- Reopen the workspace when the mod root, rules override, vanilla root, ignore globs, or language configuration changes.
- Project diagnostics: `validate_hoi4_project` defaults to hybrid CWT plus legacy checks. Use `validation_mode = "legacy"` for legacy-only behavior, `validation_mode = "cwt"` for CWT-only behavior, or `validation_mode = "hybrid"` explicitly when you want both.
- File diagnostics: `validate_hoi4_file` validates one saved file or unsaved content with embedded rules and an optional resident workspace handle.
- Diagnostic explanations: `explain_hoi4_diagnostic` gives model-facing meaning, likely cause, and repair guidance.
- Language intelligence: `list_hoi4_workspace_symbols`, `find_hoi4_definition`, `find_hoi4_references`, `suggest_hoi4_completion`, `inspect_hoi4_scope`, and `inspect_hoi4_type_rule` return locations, completions, scope context, and applicable rule profiles.
- Localisation assistance: `generate_missing_localisation` returns reviewable dry-run localisation candidates and generated file content. It never writes files; use `generate_localisation_batch` with the returned entries only after write approval.

Recommended CWT workflow:

```text
open_hoi4_language_workspace
get_hoi4_language_status
validate_hoi4_project
validate_hoi4_file
explain_hoi4_diagnostic
inspect_hoi4_scope
inspect_hoi4_type_rule
```

## Project Quality Tools

- Project index: `index_hoi4_project` returns structured definitions, references, and files for a mod root and optional game roots.
- Project validation: `validate_hoi4_project` returns red/yellow/green static checks for CWT schema diagnostics, duplicate IDs, brace balance where CWT parse diagnostics are not available, missing GUI/GFX/localisation links, and `replace_path` risks.
- Repair checks: `repair_hoi4_project` can dry-run or apply UTF-8 BOM rules, Paradox script formatting, and audio checks.
- If ffmpeg is missing, dry-run returns guidance. After user approval, `dry_run=false` with `install_ffmpeg=true` allows a silent installation attempt.

Recommended project check workflow:

```text
index_hoi4_project
validate_hoi4_project
repair_hoi4_project with dry_run = true
```

Only use repair apply mode after reviewing the returned changes.

## Generation And Editing

- Write mode: generation tools require `dry_run = false` and `output_root = "<MOD_OUTPUT_ROOT>"`.
- Localisation generation: call `generate_localisation_batch` with explicit key/value entries. Use separate `_desc` entries for description text.
- Existing-file edits: `edit_hoi4_script_file` replaces or inserts named blocks in an existing HOI4 script file with dry-run preview and brace checks.
- Pass `workspace_root` for the current mod or workspace so edit targets are restricted to that tree.
- Batch focus, event, decision, and skeleton tools are intended as new-file skeleton builders; use `edit_hoi4_script_file` for detailed logic inside existing files.

Recommended localisation workflow:

```text
generate_missing_localisation
review returned entries
generate_localisation_batch
```

The returned file path should stay under a valid `localisation/<language>/` tree, including nested subdirectories when they match the user's mod. Filenames use the usual `_l_<language>.yml` suffix, and the encoding should be `utf-8-bom`.

## GUI And GFX Assets

- Experimental assets: `generate_gui_gfx_asset` can create local procedural PNG files, `.gfx` sprite registration, and optional `.gui` files without external image models.
- Writing requires `approved=true`.
- Prefer existing workspace art first. Set `approved=true` only after the user agrees to create new procedural GUI/GFX assets instead of reusing existing project art.
- For animated GUI sprites, use existing sprite-sheet conventions and `frameAnimatedSpriteType` knowledge instead of assuming a generated static sprite is animated.

Recommended asset workflow:

```text
generate_gui_gfx_asset with dry_run = true
review returned files and metadata
generate_gui_gfx_asset with approved=true only after user approval
```

## Environment And Debug

- Environment discovery: `discover_hoi4_environment` can find `<HOI4_GAME_PATH>`, `game_executable_path`, `<HOI4_DOCUMENT_PATH>`, `error_log_path`, and game version when local HOI4 is installed.
- Debug preflight: `validate_hoi4_debug_run` checks launcher descriptors, playset state, clean document folders, and can optionally launch `hoi4.exe -gdpr-compliant -debug_mode`.
- Rchadow debug launch: `launch_hoi4_debug_with_rchadow` can prepare a debug playset, choose memory or disk mode, and optionally start HOI4 through Rchadow.

Recommended debug workflow:

```text
discover_hoi4_environment
validate_hoi4_debug_run with launch = false
launch_hoi4_debug_with_rchadow with launch = false
```

Only set `launch = true` after the preflight result is green and the user wants RHoiScribe to start the game.

## Preferences And Logs

- Agent preferences: `list_agent_preferences`, `set_agent_preference`, and `delete_agent_preference` persist cross-IDE habits in an RNMDB-backed `.rhoiscribe` store.
- Tool logs: `query_tool_logs` and `export_tool_logs` read recent non-CWT tool calls from the same RNMDB-backed `.rhoiscribe` store as agent preferences, with optional regex filtering.
- Log triage: `classify_error_log` groups `error.log` entries by likely HOI4 subsystem and can correlate entries with changed mod-relative paths.

Direct log commands:

```powershell
.\rhoiscribe-windows-x86_64.exe --logs "generate_.*"
.\rhoiscribe-windows-x86_64.exe --export-logs rhoiscribe-tool-logs.json "error|failed"
```

Linux and macOS use the same arguments on their downloaded binaries.
