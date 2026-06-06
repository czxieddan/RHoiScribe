<div align="center">

# RHoiScribe

面向 Hearts of Iron IV Modding Agents 的本地 MCP 服务器

[English](../README.md) | [Русский](README.ru.md) | [日本語](README.ja.md)

[![GitHub Stars](https://img.shields.io/github/stars/czxieddan/RHoiScribe?style=for-the-badge&label=Stars)](https://github.com/czxieddan/RHoiScribe/stargazers)
[![License](https://img.shields.io/badge/License-AGPL--3.0--or--later-blue?style=for-the-badge)](../LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024-orange?style=for-the-badge)](../Cargo.toml)
[![MCP](https://img.shields.io/badge/MCP-stdio-green?style=for-the-badge)](client-setup.md)

如果 RHoiScribe 对你的 Modding 工作流有帮助，给仓库 Star 可以让更多 HOI4 Mod 作者发现它。

</div>

RHoiScribe 为 Codex、Claude Code 和其他兼容 MCP 的客户端提供本地 HOI4 Modding 参考层，以及生成游戏可读文件的工具。

它的目标很明确：减少 agent 因重复联网搜索、过时假设、不安全路径、缺少本地化编码、以及“看起来像 Paradox 脚本但游戏无法加载”的内容造成的浪费。

## 环境

| 项目 | 内容 |
| --- | --- |
| 服务传输 | 基于 stdio 的 MCP |
| 实现语言 | Rust 2024 |
| 构建工具 | Cargo |
| 主要客户端 | Codex、Claude Code、MCP-compatible clients |
| 运行时联网 | 内置 prompts、resources、tools 不需要联网 |
| Modding 目标 | Hearts of Iron IV 本地 Mod |

## 适合谁

- 希望 AI agents 生成 HOI4 内容时拥有更好本地上下文的 Mod 作者。
- 需要把 prompts、resources、tools 统一接入一个 MCP server 的 agent 工作流。
- 离线或低搜索开发场景，要求 agent 写文件前先读取内置 HOI4 指引。
- 需要生成内容使用可预测 mod-root 路径和可审查输出结构的团队。

## Agents 能得到什么

### Prompts

内置 prompts 覆盖：

- Mod 功能规划
- HOI4 脚本编写
- 本地化编写
- GUI、GFX、scripted GUI 工作
- 生成内容审查

当前 prompt 名称包括 `hoi4_mod_planner`、`hoi4_script_writer`、`hoi4_localisation_writer`、`hoi4_gui_assistant`、`hoi4_review`。

### Resources

Agents 可以读取本地资源，而不是从空白提示开始：

- `rhoiscribe://hoi4/latest-update`
- `rhoiscribe://hoi4/knowledge/catalog`
- `rhoiscribe://hoi4/knowledge/<topic_id>`

知识目录为 agent 使用而结构化。Topic 包含分类、文件类型、标签、语法示例、与其他 HOI4 系统的关系、验证建议和来源引用。当前覆盖脚本基础、scope、trigger、effect、modifier、变量、数组、本地化、scripted localisation、scripted triggers/effects、GUI、scripted GUI、国策、事件、决议、理念、角色、历史、地图文件、科技、装备、单位、AI、外交、游戏规则、defines、书签、音频和常见加载错误。

### Tools

Agents 可以调用工具进行可重复的生成和验证：

- `generate_localisation_batch`
- `generate_focus_batch`
- `generate_event_batch`
- `generate_decision_batch`
- `validate_hoi4_paths`
- `format_paradox_script`

生成工具支持 dry-run 预览。写入模式需要 `output_root`，并且只按目标 Mod 根目录的相对路径写入。

## 快速开始

构建服务器：

```powershell
cargo build --release
```

在你的 MCP 客户端中使用 release binary：

```text
<ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe
```

Linux 和 macOS 使用：

```text
<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe
```

只有当你想手动启动 stdio MCP server 时，才需要直接运行：

```powershell
.\target\release\rhoiscribe.exe
```

Codex、Claude Code 和通用 MCP 配置示例见 [client-setup.md](client-setup.md)。

## MCP Surface

客户端启动 RHoiScribe 后，agent 可以使用标准 MCP 方法：

- `prompts/list`
- `prompts/get`
- `resources/list`
- `resources/read`
- `tools/list`
- `tools/call`

示例 resource read：

```text
rhoiscribe://hoi4/knowledge/scripted_gui.dynamic_lists
```

示例 `tools/call` 本地化 dry-run 参数：

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

写入模式需要增加 Mod 输出根目录：

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

写入模式下生成的本地化文件会使用 UTF-8 BOM。
当用户 Mod 已经使用嵌套本地化目录时，可以使用 `common/autonomy/CHI` 这样的 `file_stem`，也可以使用 `localisation/simp_chinese/common/autonomy/CHI_l_simp_chinese.yml` 这样的完整 Mod 相对路径。

## 输出模型

生成工具返回结构化文件计划：

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

路径是 Mod 相对路径；当它符合用户工作区规范时，可以使用 HOI4 可读取的嵌套目录。不安全路径、带盘符路径和目录穿越会在写入前被拒绝。
