<div align="center">

<img src="resources/RHoiScribe.ico" alt="RHoiScribe" width="128" height="128">

<h1 align="center">RHoiScribe</h1>

Local MCP server and SKILL for Hearts of Iron IV modding agents

[简体中文](docs/README.zh-CN.md) | [Русский](docs/README.ru.md) | [日本語](docs/README.ja.md)

[![GitHub Stars](https://img.shields.io/github/stars/czxieddan/RHoiScribe?style=for-the-badge&label=Stars)](https://github.com/czxieddan/RHoiScribe/stargazers)
[![License](https://img.shields.io/badge/License-AGPL--3.0--or--later-blue?style=for-the-badge)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024-orange?style=for-the-badge)](Cargo.toml)
[![MCP](https://img.shields.io/badge/MCP-stdio-green?style=for-the-badge)](docs/client-setup.md)

If RHoiScribe helps your modding workflow, starring the repository helps other HOI4 mod authors find it.

</div>

RHoiScribe gives Codex, Claude Code, and other MCP-compatible clients a local HOI4 modding reference layer plus tools for generating game-readable files.

The goal is simple: reduce wasted agent work caused by repeated web searches, stale assumptions, unsafe file paths, missing localisation encoding, and Paradox script that looks plausible but does not load in game.

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

<h2 align="center">Who It Is For</h2>

- Mod authors who want AI agents to generate HOI4 content with better local context.
- Agent workflows that need prompts, resources, and tools available through one MCP server.
- Offline or low-search development sessions where the agent should read bundled HOI4 guidance before writing files.
- Teams that want generated content to follow predictable mod-root paths and reviewable output shapes.

<h2 align="center">What It Provides</h2>

RHoiScribe gives agents a local HOI4 knowledge layer, reusable prompts, and file-oriented tools for common modding work. MCP clients can discover the exact prompts, resources, and tools through the standard MCP list methods after the server is configured.

At a high level, it helps agents with:

- project structure and reference awareness before broad edits
- red/yellow/green checks for load-risk issues before delivery
- fast encoding, formatting, and media convention checks
- safe edits to existing files while respecting workspace conventions
- Rchadow-assisted HOI4 debug launch preparation
- RNMDB-backed cross-IDE agent preferences under `.rhoiscribe`
- tool-call audit logs with regex-filtered export
- experimental GUI/GFX asset creation when the user approves new procedural art

The bundled Skill package is the quickest way to give compatible agents local access without editing MCP configuration. For complete functionality and a smoother experience, use the MCP server when your agent supports it.

<h2 align="center">Quick Start</h2>

Download a prebuilt binary from [GitHub Releases](https://github.com/czxieddan/RHoiScribe/releases):

- Windows: `rhoiscribe-windows-x86_64.exe`
- Linux: `rhoiscribe-linux-x86_64`
- macOS: `rhoiscribe-macos-universal`

For agents that can read a Skill folder, download the matching Skill package:

- Windows: `rhoiscribe-skill-windows-x86_64.zip`
- Linux: `rhoiscribe-skill-linux-x86_64.zip`
- macOS: `rhoiscribe-skill-macos-universal.zip`

Unzip it into a stable folder. The package contains `SKILL.md` and the matching executable, so an agent can use RHoiScribe directly even when you do not want to configure an MCP server.

Keep the downloaded file in a stable folder. On Linux and macOS, run `chmod +x` on the downloaded file if the system asks for executable permission.

Build from source only when you want a local Cargo build:

```powershell
cargo build --release
```

Source builds place the executable under `<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/`.

Print the command path to use in your MCP client:

```powershell
.\rhoiscribe-windows-x86_64.exe --print-command
```

Linux and macOS users can run the same option on their downloaded file:

```bash
./rhoiscribe-linux-x86_64 --print-command
./rhoiscribe-macos-universal --print-command
```

For Codex, Claude Code, and generic MCP configuration examples, see [docs/client-setup.md](docs/client-setup.md).

<h2 align="center">Help Improve RHoiScribe</h2>

HOI4 syntax and modding practice change over time. If you find bundled knowledge that is outdated, incomplete, or wrong, please open an [Issue](https://github.com/czxieddan/RHoiScribe/issues) with the game version, file type, source reference, and a minimal example when possible.

Pull requests are welcome for expanding the knowledge catalog, improving examples, or building more MCP tools for generation, validation, project scanning, and other agent workflows.

<h2 align="center">Attribution</h2>

Projects based on RHoiScribe must include a clear README attribution section. Use the short statement template in [docs/rhoiscribe-attribution.md](docs/rhoiscribe-attribution.md).

<h2 align="center">Acknowledgements</h2>

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
  <h3>Contributors</h3>
  <a href="https://github.com/czxieddan/RHoiScribe/graphs/contributors">
    <img src="https://stg.contrib.rocks/image?repo=czxieddan/RHoiScribe" alt="RHoiScribe contributors">
  </a>
</div>
