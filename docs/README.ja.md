<div align="center">

# RHoiScribe

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

## Environment

| Area | Value |
| --- | --- |
| Server transport | MCP over stdio |
| Implementation | Rust 2024 |
| Build tool | Cargo |
| Primary clients | Codex, Claude Code, MCP-compatible clients |
| Runtime network | Not required for bundled prompts, resources, and tools |
| Modding target | Hearts of Iron IV local mods |

## 対象ユーザー

- AI agents により良いローカル文脈で HOI4 コンテンツを生成させたい Mod 作者。
- prompts、resources、tools を 1 つの MCP server にまとめたい agent workflows。
- オフラインまたは低検索の開発セッションで、agent がファイルを書く前に内蔵 HOI4 guidance を読む必要があるケース。
- 生成物に予測可能な mod-root path とレビューしやすい出力形式を求めるチーム。

## Agents が得られるもの

### Prompts

内蔵 prompts は次を支援します。

- Mod feature planning
- HOI4 script writing
- localisation writing
- GUI、GFX、scripted GUI work
- generated-content review

現在の prompt 名は `hoi4_mod_planner`、`hoi4_script_writer`、`hoi4_localisation_writer`、`hoi4_gui_assistant`、`hoi4_review` です。

### Resources

Agents は空の prompt から始める代わりに、ローカル resources を読めます。

- `rhoiscribe://hoi4/latest-update`
- `rhoiscribe://hoi4/knowledge/catalog`
- `rhoiscribe://hoi4/knowledge/<topic_id>`

Knowledge catalog は agent 向けに構造化されています。Topics には category、file types、tags、syntax examples、他の HOI4 systems との relationships、validation guidance、source references が含まれます。現在の範囲は script basics、scopes、triggers、effects、modifiers、variables、arrays、localisation、scripted localisation、scripted triggers/effects、GUI、scripted GUI、focuses、events、decisions、ideas、characters、history、map files、technology、equipment、units、AI、diplomacy、game rules、defines、bookmarks、audio、common loading errors です。

### Tools

Agents は反復可能な生成と検証のために tools を呼び出せます。

- `generate_localisation_batch`
- `generate_focus_batch`
- `generate_event_batch`
- `generate_decision_batch`
- `validate_hoi4_paths`
- `format_paradox_script`

Generation tools は dry-run preview をサポートします。write mode では `output_root` が必要で、対象 Mod の root からの相対 path にのみ書き込みます。

## クイックスタート

サーバーをビルドします。

```powershell
cargo build --release
```

MCP クライアントで release binary を指定します。

```text
<ABSOLUTE_PATH_TO_RHOISCRIBE>\target\release\rhoiscribe.exe
```

Linux と macOS では次を使います。

```text
<ABSOLUTE_PATH_TO_RHOISCRIBE>/target/release/rhoiscribe
```

stdio MCP server を手動で起動したい場合だけ直接実行します。

```powershell
.\target\release\rhoiscribe.exe
```

Codex、Claude Code、汎用 MCP 設定例は [client-setup.md](client-setup.md) を参照してください。

## MCP Surface

クライアントが RHoiScribe を起動した後、agent は標準 MCP methods を使えます。

- `prompts/list`
- `prompts/get`
- `resources/list`
- `resources/read`
- `tools/list`
- `tools/call`

Resource read の例:

```text
rhoiscribe://hoi4/knowledge/scripted_gui.dynamic_lists
```

localisation dry-run 用の `tools/call` 引数例:

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

Write mode では Mod output root を追加します。

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

write mode で生成される localisation file は UTF-8 BOM で書き込まれます。
ユーザーの Mod がネストされた localisation folders を使っている場合は、`common/autonomy/CHI` のような `file_stem`、または `localisation/simp_chinese/common/autonomy/CHI_l_simp_chinese.yml` のような完全な mod-relative path を使えます。

## 出力モデル

Generation tools は構造化された file plan を返します。

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

Paths は Mod 相対です。ユーザーの workspace に合う場合は、HOI4-readable なネストされた folders を使えます。安全でない path、drive prefix 付き path、directory traversal は書き込み前に拒否されます。
