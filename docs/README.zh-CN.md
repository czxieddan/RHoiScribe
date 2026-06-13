<div align="center">

<img src="../resources/RHoiScribe.ico" alt="RHoiScribe" width="128" height="128">

<h1 align="center">RHoiScribe</h1>

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

<h2 align="center">环境</h2>

<table align="center">
  <tr>
    <th align="center">项目</th>
    <th align="center">内容</th>
  </tr>
  <tr>
    <td align="center">服务传输</td>
    <td align="center">基于 stdio 的 MCP</td>
  </tr>
  <tr>
    <td align="center">实现语言</td>
    <td align="center">Rust 2024</td>
  </tr>
  <tr>
    <td align="center">构建工具</td>
    <td align="center">Cargo</td>
  </tr>
  <tr>
    <td align="center">主要客户端</td>
    <td align="center">Codex、Claude Code、MCP-compatible clients</td>
  </tr>
  <tr>
    <td align="center">运行时联网</td>
    <td align="center">内置 prompts、resources、tools 不需要联网</td>
  </tr>
  <tr>
    <td align="center">Modding 目标</td>
    <td align="center">Hearts of Iron IV 本地 Mod</td>
  </tr>
</table>

<h2 align="center">适合谁</h2>

- 希望 AI agents 生成 HOI4 内容时拥有更好本地上下文的 Mod 作者。
- 需要把 prompts、resources、tools 统一接入一个 MCP server 的 agent 工作流。
- 离线或低搜索开发场景，要求 agent 写文件前先读取内置 HOI4 指引。
- 需要生成内容使用可预测 mod-root 路径和可审查输出结构的团队。

<h2 align="center">Agents 能得到什么</h2>

<h3 align="center">Prompts</h3>

内置 prompts 覆盖：

- Mod 功能规划
- HOI4 脚本编写
- 本地化编写
- GUI、GFX、scripted GUI 工作
- 生成内容审查

当前 prompt 名称包括 `hoi4_mod_planner`、`hoi4_script_writer`、`hoi4_localisation_writer`、`hoi4_gui_assistant`、`hoi4_review`。

<h3 align="center">Resources</h3>

Agents 可以读取本地资源，而不是从空白提示开始：

- `rhoiscribe://hoi4/latest-update`
- `rhoiscribe://hoi4/knowledge/catalog`
- `rhoiscribe://hoi4/knowledge/<topic_id>`

知识目录为 agent 使用而结构化。Topic 包含分类、文件类型、标签、语法示例、与其他 HOI4 系统的关系、验证建议和来源引用。当前覆盖脚本基础、scope、trigger、effect、modifier、变量、MTTH 变量、唯一 ID 检查、数组、本地化、scripted localisation、scripted triggers/effects、GUI、scripted GUI、国策、事件、细分 on_action scope 家族、决议、任务、理念、角色、历史、地图文件、科技、装备、单位、AI、外交、游戏规则、defines、书签、音频、游戏路径发现、debug 启动检查、error.log 整理和常见加载错误。

<h3 align="center">Tools</h3>

Agents 可以调用工具进行可重复的生成和验证：

- `generate_localisation_batch`
- `generate_focus_batch`
- `generate_event_batch`
- `generate_decision_batch`
- `search_hoi4_knowledge`
- `scan_unique_identifiers`
- `index_hoi4_project`
- `validate_hoi4_project`
- `repair_hoi4_project`
- `edit_hoi4_script_file`
- `generate_gui_gfx_asset`
- `discover_hoi4_environment`
- `validate_hoi4_debug_run`
- `classify_error_log`
- `validate_hoi4_paths`
- `format_paradox_script`

生成工具支持 dry-run 预览。写入模式需要 `output_root`，并且只按目标 Mod 根目录的相对路径写入。
知识搜索会为 `mtth variables`、`decision mission blocks`、`on_actions FROM.FROM` 这类查询返回匹配 topic ID 和 MCP resource URI。
唯一 ID 扫描会按结构化 HOI4 定义批量检查候选新 ID，并报告重复、已有输出文件和 `replace_path` 风险。
项目索引会为 flag、变量、scripted trigger/effect、国策、事件、GUI 元素、GFX sprite、贴图路径和本地化键建立结构化定义与引用图。
项目验证会返回红/黄/绿检查，覆盖重复定义、括号平衡、缺失贴图或 sprite、缺失本地化键和 `replace_path` 风险。
项目修复支持 dry-run 或直接应用编码与格式修复。它会检查 UTF-8 BOM 规则、脚本格式、`sound/` 文件类型和 `music/` OGG 元数据。需要 ffmpeg 但本机缺失时，RHoiScribe 只返回安装建议，不会在没有用户批准时安装。
脚本编辑可以对现有 HOI4 脚本文件替换或插入命名 block，并提供 dry-run 预览和括号检查。
实验性 GUI/GFX 资产工具可以在本地生成程序化 PNG、`.gfx` 注册文件，以及可选的 `.gui` 文件，不依赖外部生图模型。写入新资产需要 `approved=true`。
环境发现会优先通过 Steam 元数据定位 HOI4 安装目录，再按需扫描文件夹，并从 `launcher-settings.json` 读取文档目录、`hoi4.exe` 路径、`logs/error.log` 路径和游戏版本。
Debug 启动检查会在可选启动 `hoi4.exe -gdpr-compliant -debug_mode` 前检查文档目录下的 `map`、`localisation`、`history` 文件夹、launcher mod 描述文件、当前播放集、依赖描述文件和工作区 mod 路径。
Error log 分类会按可能的 HOI4 子系统整理 `error.log`，并在 agent 提供 diff 或生成文件列表时关联到对应变更路径。

<h2 align="center">帮助改进 RHoiScribe</h2>

HOI4 语法和 Modding 实践会随着游戏版本持续变化。如果你发现内置知识过时、不完整或存在错误，请通过 [Issue](https://github.com/czxieddan/RHoiScribe/issues) 告诉我们；最好同时提供游戏版本、文件类型、来源引用和可复现的最小示例。

也欢迎通过 Pull Request 扩展知识目录、改进示例，或开发更多面向生成、验证、项目扫描和 agent 工作流的 MCP 工具。

<h2 align="center">快速开始</h2>

从 [GitHub Releases](https://github.com/czxieddan/RHoiScribe/releases) 下载预构建二进制文件：

- Windows: `rhoiscribe-windows-x86_64.exe`
- Linux: `rhoiscribe-linux-x86_64`
- macOS: `rhoiscribe-macos-universal`

如果你的 agent 可以读取 Skill 文件夹，也可以下载对应平台的 Skill 包：

- Windows: `rhoiscribe-skill-windows-x86_64.zip`
- Linux: `rhoiscribe-skill-linux-x86_64.zip`
- macOS: `rhoiscribe-skill-macos-universal.zip`

解压到稳定目录即可。压缩包包含 `SKILL.md` 和对应平台的可执行文件；不配置 MCP server 时，agent 也能直接使用 RHoiScribe。

把下载的文件放在一个稳定目录。Linux 和 macOS 如果提示没有执行权限，对下载文件运行 `chmod +x`。

只有当你需要本地 Cargo 构建时才从源码构建：

```powershell
cargo build --release
```

源码构建会把可执行文件放在 `<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/` 下。

打印 MCP 客户端里 `command` 应填写的路径：

```powershell
.\rhoiscribe-windows-x86_64.exe --print-command
```

Linux 和 macOS 对下载文件执行同一个参数：

```bash
./rhoiscribe-linux-x86_64 --print-command
./rhoiscribe-macos-universal --print-command
```

只有当你想手动启动 stdio MCP server 时，才需要直接运行：

```powershell
.\rhoiscribe-windows-x86_64.exe
```

```bash
./rhoiscribe-linux-x86_64
./rhoiscribe-macos-universal
```

Skill 包也可以直接输出 JSON：

```powershell
.\rhoiscribe-windows-x86_64.exe --skill list-tools
.\rhoiscribe-windows-x86_64.exe --skill list-resources
.\rhoiscribe-windows-x86_64.exe --skill list-prompts
.\rhoiscribe-windows-x86_64.exe --skill read-resource "rhoiscribe://hoi4/latest-update"
```

```bash
./rhoiscribe-linux-x86_64 --skill call-tool "search_hoi4_knowledge" '{"query":"on_actions ROOT FROM"}'
```

Codex、Claude Code 和通用 MCP 配置示例见 [client-setup.md](client-setup.md)。

<h2 align="center">MCP Surface</h2>

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

示例环境发现调用：

```json
{
  "scan_fallback": true
}
```

示例 debug 启动前检查：

```json
{
  "game_path": "<HOI4_GAME_PATH>",
  "document_path": "<HOI4_DOCUMENT_PATH>",
  "workspace_mod_path": "<MOD_OUTPUT_ROOT>",
  "launch": false
}
```

示例 error.log 分类：

```json
{
  "error_log_path": "<HOI4_DOCUMENT_PATH>/logs/error.log",
  "changed_paths": ["common/national_focus/CHI.txt"],
  "limit": 5
}
```

示例项目验证调用：

```json
{
  "roots": [
    {
      "path": "<MOD_OUTPUT_ROOT>",
      "role": "mod"
    }
  ],
  "include_game_roots": true
}
```

示例修复 dry-run 调用：

```json
{
  "roots": [
    {
      "path": "<MOD_OUTPUT_ROOT>",
      "role": "mod"
    }
  ],
  "dry_run": true,
  "format_scripts": true,
  "check_media": true
}
```

示例实验性 GUI/GFX 资产 dry-run：

```json
{
  "asset_name": "CHI_command_button",
  "sprite_name": "GFX_CHI_command_button",
  "width": 128,
  "height": 64,
  "style": "button",
  "primary_color": "#214a67",
  "secondary_color": "#d5b261",
  "approved": true,
  "dry_run": true
}
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

<h2 align="center">输出模型</h2>

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
