<div align="center">

<img src="../resources/RHoiScribe.ico" alt="RHoiScribe" width="128" height="128">

<h1 align="center">RHoiScribe</h1>

Hearts of Iron IV Modding Agents 向けのローカル MCP サーバー

[English](../README.md) | [简体中文](README.zh-CN.md) | [Русский](README.ru.md)

[![GitHub Stars](https://img.shields.io/github/stars/czxieddan/RHoiScribe?style=for-the-badge&label=Stars)](https://github.com/czxieddan/RHoiScribe/stargazers)
[![License](https://img.shields.io/badge/License-AGPL--3.0--or--later-blue?style=for-the-badge)](../LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024-orange?style=for-the-badge)](../Cargo.toml)
[![MCP](https://img.shields.io/badge/MCP-stdio-green?style=for-the-badge)](client-setup.md)

RHoiScribe があなたの modding workflow に役立つなら、Star は他の HOI4 mod authors がこの project を見つける助けになります。

</div>

RHoiScribe は Codex、Claude Code、その他の MCP-compatible clients に、ローカルの HOI4 Modding 参照レイヤーと、ゲームが読めるファイルを生成する tools を提供します。

目的は明確です。繰り返しの Web 検索、古い前提、安全でないパス、localisation のエンコーディング漏れ、そして「Paradox script らしく見えるがゲームでは読み込めない」内容による agent の無駄を減らします。

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

<h2 align="center">対象ユーザー</h2>

- AI agents により良いローカル文脈で HOI4 コンテンツを生成させたい Mod 作者。
- prompts、resources、tools を 1 つの MCP server にまとめたい agent workflows。
- オフラインまたは低検索の開発セッションで、agent がファイルを書く前に内蔵 HOI4 guidance を読む必要があるケース。
- 生成物に予測可能な mod-root path とレビューしやすい出力形式を求めるチーム。

<h2 align="center">提供内容</h2>

RHoiScribe は agents にローカル HOI4 knowledge layer、reusable prompts、そして一般的な modding 作業向けの file-oriented tools を提供します。MCP server を設定した後、クライアントは標準 MCP list methods で正確な prompts、resources、tools を発見できます。

ローカル Skill folder を使う方が適した agents には、同じ機能を提供する platform 別 Skill package も用意されています。

<h2 align="center">RHoiScribe の改善に参加</h2>

HOI4 の構文と Modding の実践は、ゲームのバージョンに合わせて変化します。内蔵知識が古い、不完全、または誤っている場合は、[Issue](https://github.com/czxieddan/RHoiScribe/issues) を作成してください。可能であれば、ゲームバージョン、ファイル種別、参照元、最小の再現例を添えてください。

Knowledge catalog の拡張、examples の改善、生成、検証、project scanning、agent workflows 向けの MCP tools 開発に関する Pull Request も歓迎します。

<h2 align="center">クイックスタート</h2>

[GitHub Releases](https://github.com/czxieddan/RHoiScribe/releases) から prebuilt binary をダウンロードします。

- Windows: `rhoiscribe-windows-x86_64.exe`
- Linux: `rhoiscribe-linux-x86_64`
- macOS: `rhoiscribe-macos-universal`

agent が Skill folder を読める場合は、対応する Skill package も使えます。

- Windows: `rhoiscribe-skill-windows-x86_64.zip`
- Linux: `rhoiscribe-skill-linux-x86_64.zip`
- macOS: `rhoiscribe-skill-macos-universal.zip`

安定したフォルダーに展開してください。Package には `SKILL.md` と対象 platform の executable が入っているため、MCP server を設定しなくても agent が RHoiScribe を直接使えます。

ダウンロードしたファイルは、移動しない安定したフォルダーに置いてください。Linux と macOS で実行権限を求められた場合は、ダウンロードしたファイルに `chmod +x` を実行します。

ローカル Cargo build が必要な場合だけ source からビルドします。

```powershell
cargo build --release
```

Source build では executable が `<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/` に置かれます。

MCP クライアントの `command` に入れる path を表示します。

```powershell
.\rhoiscribe-windows-x86_64.exe --print-command
```

Linux と macOS では、ダウンロードしたファイルで同じ option を実行します。

```bash
./rhoiscribe-linux-x86_64 --print-command
./rhoiscribe-macos-universal --print-command
```

Codex、Claude Code、汎用 MCP 設定例は [client-setup.md](client-setup.md) を参照してください。
