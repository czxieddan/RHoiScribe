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

<h2 align="center">Что он предоставляет</h2>

RHoiScribe дает agents локальный слой знаний HOI4, reusable prompts и file-oriented tools для типичных modding задач. После настройки MCP server клиент может обнаружить точные prompts, resources и tools через стандартные MCP list methods.

На верхнем уровне он помогает agents с:

- пониманием структуры проекта и связей перед крупными правками
- red/yellow/green проверками рисков загрузки перед передачей результата
- быстрой проверкой encoding, formatting и media conventions
- безопасным редактированием существующих файлов с учетом workspace conventions
- experimental GUI/GFX asset creation после user approval на новую procedural art

Skill package для нужной платформы предоставляет те же возможности для agents, которым удобнее читать локальную Skill folder без отдельной записи в MCP server.

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

Примеры конфигурации для Codex, Claude Code и generic MCP см. в [client-setup.md](client-setup.md).
