# RHoiScribe 详细功能介绍

[English](features.md) | [Русский](features.ru.md) | [日本語](features.ja.md)

这份文档介绍 RHoiScribe 在 MCP server 或 Skill 包配置完成后可以做什么。安装和客户端配置请看 [MCP 接入指南](client-setup.zh-CN.md)。

## 运行模型

- Transport: 本地 stdio MCP。
- 运行时联网：不需要。
- Prompts: 通过 `prompts/list` 和 `prompts/get` 获取。
- Resources: 通过 `resources/list` 和 `resources/read` 读取。
- Tools: 通过 `tools/list` 和 `tools/call` 调用。
- MCP 模式会在同一进程中保留已预热的 CWT 语言工作区，适合连续诊断、补全、引用分析和多轮修改。
- 直接 `--skill` 命令使用同一套 prompts、resources 和 tools，但每次命令都是短进程，因此已预热的 CWT 状态不会跨命令保留。

## CWT 语言支持

- CWT resources: `rhoiscribe://hoi4/cwt/catalog` 和 `rhoiscribe://hoi4/cwt/metadata` 会说明当前内置的 HOI4 CWT rules crate、上游 revision、hash、虚拟路径前缀，以及不落盘的运行策略。
- CWT 内存策略：内置 CWT rules 来自编译进二进制的 Cargo git dependency 静态 source table，并在进程内存中读取。
- RHoiScribe 不会解压规则文件，不会创建 CWT cache、lock file，也不会把 CWT 语言状态写入 RNMDB。
- CWT 语言工具会跳过 RHoiScribe 的 tool-call logging，因此 CWT diagnostics 和 workspace language state 不会写入 `.rhoiscribe` 日志存储。
- 工作区预热：在 MCP 会话里尽早用当前 mod root 调用 `open_hoi4_language_workspace`，然后轮询 `get_hoi4_language_status`，直到状态变为 warm。
- 如果 mod root、rules override、vanilla root、ignore globs 或语言配置变化，请重新打开工作区。
- 项目诊断：`validate_hoi4_project` 默认使用 hybrid 模式，也就是 CWT 加 legacy checks。需要旧行为时使用 `validation_mode = "legacy"`；只看 CWT 时使用 `validation_mode = "cwt"`；需要明确两者都跑时使用 `validation_mode = "hybrid"`。
- 单文件诊断：`validate_hoi4_file` 可以验证一个已保存文件，也可以验证未保存文本；如果传入 resident workspace handle，会结合已预热状态。
- 诊断解释：`explain_hoi4_diagnostic` 会给出适合 agent 阅读的含义、可能原因和修复方向。
- 语言智能：`list_hoi4_workspace_symbols`、`find_hoi4_definition`、`find_hoi4_references`、`suggest_hoi4_completion`、`inspect_hoi4_scope`、`inspect_hoi4_type_rule` 可返回位置、补全、scope 语境和适用规则。
- 本地化辅助：`generate_missing_localisation` 返回可审查的 dry-run 本地化候选和生成文件内容。它本身不会写文件；只有在审查通过后，才把返回的 entries 交给 `generate_localisation_batch` 写入。

推荐的 CWT 工作流：

```text
open_hoi4_language_workspace
get_hoi4_language_status
validate_hoi4_project
validate_hoi4_file
explain_hoi4_diagnostic
inspect_hoi4_scope
inspect_hoi4_type_rule
```

## 项目质量工具

- 项目索引：`index_hoi4_project` 返回 mod root 以及可选 game roots 中的结构化 definitions、references 和 files。
- 项目验证：`validate_hoi4_project` 返回红/黄/绿静态检查，覆盖 CWT schema diagnostics、重复 ID、CWT parse diagnostics 不可用时的 brace balance、缺失 GUI/GFX/localisation 引用，以及 `replace_path` 风险。
- 修复检查：`repair_hoi4_project` 可以 dry-run，也可以应用 UTF-8 BOM、Paradox script formatting 和音频检查修复。
- 如果缺少 ffmpeg，dry-run 会返回说明。只有在用户明确同意后，才使用 `dry_run=false` 和 `install_ffmpeg=true` 尝试静默安装。

推荐的项目检查流程：

```text
index_hoi4_project
validate_hoi4_project
repair_hoi4_project with dry_run = true
```

只有在审查返回的变更后，才使用 repair 的 apply mode。

## 生成与编辑

- 写入模式：生成工具需要 `dry_run = false` 和 `output_root = "<MOD_OUTPUT_ROOT>"`。
- 本地化生成：`generate_localisation_batch` 接收明确的 key/value entries。描述文本请作为单独的 `_desc` key/value entry 提供。
- 现有文件编辑：`edit_hoi4_script_file` 可以在已有 HOI4 script 文件中替换或插入具名 block，并提供 dry-run preview 和 brace checks。
- 调用编辑工具时请传入当前 mod 或 workspace 的 `workspace_root`，让目标文件被限制在这棵目录树内。
- focus、event、decision 和 skeleton 批量工具更适合生成新文件骨架；已有文件中的细节逻辑，请用 `edit_hoi4_script_file` 定点修改。

推荐的本地化流程：

```text
generate_missing_localisation
review returned entries
generate_localisation_batch
```

返回的文件路径应留在有效的 `localisation/<language>/` 树下；如果用户的 mod 已经有嵌套目录，也可以遵循现有结构。文件名使用常见的 `_l_<language>.yml` 后缀，编码应为 `utf-8-bom`。

## GUI 与 GFX 资产

- 实验性资产工具：`generate_gui_gfx_asset` 可以在本地生成程序化 PNG、`.gfx` sprite registration，以及可选 `.gui` 文件，不依赖外部图像模型。
- 写入需要 `approved=true`。
- 优先复用工作区已有素材。只有在用户同意创建新的程序化 GUI/GFX 资产后，才设置 `approved=true`。
- 动画 GUI sprite 应遵循现有 sprite-sheet 约定和 `frameAnimatedSpriteType` 知识，不要把生成的静态 sprite 说成动画。

推荐的资产流程：

```text
generate_gui_gfx_asset with dry_run = true
review returned files and metadata
generate_gui_gfx_asset with approved=true only after user approval
```

## 环境与调试

- 环境发现：`discover_hoi4_environment` 可以在本地安装 HOI4 时找到 `<HOI4_GAME_PATH>`、`game_executable_path`、`<HOI4_DOCUMENT_PATH>`、`error_log_path` 和游戏版本。
- 调试预检：`validate_hoi4_debug_run` 检查 launcher descriptors、playset state、文档目录是否干净，并可选启动 `hoi4.exe -gdpr-compliant -debug_mode`。
- Rchadow 调试启动：`launch_hoi4_debug_with_rchadow` 可以准备 debug playset，选择 memory 或 disk mode，并可选通过 Rchadow 启动 HOI4。

推荐的调试流程：

```text
discover_hoi4_environment
validate_hoi4_debug_run with launch = false
launch_hoi4_debug_with_rchadow with launch = false
```

只有当预检结果为 green，且用户确实希望 RHoiScribe 启动游戏时，才设置 `launch = true`。

## 偏好与日志

- Agent preferences: `list_agent_preferences`、`set_agent_preference`、`delete_agent_preference` 会把跨 IDE 习惯保存在 RNMDB-backed `.rhoiscribe` store 中。
- Tool logs: `query_tool_logs` 和 `export_tool_logs` 从同一个 `.rhoiscribe` store 读取最近的非 CWT tool calls，并支持 regex filtering。
- 日志归类：`classify_error_log` 会按可能的 HOI4 subsystem 分组 `error.log` 条目，也可以把条目与变更过的 mod-relative paths 关联起来。

直接日志命令：

```powershell
.\rhoiscribe-windows-x86_64.exe --logs "generate_.*"
.\rhoiscribe-windows-x86_64.exe --export-logs rhoiscribe-tool-logs.json "error|failed"
```

Linux 和 macOS 的下载二进制使用同样的参数。
