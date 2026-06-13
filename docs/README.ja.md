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

<h2 align="center">Agents が得られるもの</h2>

<h3 align="center">Prompts</h3>

内蔵 prompts は次を支援します。

- Mod feature planning
- HOI4 script writing
- localisation writing
- GUI、GFX、scripted GUI work
- generated-content review

現在の prompt 名は `hoi4_mod_planner`、`hoi4_script_writer`、`hoi4_localisation_writer`、`hoi4_gui_assistant`、`hoi4_review` です。

<h3 align="center">Resources</h3>

Agents は空の prompt から始める代わりに、ローカル resources を読めます。

- `rhoiscribe://hoi4/latest-update`
- `rhoiscribe://hoi4/knowledge/catalog`
- `rhoiscribe://hoi4/knowledge/<topic_id>`

Knowledge catalog は agent 向けに構造化されています。Topics には category、file types、tags、syntax examples、他の HOI4 systems との relationships、validation guidance、source references が含まれます。現在の範囲は script basics、scopes、triggers、effects、modifiers、variables、MTTH variables、unique identifier checks、arrays、localisation、scripted localisation、scripted triggers/effects、GUI、scripted GUI、focuses、events、detailed on_action scope families、decisions、missions、ideas、characters、history、map files、technology、equipment、units、AI、diplomacy、game rules、defines、bookmarks、audio、game path discovery、debug launch checks、error log triage、common loading errors です。

<h3 align="center">Tools</h3>

Agents は反復可能な生成と検証のために tools を呼び出せます。

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

Generation tools は dry-run preview をサポートします。write mode では `output_root` が必要で、対象 Mod の root からの相対 path にのみ書き込みます。
Knowledge search は `mtth variables`、`decision mission blocks`、`on_actions FROM.FROM` のような query に対して matching topic IDs と MCP resource URIs を返します。
Identifier scanning は proposed new IDs を structured HOI4 definitions に対して batch check し、duplicates、existing output files、`replace_path` risks を返します。
Project indexing は flags、variables、scripted triggers/effects、focuses、events、GUI elements、GFX sprites、texture paths、localisation keys の definitions、references、files を構造化して返します。
Project validation は duplicate definitions、brace balance、missing textures or sprites、missing localisation keys、`replace_path` risks を red/yellow/green checks として返します。
Project repair は encoding と formatting の fixes を dry-run または apply できます。UTF-8 BOM rules、script formatting、`sound/` file types、`music/` OGG metadata を確認します。ffmpeg が必要で見つからない場合、RHoiScribe は install guidance を返し、user approval なしではインストールしません。
Script editing は existing HOI4 script files に対して named block の replace または insert を行い、dry-run preview と brace checks を返します。
Experimental GUI/GFX asset tool は external image models に依存せず、local procedural PNG assets、`.gfx` sprite registration、optional `.gui` files を生成できます。新しい assets の書き込みには `approved=true` が必要です。
Environment discovery はまず Steam metadata から HOI4 install を探し、必要に応じて folder scan を行い、`launcher-settings.json` から document data path、`hoi4.exe` path、`logs/error.log` path、game version を読み取ります。
Debug-run validation は optional な `hoi4.exe -gdpr-compliant -debug_mode` 起動の前に、document folders の `map`、`localisation`、`history`、launcher mod descriptors、現在の playset、dependency descriptors、workspace mod path を確認します。
Error-log classification は `error.log` を HOI4 subsystem ごとに分類し、agent が diff または generated file list を持っている場合は changed paths に結び付けます。

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

stdio MCP server を手動で起動したい場合だけ直接実行します。

```powershell
.\rhoiscribe-windows-x86_64.exe
```

```bash
./rhoiscribe-linux-x86_64
./rhoiscribe-macos-universal
```

Skill package は JSON output 用に直接呼び出すこともできます。

```powershell
.\rhoiscribe-windows-x86_64.exe --skill list-tools
.\rhoiscribe-windows-x86_64.exe --skill list-resources
.\rhoiscribe-windows-x86_64.exe --skill list-prompts
.\rhoiscribe-windows-x86_64.exe --skill read-resource "rhoiscribe://hoi4/latest-update"
```

```bash
./rhoiscribe-linux-x86_64 --skill call-tool "search_hoi4_knowledge" '{"query":"on_actions ROOT FROM"}'
```

Codex、Claude Code、汎用 MCP 設定例は [client-setup.md](client-setup.md) を参照してください。

<h2 align="center">MCP Surface</h2>

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

Environment discovery call の例:

```json
{
  "scan_fallback": true
}
```

Debug preflight call の例:

```json
{
  "game_path": "<HOI4_GAME_PATH>",
  "document_path": "<HOI4_DOCUMENT_PATH>",
  "workspace_mod_path": "<MOD_OUTPUT_ROOT>",
  "launch": false
}
```

Error log classification call の例:

```json
{
  "error_log_path": "<HOI4_DOCUMENT_PATH>/logs/error.log",
  "changed_paths": ["common/national_focus/CHI.txt"],
  "limit": 5
}
```

Project validation call の例:

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

Repair dry-run call の例:

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

Experimental GUI/GFX asset dry-run の例:

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

<h2 align="center">出力モデル</h2>

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
