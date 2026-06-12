<div align="center">

<img src="../resources/RHoiScribe.ico" alt="RHoiScribe" width="128" height="128">

<h1 align="center">RHoiScribe</h1>

Локальный MCP-сервер для Hearts of Iron IV modding agents

[English](../README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md)

[![GitHub Stars](https://img.shields.io/github/stars/czxieddan/RHoiScribe?style=for-the-badge&label=Stars)](https://github.com/czxieddan/RHoiScribe/stargazers)
[![License](https://img.shields.io/badge/License-AGPL--3.0--or--later-blue?style=for-the-badge)](../LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024-orange?style=for-the-badge)](../Cargo.toml)
[![MCP](https://img.shields.io/badge/MCP-stdio-green?style=for-the-badge)](client-setup.md)

Если RHoiScribe помогает вашему modding workflow, Star помогает другим авторам HOI4 mods найти проект.

</div>

RHoiScribe дает Codex, Claude Code и другим MCP-совместимым клиентам локальный справочный слой по HOI4 modding и инструменты для генерации файлов, читаемых игрой.

Цель проекта проста: уменьшить лишнюю работу agents из-за повторного веб-поиска, устаревших предположений, небезопасных путей, пропущенной кодировки локализации и Paradox script, который выглядит правдоподобно, но не загружается в игре.

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

<h2 align="center">Для кого</h2>

- Для авторов модов, которые хотят давать AI agents более сильный локальный контекст.
- Для agent workflows, где prompts, resources и tools должны быть доступны через один MCP server.
- Для офлайн-сессий или разработки с минимальным поиском, где agent должен читать встроенные HOI4 guidance перед записью файлов.
- Для команд, которым нужны предсказуемые mod-root пути и проверяемая форма сгенерированного вывода.

<h2 align="center">Что получают agents</h2>

<h3 align="center">Prompts</h3>

Встроенные prompts помогают с:

- планированием mod feature
- написанием HOI4 script
- написанием localisation
- работой с GUI, GFX и scripted GUI
- проверкой сгенерированного контента

Текущие имена prompts: `hoi4_mod_planner`, `hoi4_script_writer`, `hoi4_localisation_writer`, `hoi4_gui_assistant`, `hoi4_review`.

<h3 align="center">Resources</h3>

Agents могут читать локальные resources вместо старта с пустого prompt:

- `rhoiscribe://hoi4/latest-update`
- `rhoiscribe://hoi4/knowledge/catalog`
- `rhoiscribe://hoi4/knowledge/<topic_id>`

Knowledge catalog структурирован для agents. Topics содержат category, file types, tags, syntax examples, relationships с другими системами HOI4, validation guidance и source references. Текущее покрытие включает script basics, scopes, triggers, effects, modifiers, variables, MTTH variables, unique identifier checks, arrays, localisation, scripted localisation, scripted triggers/effects, GUI, scripted GUI, focuses, events, detailed on_action scope families, decisions, missions, ideas, characters, history, map files, technology, equipment, units, AI, diplomacy, game rules, defines, bookmarks, audio, game path discovery, debug launch checks, error log triage и common loading errors.

<h3 align="center">Tools</h3>

Agents могут вызывать tools для повторяемой генерации и проверки:

- `generate_localisation_batch`
- `generate_focus_batch`
- `generate_event_batch`
- `generate_decision_batch`
- `search_hoi4_knowledge`
- `scan_unique_identifiers`
- `discover_hoi4_environment`
- `validate_hoi4_debug_run`
- `classify_error_log`
- `validate_hoi4_paths`
- `format_paradox_script`

Generation tools поддерживают dry-run preview. В write mode требуется `output_root`, а запись идет только по путям относительно корня целевого мода.
Knowledge search возвращает matching topic IDs и MCP resource URIs для запросов вроде `mtth variables`, `decision mission blocks` или `on_actions FROM.FROM`.
Identifier scanning пакетно проверяет proposed new IDs по structured HOI4 definitions и сообщает duplicates, existing output files и `replace_path` risks.
Environment discovery сначала ищет установку HOI4 через Steam metadata, затем при необходимости сканирует folders и читает `launcher-settings.json` для document data path, `hoi4.exe` path, `logs/error.log` path и версии игры.
Debug-run validation проверяет document folders `map`, `localisation`, `history`, launcher mod descriptors, текущий playset, dependency descriptors и path рабочего мода перед опциональным запуском `hoi4.exe -gdpr-compliant -debug_mode`.
Error-log classification группирует строки `error.log` по вероятной подсистеме HOI4 и связывает их с changed paths, если agent передал diff или список generated files.

<h2 align="center">Помогите улучшить RHoiScribe</h2>

Синтаксис HOI4 и modding-практики меняются вместе с версиями игры. Если встроенные знания устарели, неполны или содержат ошибку, откройте [Issue](https://github.com/czxieddan/RHoiScribe/issues) и по возможности укажите версию игры, тип файла, ссылку на источник и минимальный воспроизводимый пример.

Pull requests приветствуются для расширения knowledge catalog, улучшения примеров и разработки новых MCP tools для генерации, проверки, сканирования проектов и agent workflows.

<h2 align="center">Быстрый старт</h2>

Скачайте готовый binary из [GitHub Releases](https://github.com/czxieddan/RHoiScribe/releases):

- Windows: `rhoiscribe-windows-x86_64.exe`
- Linux: `rhoiscribe-linux-x86_64`
- macOS: `rhoiscribe-macos-universal`

Если ваш agent умеет читать Skill folder, скачайте Skill package для своей платформы:

- Windows: `rhoiscribe-skill-windows-x86_64.zip`
- Linux: `rhoiscribe-skill-linux-x86_64.zip`
- macOS: `rhoiscribe-skill-macos-universal.zip`

Распакуйте его в постоянную папку. В package находятся `SKILL.md` и подходящий executable, поэтому agent может использовать RHoiScribe напрямую без настройки MCP server.

Поместите скачанный файл в постоянную папку. В Linux и macOS выполните `chmod +x` для скачанного файла, если система просит разрешение на запуск.

Собирайте из исходников только если нужен локальный Cargo build:

```powershell
cargo build --release
```

Source build помещает исполняемый файл в `<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/`.

Выведите path, который нужно указать в поле `command` MCP-клиента:

```powershell
.\rhoiscribe-windows-x86_64.exe --print-command
```

В Linux и macOS используйте тот же option для скачанного файла:

```bash
./rhoiscribe-linux-x86_64 --print-command
./rhoiscribe-macos-universal --print-command
```

Запускайте напрямую только если хотите вручную стартовать stdio MCP server:

```powershell
.\rhoiscribe-windows-x86_64.exe
```

```bash
./rhoiscribe-linux-x86_64
./rhoiscribe-macos-universal
```

Skill packages также можно вызывать напрямую для JSON output:

```powershell
.\rhoiscribe-windows-x86_64.exe --skill list-tools
.\rhoiscribe-windows-x86_64.exe --skill list-resources
.\rhoiscribe-windows-x86_64.exe --skill list-prompts
.\rhoiscribe-windows-x86_64.exe --skill read-resource "rhoiscribe://hoi4/latest-update"
```

```bash
./rhoiscribe-linux-x86_64 --skill call-tool "search_hoi4_knowledge" '{"query":"on_actions ROOT FROM"}'
```

Примеры конфигурации для Codex, Claude Code и generic MCP см. в [client-setup.md](client-setup.md).

<h2 align="center">MCP Surface</h2>

После запуска RHoiScribe клиентом agent может использовать стандартные MCP methods:

- `prompts/list`
- `prompts/get`
- `resources/list`
- `resources/read`
- `tools/list`
- `tools/call`

Пример чтения resource:

```text
rhoiscribe://hoi4/knowledge/scripted_gui.dynamic_lists
```

Пример environment discovery call:

```json
{
  "scan_fallback": true
}
```

Пример debug preflight call:

```json
{
  "game_path": "<HOI4_GAME_PATH>",
  "document_path": "<HOI4_DOCUMENT_PATH>",
  "workspace_mod_path": "<MOD_OUTPUT_ROOT>",
  "launch": false
}
```

Пример error log classification call:

```json
{
  "error_log_path": "<HOI4_DOCUMENT_PATH>/logs/error.log",
  "changed_paths": ["common/national_focus/CHI.txt"],
  "limit": 5
}
```

Пример аргументов `tools/call` для dry-run локализации:

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

Write mode добавляет корень вывода мода:

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

В write mode файл localisation записывается с UTF-8 BOM.
Если мод пользователя уже использует вложенные папки localisation, можно передать `file_stem` вроде `common/autonomy/CHI` или полный mod-relative path вроде `localisation/simp_chinese/common/autonomy/CHI_l_simp_chinese.yml`.

<h2 align="center">Модель вывода</h2>

Generation tools возвращают структурированный план файлов:

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

Пути являются относительными к моду и могут использовать вложенные HOI4-readable папки, если это соответствует workspace пользователя. Небезопасные пути, пути с drive prefix и directory traversal отклоняются до записи.
