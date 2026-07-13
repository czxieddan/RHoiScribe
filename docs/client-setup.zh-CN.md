# MCP 接入指南

[English](client-setup.md) | [Русский](client-setup.ru.md) | [日本語](client-setup.ja.md)

RHoiScribe 是一个通过 stdio 启动的本地 MCP server，适用于 Codex、Claude Code，以及其他能启动本地命令的 MCP 兼容客户端。

接入完成后，如果想了解具体能力和推荐工作流，请看 [详细功能介绍](features.zh-CN.md)。

## 下载

从 [GitHub Releases](https://github.com/czxieddan/RHoiScribe/releases) 下载预构建二进制文件：

- Windows: `rhoiscribe-windows-x86_64.exe`
- Linux: `rhoiscribe-linux-x86_64`
- macOS: `rhoiscribe-macos-universal`

> [!WARNING]
> Skill 包暂时还会保留，但它已经不是新语言支持的推荐入口。语言支持需要一个能持续保温的进程，Skill 的短进程模型不太合适；后续会逐步灰度退场。能用 MCP server 的场景，请优先使用 MCP server。

如果你的 agent 可以读取本地 Skill 文件夹，也可以下载 Skill 包：

- Windows: `rhoiscribe-skill-windows-x86_64.zip`
- Linux: `rhoiscribe-skill-linux-x86_64.zip`
- macOS: `rhoiscribe-skill-macos-universal.zip`

Skill 包中包含 `SKILL.md` 和对应平台的可执行文件。适合不想配置 MCP server、但仍希望 agent 直接调用 RHoiScribe prompts、resources 和 tools 的场景。

请把下载文件放在一个稳定目录。Linux 和 macOS 如果提示没有执行权限，对下载文件运行 `chmod +x`。

只有在需要本地 Cargo 构建时才从源码构建：

```powershell
cargo build --release
```

## 路径

- `<RHOISCRIBE_COMMAND>`：`--print-command` 打印出的绝对路径。
- `<ABSOLUTE_PATH_TO_RHOISCRIBE>`：本仓库或 release 文件夹在用户机器上的绝对路径。
- `<MOD_OUTPUT_ROOT>`：写入工具使用的 HOI4 mod 文件夹绝对路径。

打印命令路径：

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

常见二进制路径：

- 预构建 Windows: `<ABSOLUTE_PATH_TO_RHOISCRIBE>\rhoiscribe-windows-x86_64.exe`
- 预构建 Linux: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/rhoiscribe-linux-x86_64`
- 预构建 macOS: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/rhoiscribe-macos-universal`
- Windows 本地 Cargo 构建: `<ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe`
- Linux 或 macOS 本地 Cargo 构建: `<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe`

## Codex

在 Codex 的 MCP server 配置中加入 RHoiScribe，把 release 二进制文件作为 `command`。

```toml
[mcp_servers.rhoiscribe]
command = "<RHOISCRIBE_COMMAND>"
args = []
```

Windows TOML 字符串通常需要转义反斜杠，也可以使用你的客户端接受的路径写法：

```toml
[mcp_servers.rhoiscribe]
command = "<RHOISCRIBE_COMMAND>"
args = []
```

如果你的 Codex 界面使用其他配置位置，保持相同的 server name、command path 和空 `args` 即可。

## Claude Code

Claude Code 可以通过 MCP 配置或 CLI 注册本地 stdio MCP server。这里同样使用 release 二进制文件作为命令。

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

Windows JSON 字符串需要转义反斜杠：

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

CLI 注册可以使用同一个 command path：

```powershell
claude mcp add rhoiscribe -- <RHOISCRIBE_COMMAND>
```

## 通用 MCP JSON

许多 MCP 客户端都接受带有 `command` 和 `args` 字段的 server map：

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

Windows 客户端通常需要 `.exe` 路径，并在 JSON 中转义反斜杠：

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

## 直接 Skill 命令

直接 Skill 命令会返回 JSON，并暴露与 MCP server 相同的 prompts、resources 和 tools：

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

MCP server 模式会在同一进程内保留已预热的 CWT 语言工作区。直接 `--skill` 命令也能使用同一套能力，但每次命令都是短进程，因此温热状态不会跨命令保留。

## 冒烟测试

把 server 加入客户端后，请让客户端列出 MCP resources，并读取：

```text
rhoiscribe://hoi4/knowledge/catalog
```

然后先调用一个只读工具，例如 `search_hoi4_knowledge`，再使用任何具备写入能力的工具。

项目验证、生成、CWT 语言支持、资产制作和调试工作流，请继续阅读 [详细功能介绍](features.zh-CN.md)。
