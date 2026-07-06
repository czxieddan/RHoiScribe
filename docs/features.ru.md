# Руководство по возможностям RHoiScribe

[English](features.md) | [简体中文](features.zh-CN.md) | [日本語](features.ja.md)

Этот документ описывает, что умеет RHoiScribe после настройки MCP server или Skill package. Установку и client configuration см. в [руководстве по MCP-настройке](client-setup.ru.md).

## Модель выполнения

- Transport: local stdio MCP.
- Runtime network: не требуется.
- Prompts: доступны через `prompts/list` и `prompts/get`.
- Resources: доступны через `resources/list` и `resources/read`.
- Tools: доступны через `tools/list` и `tools/call`.
- MCP mode держит resident CWT language workspaces прогретыми в process memory между tool calls. Это удобно для последовательной диагностики, completion, reference analysis и нескольких раундов правок.
- Прямые `--skill` commands используют те же prompts, resources и tools, но каждый command — короткоживущий process, поэтому прогретое CWT state не сохраняется между вызовами.

## Языковая поддержка CWT

- CWT resources: `rhoiscribe://hoi4/cwt/catalog` и `rhoiscribe://hoi4/cwt/metadata` описывают pinned HOI4 CWT rules crate, upstream revision, hash, virtual source prefix и no-runtime-disk policy.
- CWT memory policy: embedded CWT rules читаются из static source table, скомпилированной из Cargo git dependency, и живут в process memory.
- RHoiScribe не распаковывает rule files, не создает CWT caches или CWT lock files и не сохраняет CWT language state в RNMDB.
- CWT language tools пропускают RHoiScribe tool-call logging, поэтому CWT diagnostics и workspace language state не записываются в `.rhoiscribe` log store.
- Workspace warm-up: в начале MCP session вызовите `open_hoi4_language_workspace` с current mod root, затем polling `get_hoi4_language_status`, пока workspace не станет warm.
- Переоткройте workspace, если изменились mod root, rules override, vanilla root, ignore globs или language configuration.
- Project diagnostics: `validate_hoi4_project` по умолчанию использует hybrid CWT plus legacy checks. Для legacy-only используйте `validation_mode = "legacy"`; только для CWT — `validation_mode = "cwt"`; для явного запуска обоих режимов — `validation_mode = "hybrid"`.
- File diagnostics: `validate_hoi4_file` проверяет saved file или unsaved content и использует resident workspace handle, если он передан.
- Diagnostic explanations: `explain_hoi4_diagnostic` возвращает понятное для agent объяснение, вероятную причину и направление исправления.
- Language intelligence: `list_hoi4_workspace_symbols`, `find_hoi4_definition`, `find_hoi4_references`, `suggest_hoi4_completion`, `inspect_hoi4_scope` и `inspect_hoi4_type_rule` возвращают locations, completions, scope context и applicable rule profiles.
- Localisation assistance: `generate_missing_localisation` возвращает reviewable dry-run localisation candidates и generated file content. Сам tool не пишет файлы; после проверки returned entries можно передать в `generate_localisation_batch`.

Рекомендуемый CWT workflow:

```text
open_hoi4_language_workspace
get_hoi4_language_status
validate_hoi4_project
validate_hoi4_file
explain_hoi4_diagnostic
inspect_hoi4_scope
inspect_hoi4_type_rule
```

## Проверка качества проекта

- Project index: `index_hoi4_project` возвращает structured definitions, references и files для mod root и optional game roots.
- Project validation: `validate_hoi4_project` возвращает red/yellow/green static checks для CWT schema diagnostics, duplicate IDs, brace balance там, где CWT parse diagnostics недоступны, missing GUI/GFX/localisation links и `replace_path` risks.
- Repair checks: `repair_hoi4_project` может работать в dry-run или apply mode для UTF-8 BOM rules, Paradox script formatting и audio checks.
- Если ffmpeg отсутствует, dry-run вернет пояснение. После явного user approval можно использовать `dry_run=false` с `install_ffmpeg=true`, чтобы разрешить silent installation attempt.

Рекомендуемый project check workflow:

```text
index_hoi4_project
validate_hoi4_project
repair_hoi4_project with dry_run = true
```

Repair apply mode стоит использовать только после просмотра возвращенных изменений.

## Генерация и редактирование

- Write mode: generation tools требуют `dry_run = false` и `output_root = "<MOD_OUTPUT_ROOT>"`.
- Localisation generation: `generate_localisation_batch` принимает явные key/value entries. Текст описания лучше передавать отдельной `_desc` key/value entry.
- Existing-file edits: `edit_hoi4_script_file` заменяет или вставляет named blocks в существующий HOI4 script file, возвращая dry-run preview и brace checks.
- Передавайте `workspace_root` текущего mod или workspace, чтобы target file не вышел за пределы нужного дерева.
- Batch tools для focus, event, decision и skeleton лучше использовать как builders для новых файлов. Для точечной логики в существующих файлах используйте `edit_hoi4_script_file`.

Рекомендуемый localisation workflow:

```text
generate_missing_localisation
review returned entries
generate_localisation_batch
```

Returned file path должен оставаться под корректным `localisation/<language>/` tree. Если в mod уже есть nested directories, можно следовать существующей структуре. Filenames обычно используют suffix `_l_<language>.yml`, encoding должен быть `utf-8-bom`.

## GUI и GFX-активы

- Experimental assets: `generate_gui_gfx_asset` может создать local procedural PNG files, `.gfx` sprite registration и optional `.gui` files без внешних image models.
- Для записи требуется `approved=true`.
- Сначала предпочитайте existing workspace art. Устанавливайте `approved=true` только после user approval на создание новых procedural GUI/GFX assets.
- Для animated GUI sprites используйте existing sprite-sheet conventions и знание `frameAnimatedSpriteType`; не называйте generated static sprite анимацией.

Рекомендуемый asset workflow:

```text
generate_gui_gfx_asset with dry_run = true
review returned files and metadata
generate_gui_gfx_asset with approved=true only after user approval
```

## Окружение и отладка

- Environment discovery: `discover_hoi4_environment` может найти `<HOI4_GAME_PATH>`, `game_executable_path`, `<HOI4_DOCUMENT_PATH>`, `error_log_path` и game version, если HOI4 установлен локально.
- Debug preflight: `validate_hoi4_debug_run` проверяет launcher descriptors, playset state, clean document folders и при необходимости может запустить `hoi4.exe -gdpr-compliant -debug_mode`.
- Rchadow debug launch: `launch_hoi4_debug_with_rchadow` может подготовить debug playset, выбрать memory или disk mode и при необходимости запустить HOI4 через Rchadow.

Рекомендуемый debug workflow:

```text
discover_hoi4_environment
validate_hoi4_debug_run with launch = false
launch_hoi4_debug_with_rchadow with launch = false
```

Устанавливайте `launch = true` только если preflight result green и пользователь действительно хочет, чтобы RHoiScribe запустил игру.

## Предпочтения и журналы

- Agent preferences: `list_agent_preferences`, `set_agent_preference` и `delete_agent_preference` сохраняют cross-IDE habits в RNMDB-backed `.rhoiscribe` store.
- Tool logs: `query_tool_logs` и `export_tool_logs` читают recent non-CWT tool calls из того же `.rhoiscribe` store и поддерживают optional regex filtering.
- Log triage: `classify_error_log` группирует `error.log` entries по вероятным HOI4 subsystem и может связать их с changed mod-relative paths.

Прямые команды для журналов:

```powershell
.\rhoiscribe-windows-x86_64.exe --logs "generate_.*"
.\rhoiscribe-windows-x86_64.exe --export-logs rhoiscribe-tool-logs.json "error|failed"
```

В Linux и macOS downloaded binaries используют те же arguments.
