<div align="center">

<img src="../resources/RHoiScribe.ico" alt="RHoiScribe" width="128" height="128">

<h1 align="center">RHoiScribe</h1>

面向 Hearts of Iron IV Modding Agents 的本地 MCP 服务器 和 SKILL

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

<h2 align="center">提供什么</h2>

RHoiScribe 为 agents 提供本地 HOI4 知识层、可复用 prompts，以及面向常见 Modding 工作的文件工具。配置 MCP server 后，客户端可以通过标准 MCP list 方法发现完整 prompts、resources 和 tools。

概括来说，它能帮助 agent 完成：

- 大范围编辑前的项目结构与引用关系感知
- 交付前的红/黄/绿加载风险检查
- 编码、格式和媒体规范的快速校对
- 在尊重工作区既有规范的前提下安全修改现有文件
- 通过 Rchadow 辅助准备 HOI4 调试启动
- 通过 RNMDB 在 `.rhoiscribe` 下保存跨 IDE 的 agent 偏好
- 工具调用审计日志与正则筛选导出
- 用户批准后进行实验性 GUI/GFX 程序化资产制作

Skill 包是让兼容 agent 快速获得本地能力的最短路径，不需要先修改 MCP 配置。如果你的 agent 支持 MCP，使用 MCP server 可以获得完整功能和更顺手的体验。

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

Codex、Claude Code 和通用 MCP 配置示例见 [client-setup.md](client-setup.md)。

<h2 align="center">帮助改进 RHoiScribe</h2>

HOI4 语法和 Modding 实践会随着游戏版本持续变化。如果你发现内置知识过时、不完整或存在错误，请通过 [Issue](https://github.com/czxieddan/RHoiScribe/issues) 告诉我们；最好同时提供游戏版本、文件类型、来源引用和可复现的最小示例。

也欢迎通过 Pull Request 扩展知识目录、改进示例，或开发更多面向生成、验证、项目扫描和 agent 工作流的 MCP 工具。

<h2 align="center">署名声明</h2>

任何基于 RHoiScribe 的项目都必须在 README 中加入清晰的署名声明。可直接使用 [rhoiscribe-attribution.md](rhoiscribe-attribution.md) 中的简短模板。

<h2 align="center">鸣谢</h2>

<div align="center">

<a href="https://github.com/czxieddan/Rchadow"><img align="left" src="https://i.imgur.com/omFtYoT.png" alt="Rchadow" width="95"></a>
<a href="https://www.gnu.org/licenses/agpl-3.0.html"><img align="right" src="https://i.imgur.com/9hl34Lt.png" alt="GNU AGPLv3" width="95"></a>

<p align="center">
<h3><strong>Based on Rchadow</strong></h3>
RHoiScribe uses Rchadow, an embeddable Rust playset and launch library for Hearts of Iron IV tools.<br>
Rchadow provides playset storage, HOI4 mod discovery, launcher-compatible load file generation, and launch abstractions.
</p>

<br clear="both">

</div>

<div align="center">
  <h3>贡献者</h3>
  <a href="https://github.com/czxieddan/RHoiScribe/graphs/contributors">
    <img src="https://stg.contrib.rocks/image?repo=czxieddan/RHoiScribe" alt="RHoiScribe contributors">
  </a>
</div>
