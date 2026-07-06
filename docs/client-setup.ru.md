# Настройка MCP-клиента

[English](client-setup.md) | [简体中文](client-setup.zh-CN.md) | [日本語](client-setup.ja.md)

RHoiScribe — локальный MCP server, запускаемый через stdio. Он подходит для Codex, Claude Code и других MCP-compatible clients, которые умеют запускать локальную команду.

После подключения подробное описание возможностей и рабочих порядков см. в [руководстве по возможностям](features.ru.md).

## Загрузка

Скачайте готовый binary из [GitHub Releases](https://github.com/czxieddan/RHoiScribe/releases):

- Windows: `rhoiscribe-windows-x86_64.exe`
- Linux: `rhoiscribe-linux-x86_64`
- macOS: `rhoiscribe-macos-universal`

Для agents, которые умеют читать локальную Skill folder, доступны Skill packages:

- Windows: `rhoiscribe-skill-windows-x86_64.zip`
- Linux: `rhoiscribe-skill-linux-x86_64.zip`
- macOS: `rhoiscribe-skill-macos-universal.zip`

В Skill package находятся `SKILL.md` и подходящий executable. Это удобно, когда нужен прямой доступ agent к RHoiScribe prompts, resources и tools без отдельной записи MCP server.

Положите скачанный файл в постоянную папку. В Linux и macOS выполните `chmod +x` для скачанного файла, если система попросит разрешение на запуск.

Собирайте из исходников только если нужен локальный Cargo build:

```powershell
cargo build --release
```

## Пути

В документации и примерах, которые попадают в репозиторий, используйте placeholders. Заменяйте их на реальные пути только в своей private client configuration:

- `<RHOISCRIBE_COMMAND>`: абсолютный путь, который выводит `--print-command`.
- `<ABSOLUTE_PATH_TO_RHOISCRIBE>`: абсолютный путь к этому repository или release folder на машине пользователя.
- `<MOD_OUTPUT_ROOT>`: абсолютный путь к папке HOI4 mod, куда tool будет писать файлы.

Вывести command path:

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

Обычные binary paths:

- Prebuilt Windows: `<ABSOLUTE_PATH_TO_RHOISCRIBE>\rhoiscribe-windows-x86_64.exe`
- Prebuilt Linux: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/rhoiscribe-linux-x86_64`
- Prebuilt macOS: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/rhoiscribe-macos-universal`
- Локальный Cargo build на Windows: `<ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe`
- Локальный Cargo build на Linux или macOS: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe`

## Codex

Добавьте RHoiScribe в конфигурацию Codex MCP server, указав release binary как `command`.

```toml
[mcp_servers.rhoiscribe]
command = "<RHOISCRIBE_COMMAND>"
args = []
```

В Windows TOML strings обычно нужно экранировать обратные слеши либо использовать формат path, который принимает ваш client:

```toml
[mcp_servers.rhoiscribe]
command = "<RHOISCRIBE_COMMAND>"
args = []
```

Если ваша поверхность Codex хранит настройки в другом месте, сохраняйте то же server name, command path и пустой `args`.

## Claude Code

Claude Code может зарегистрировать local stdio MCP server через MCP configuration или CLI. Используйте release binary как command.

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

В Windows JSON strings экранируйте обратные слеши:

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

CLI-регистрация использует тот же command path:

```powershell
claude mcp add rhoiscribe -- <RHOISCRIBE_COMMAND>
```

## Универсальный MCP JSON

Многие MCP clients принимают server map с полями `command` и `args`:

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

Windows clients обычно требуют path к `.exe` и экранированные обратные слеши в JSON:

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

## Прямые Skill-команды

Прямые Skill-команды возвращают JSON и показывают те же prompts, resources и tools, что и MCP server:

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

MCP server mode держит CWT language workspaces прогретыми в process memory между tool calls. Прямые `--skill` commands открывают те же возможности, но каждый command — короткоживущий process, поэтому прогретое CWT state не сохраняется между вызовами.

## Основы выполнения

- Transport: stdio.
- Runtime network: не требуется.
- Prompts: доступны через `prompts/list` и `prompts/get`.
- Resources: доступны через `resources/list` и `resources/read`.
- Tools: доступны через `tools/list` и `tools/call`.
- Описание возможностей и рекомендуемые workflows: [руководство по возможностям](features.ru.md).

## Быстрая проверка

После добавления server в client попросите client вывести MCP resources и прочитать:

```text
rhoiscribe://hoi4/knowledge/catalog
```

Затем вызовите read-only tool, например `search_hoi4_knowledge`, прежде чем использовать tools, которые могут писать файлы.

Для project validation, генерации, CWT language support, assets и debug workflows используйте [руководство по возможностям](features.ru.md).
