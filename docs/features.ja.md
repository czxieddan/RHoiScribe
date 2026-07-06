# RHoiScribe 機能ガイド

[English](features.md) | [简体中文](features.zh-CN.md) | [Русский](features.ru.md)

この document は、MCP server または Skill package の設定後に RHoiScribe で何ができるかを説明します。インストールと client configuration は [MCP セットアップガイド](client-setup.ja.md) を参照してください。

## 実行時の考え方

- Transport: local stdio MCP。
- Runtime network: 不要です。
- Prompts: `prompts/list` と `prompts/get` で取得できます。
- Resources: `resources/list` と `resources/read` で読みます。
- Tools: `tools/list` と `tools/call` で呼び出します。
- MCP mode は resident CWT language workspace を process memory に保持するため、連続した diagnostics、completion、reference analysis、複数回の編集に向いています。
- 直接 `--skill` commands は同じ prompts、resources、tools を使いますが、各 command は短命な process なので、温めた CWT state は次の command には残りません。

## CWT による言語支援

- CWT resources: `rhoiscribe://hoi4/cwt/catalog` と `rhoiscribe://hoi4/cwt/metadata` は、pinned HOI4 CWT rules crate、upstream revision、hash、virtual source prefix、no-runtime-disk policy を示します。
- CWT memory policy: embedded CWT rules は、compiled Cargo git dependency の static source table から process memory に読み込まれます。
- RHoiScribe は rule files を展開せず、CWT caches や CWT lock files を作らず、CWT language state を RNMDB に保存しません。
- CWT language tools は RHoiScribe tool-call logging を避けるため、CWT diagnostics と workspace language state は `.rhoiscribe` log store に書かれません。
- Workspace warm-up: MCP session の早い段階で current mod root を使って `open_hoi4_language_workspace` を呼び、その後 `get_hoi4_language_status` を polling して warm になるのを待ちます。
- mod root、rules override、vanilla root、ignore globs、language configuration が変わった場合は workspace を開き直してください。
- Project diagnostics: `validate_hoi4_project` は既定で hybrid CWT plus legacy checks を使います。古い挙動だけが必要なら `validation_mode = "legacy"`、CWT だけなら `validation_mode = "cwt"`、両方を明示したいなら `validation_mode = "hybrid"` を使います。
- File diagnostics: `validate_hoi4_file` は saved file または unsaved content を検証し、resident workspace handle があれば温めた状態を利用します。
- Diagnostic explanation: `explain_hoi4_diagnostic` は agent が読みやすい意味、原因の見当、修復の方向を返します。
- Language intelligence: `list_hoi4_workspace_symbols`、`find_hoi4_definition`、`find_hoi4_references`、`suggest_hoi4_completion`、`inspect_hoi4_scope`、`inspect_hoi4_type_rule` は locations、completions、scope context、applicable rule profiles を返します。
- Localisation assistance: `generate_missing_localisation` は reviewable dry-run localisation candidates と generated file content を返します。この tool 自体は書き込みません。確認後に returned entries を `generate_localisation_batch` へ渡して書き込みます。

推奨 CWT workflow:

```text
open_hoi4_language_workspace
get_hoi4_language_status
validate_hoi4_project
validate_hoi4_file
explain_hoi4_diagnostic
inspect_hoi4_scope
inspect_hoi4_type_rule
```

## プロジェクト品質の確認

- Project index: `index_hoi4_project` は mod root と optional game roots から structured definitions、references、files を返します。
- Project validation: `validate_hoi4_project` は CWT schema diagnostics、duplicate IDs、CWT parse diagnostics が使えない場合の brace balance、missing GUI/GFX/localisation links、`replace_path` risks を red/yellow/green checks として返します。
- Repair checks: `repair_hoi4_project` は dry-run または apply mode で UTF-8 BOM rules、Paradox script formatting、audio checks を扱えます。
- ffmpeg が見つからない場合、dry-run は案内を返します。user approval の後だけ、`dry_run=false` と `install_ffmpeg=true` で silent installation attempt を許可します。

推奨 project check workflow:

```text
index_hoi4_project
validate_hoi4_project
repair_hoi4_project with dry_run = true
```

repair apply mode は、返された変更を確認してから使ってください。

## 生成と編集

- Write mode: generation tools には `dry_run = false` と `output_root = "<MOD_OUTPUT_ROOT>"` が必要です。
- Localisation generation: `generate_localisation_batch` には明示的な key/value entries を渡します。説明文は別の `_desc` key/value entry として渡してください。
- Existing-file edits: `edit_hoi4_script_file` は既存の HOI4 script file 内で named blocks を置換または挿入し、dry-run preview と brace checks を返します。
- 現在の mod または workspace の `workspace_root` を渡し、対象 file がその tree から出ないようにしてください。
- focus、event、decision、skeleton の batch tools は新しい file skeleton を作る用途に向いています。既存 file の細かい logic は `edit_hoi4_script_file` で編集します。

推奨 localisation workflow:

```text
generate_missing_localisation
review returned entries
generate_localisation_batch
```

返される file path は、有効な `localisation/<language>/` tree の下にあるべきです。user の mod に既存の nested directories がある場合は、それに合わせて構いません。Filename は通常の `_l_<language>.yml` suffix を使い、encoding は `utf-8-bom` にします。

## GUI と GFX アセット

- Experimental assets: `generate_gui_gfx_asset` は local procedural PNG files、`.gfx` sprite registration、optional `.gui` files を作れます。外部 image models には依存しません。
- 書き込みには `approved=true` が必要です。
- まず existing workspace art を優先してください。user が新しい procedural GUI/GFX assets の作成に同意した後だけ `approved=true` を設定します。
- animated GUI sprites では existing sprite-sheet conventions と `frameAnimatedSpriteType` knowledge を使い、generated static sprite を animation として扱わないでください。

推奨 asset workflow:

```text
generate_gui_gfx_asset with dry_run = true
review returned files and metadata
generate_gui_gfx_asset with approved=true only after user approval
```

## 環境検出とデバッグ

- Environment discovery: `discover_hoi4_environment` は、local HOI4 が installed されている場合に `<HOI4_GAME_PATH>`、`game_executable_path`、`<HOI4_DOCUMENT_PATH>`、`error_log_path`、game version を見つけます。
- Debug preflight: `validate_hoi4_debug_run` は launcher descriptors、playset state、clean document folders を確認し、必要なら `hoi4.exe -gdpr-compliant -debug_mode` も起動できます。
- Rchadow debug launch: `launch_hoi4_debug_with_rchadow` は debug playset を準備し、memory または disk mode を選び、必要なら Rchadow 経由で HOI4 を起動します。

推奨 debug workflow:

```text
discover_hoi4_environment
validate_hoi4_debug_run with launch = false
launch_hoi4_debug_with_rchadow with launch = false
```

preflight result が green で、user が RHoiScribe に game を起動させたい場合だけ `launch = true` を設定します。

## 設定とログ

- Agent preferences: `list_agent_preferences`、`set_agent_preference`、`delete_agent_preference` は cross-IDE habits を RNMDB-backed `.rhoiscribe` store に保存します。
- Tool logs: `query_tool_logs` と `export_tool_logs` は同じ `.rhoiscribe` store から recent non-CWT tool calls を読み、optional regex filtering に対応します。
- Log triage: `classify_error_log` は `error.log` entries を probable HOI4 subsystem ごとにまとめ、changed mod-relative paths と関連付けることもできます。

ログを直接読むコマンド:

```powershell
.\rhoiscribe-windows-x86_64.exe --logs "generate_.*"
.\rhoiscribe-windows-x86_64.exe --export-logs rhoiscribe-tool-logs.json "error|failed"
```

Linux と macOS の downloaded binaries でも同じ arguments を使います。
