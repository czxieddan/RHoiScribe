//------------------------------------------------------------------------------------
// mod.rs -- Part of RHoiScribe
//
// Copyright (C) 2026 CzXieDdan. All rights reserved.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// https://github.com/czxieddan/RHoiScribe
//------------------------------------------------------------------------------------

mod environment;
mod error_log;
mod gui_gfx_asset;
mod hoi4_keys;
mod mod_skeleton;
mod paradox_lexer;
mod preferences;
mod project_effective_files;
mod project_files;
mod project_index;
mod project_repair;
mod project_validation;
mod rchadow_debug;
mod rnmdb_store;
mod script_edit;
mod tool_logs;
mod unique_scan;

use std::{borrow::Cow, error::Error, fmt, fs, path::Path};

use rmcp::model::{CallToolResult, JsonObject, Tool, ToolAnnotations};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::resources::{KNOWLEDGE_TOPIC_URI_PREFIX, KnowledgeCatalog};

pub use environment::{
    DiscoverHoi4EnvironmentRequest, Hoi4DebugRunRequest, Hoi4DebugRunResult, Hoi4EnvironmentResult,
    Hoi4QualityCheck,
};
pub use error_log::{
    ClassifyErrorLogRequest, ErrorLogCategory, ErrorLogClassificationResult, ErrorLogEntry,
};
pub use gui_gfx_asset::{
    GenerateGuiGfxAssetRequest, GenerateGuiGfxAssetResult, GeneratedGuiGfxAssetFile,
};
pub use mod_skeleton::Hoi4ModSkeletonRequest;
pub use preferences::{
    AgentPreferenceItem, AgentPreferenceMutationResult, AgentPreferencesResult,
    DeleteAgentPreferenceRequest, ListAgentPreferencesRequest, SetAgentPreferenceRequest,
};
pub use project_index::{IndexedFile, ProjectIndexItem, ProjectIndexRequest, ProjectIndexResult};
pub use project_repair::{
    FfmpegStatus, RepairChange, RepairCheck, RepairHoi4ProjectRequest, RepairHoi4ProjectResult,
};
pub use project_validation::{
    ProjectValidationCheck, ProjectValidationRequest, ProjectValidationResult,
};
pub use rchadow_debug::{RchadowDebugLaunchRequest, RchadowDebugLaunchResult};
pub use script_edit::{EditHoi4ScriptFileRequest, EditHoi4ScriptFileResult, ScriptEditOperation};
pub use tool_logs::{
    ToolLogEntry, ToolLogExportRequest, ToolLogExportResult, ToolLogQueryRequest,
    ToolLogQueryResult,
};
pub use unique_scan::{
    CandidateScanResult, IdentifierCandidate, IdentifierMatch, PathRisk, ScanRoot,
    UniqueIdentifierScanRequest, UniqueIdentifierScanResult,
};

pub const MODULE_PURPOSE: &str = "batch generation and validation tools";

const TOOL_SPECS: &[ToolSpec] = &[
    ToolSpec {
        name: "generate_localisation_batch",
        title: "Generate localisation batch",
        description: "Generate a HOI4 localisation yml file with UTF-8 BOM. entries is the JSON array of key/value pairs, not a mixed content object; write descriptions as separate _desc entries. file_stem may include nested subdirectories or a mod-relative localisation/<language>/ path; filenames are normalized to the usual _l_<language>.yml suffix. When dry_run=false, provide output_root for the current mod or requested output root.",
        required: &["language", "file_stem", "entries", "dry_run"],
        handler: call_generate_localisation_batch,
    },
    ToolSpec {
        name: "generate_focus_batch",
        title: "Generate focus batch",
        description: "Generate a HOI4 focus tree format skeleton. Use this generator first when creating a new focus file so the base braces and required fields are game-readable, then use edit_hoi4_script_file to replace or insert detailed trigger, effect, icon, layout, AI, and localisation-driven content. When dry_run=false, provide output_root for the current mod or requested output root.",
        required: &["country_tag", "tree_id", "focuses", "dry_run"],
        handler: call_generate_focus_batch,
    },
    ToolSpec {
        name: "generate_event_batch",
        title: "Generate event batch",
        description: "Generate a HOI4 country/news event format skeleton. events is the JSON array of event objects; options is the JSON array of event choices and renders as HOI4 option = {} blocks, not a literal options block. Use this generator first for a new event file, then use edit_hoi4_script_file to complete narrative triggers, options, hidden effects, follow-up events, pictures, and localisation. When dry_run=false, provide output_root for the current mod or requested output root.",
        required: &["namespace", "events", "dry_run"],
        handler: call_generate_event_batch,
    },
    ToolSpec {
        name: "generate_decision_batch",
        title: "Generate decision batch",
        description: "Generate a HOI4 decision format skeleton. decisions is the JSON array of decision objects; category-level visible/allowed and icon belong to the category, while per-decision fields belong to each decision. When creating a new decision category, also define or complete common/decisions/categories/*.txt metadata for the category. Use this generator first, then use edit_hoi4_script_file to complete missions, dynamic logic, triggers, effects, target rules, and AI. When dry_run=false, provide output_root for the current mod or requested output root.",
        required: &["category_id", "decisions", "dry_run"],
        handler: call_generate_decision_batch,
    },
    ToolSpec {
        name: "search_hoi4_knowledge",
        title: "Search HOI4 knowledge",
        description: "Search bundled HOI4 modding knowledge topics and return matching MCP resource URIs.",
        required: &["query"],
        handler: call_search_hoi4_knowledge,
    },
    ToolSpec {
        name: "scan_unique_identifiers",
        title: "Scan unique identifiers",
        description: "Concurrently scan mod and game roots for structured HOI4 identifiers before creating new IDs, and report duplicate, overwrite, and replace_path risks.",
        required: &["roots", "candidates"],
        handler: call_scan_unique_identifiers,
    },
    ToolSpec {
        name: "discover_hoi4_environment",
        title: "Discover HOI4 environment",
        description: "Find the HOI4 game directory through Steam metadata first, then optional folder scanning, and read launcher-settings.json for the document data path and game version.",
        required: &[],
        handler: call_discover_hoi4_environment,
    },
    ToolSpec {
        name: "validate_hoi4_debug_run",
        title: "Validate HOI4 debug run",
        description: "Check the game path, document data folders, launcher mod descriptors, active playset, dependency descriptors, and optionally launch hoi4.exe with debug arguments.",
        required: &["game_path", "document_path", "workspace_mod_path"],
        handler: call_validate_hoi4_debug_run,
    },
    ToolSpec {
        name: "launch_hoi4_debug_with_rchadow",
        title: "Launch HOI4 debug with Rchadow",
        description: "Use Rchadow to prepare a HOI4 debug playset and optionally launch hoi4.exe with debug arguments. The tool chooses memory mode for temporary launch-only debugging and disk mode for durable project sessions unless mode is provided.",
        required: &["game_path", "document_path", "workspace_mod_path"],
        handler: call_launch_hoi4_debug_with_rchadow,
    },
    ToolSpec {
        name: "classify_error_log",
        title: "Classify HOI4 error log",
        description: "Group error.log lines by likely HOI4 subsystem and link messages back to changed files when paths are provided.",
        required: &["error_log_path"],
        handler: call_classify_error_log,
    },
    ToolSpec {
        name: "list_agent_preferences",
        title: "List agent preferences",
        description: "Read persistent RHoiScribe user or project habits from the RNMDB-backed .rhoiscribe store so agents can keep cross-IDE preferences such as localisation folder style.",
        required: &[],
        handler: call_list_agent_preferences,
    },
    ToolSpec {
        name: "set_agent_preference",
        title: "Set agent preference",
        description: "Write one persistent RHoiScribe preference into the RNMDB-backed .rhoiscribe store. Use stable ASCII keys such as localisation.folder_style.",
        required: &["key", "value"],
        handler: call_set_agent_preference,
    },
    ToolSpec {
        name: "delete_agent_preference",
        title: "Delete agent preference",
        description: "Remove one persistent RHoiScribe preference from the RNMDB-backed .rhoiscribe store.",
        required: &["key"],
        handler: call_delete_agent_preference,
    },
    ToolSpec {
        name: "query_tool_logs",
        title: "Query tool logs",
        description: "Read recent RHoiScribe tool-call logs from the same RNMDB-backed .rhoiscribe store as preferences. Supports optional regex filtering over each log entry's JSON text.",
        required: &[],
        handler: call_query_tool_logs,
    },
    ToolSpec {
        name: "export_tool_logs",
        title: "Export tool logs",
        description: "Export RHoiScribe tool-call logs as JSON from the same RNMDB-backed .rhoiscribe store as preferences. Supports optional regex filtering over each log entry's JSON text.",
        required: &["output_path"],
        handler: call_export_tool_logs,
    },
    ToolSpec {
        name: "index_hoi4_project",
        title: "Index HOI4 project",
        description: "Concurrently index HOI4 mod and game roots into structured definitions and references for flags, variables, scripted triggers/effects, GUI, GFX, and localisation.",
        required: &["roots"],
        handler: call_index_hoi4_project,
    },
    ToolSpec {
        name: "validate_hoi4_project",
        title: "Validate HOI4 project",
        description: "Run red/yellow/green static checks over indexed HOI4 roots for duplicate definitions, brace balance, unclosed blocks, missing GFX textures or sprites, localisation references, structural definitions, and replace_path risks.",
        required: &["roots"],
        handler: call_validate_hoi4_project,
    },
    ToolSpec {
        name: "repair_hoi4_project",
        title: "Repair HOI4 project",
        description: "Dry-run or apply fast HOI4 project repairs for UTF-8 BOM rules, Paradox script formatting, sound/music media checks, and ffmpeg approval-gated guidance.",
        required: &["roots", "dry_run"],
        handler: call_repair_hoi4_project,
    },
    ToolSpec {
        name: "edit_hoi4_script_file",
        title: "Edit HOI4 script file",
        description: "Modify an existing HOI4 txt/gui/gfx/lua script file inside workspace_root by replacing or inserting a named block, or update a localisation yml key/value entry, with dry-run preview, brace-closure checks for replacement content and final files, and encoding preservation.",
        required: &["path", "operation", "dry_run"],
        handler: call_edit_hoi4_script_file,
    },
    ToolSpec {
        name: "generate_gui_gfx_asset",
        title: "Generate GUI/GFX asset",
        description: "Experimentally generate a local procedural HOI4 PNG asset, .gfx sprite registration, and optional .gui files without external image models; writing requires approved=true.",
        required: &["asset_name", "width", "height", "approved", "dry_run"],
        handler: call_generate_gui_gfx_asset,
    },
    ToolSpec {
        name: "setup_hoi4_mod_skeleton",
        title: "Setup HOI4 mod skeleton",
        description: "Create an early HOI4 mod skeleton for sparse new projects with descriptor.mod, starter common/events/localisation files, decision category metadata, and core directories. Use this before specialized generators when the workspace lacks common, events, localisation, or other loadable folders. When dry_run=false, provide output_root for the current mod workspace or requested output root.",
        required: &["mod_name", "dry_run"],
        handler: call_setup_hoi4_mod_skeleton,
    },
    ToolSpec {
        name: "validate_hoi4_paths",
        title: "Validate HOI4 paths",
        description: "Validate generated paths against safe HOI4 mod folder conventions.",
        required: &["paths"],
        handler: call_validate_hoi4_paths,
    },
    ToolSpec {
        name: "format_paradox_script",
        title: "Format Paradox script",
        description: "Apply basic readable indentation to Paradox-style key/value script.",
        required: &["script"],
        handler: call_format_paradox_script,
    },
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalisationEntry {
    #[serde(alias = "id")]
    pub key: String,
    #[serde(alias = "title")]
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FocusEntry {
    pub id: String,
    #[serde(alias = "sprite")]
    pub icon: Option<String>,
    #[serde(alias = "position_x", alias = "offset_x")]
    pub x: Option<i32>,
    #[serde(alias = "position_y", alias = "offset_y")]
    pub y: Option<i32>,
    #[serde(default, alias = "pos")]
    pub position: Option<LayoutPosition>,
    pub cost: Option<i32>,
    #[serde(default)]
    pub prerequisite: Vec<String>,
    #[serde(default)]
    pub mutually_exclusive: Vec<String>,
    pub available: Option<String>,
    pub bypass: Option<String>,
    pub cancel_if_invalid: Option<bool>,
    pub continue_if_invalid: Option<bool>,
    pub available_if_capitulated: Option<bool>,
    pub will_lead_to_war_with: Option<String>,
    pub select_effect: Option<String>,
    pub complete_tooltip: Option<String>,
    #[serde(alias = "completion_effect", alias = "effect", alias = "effects")]
    pub completion_reward: Option<String>,
    pub ai_will_do: Option<String>,
    #[serde(default)]
    pub extra_assignments: Vec<ScriptAssignment>,
    #[serde(default)]
    pub extra_blocks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EventEntry {
    #[serde(default)]
    pub id: Option<String>,
    pub event_type: Option<String>,
    pub title: Option<String>,
    pub desc: Option<String>,
    pub picture: Option<String>,
    pub major: Option<bool>,
    pub is_triggered_only: Option<bool>,
    pub fire_only_once: Option<bool>,
    pub trigger: Option<String>,
    pub mean_time_to_happen: Option<String>,
    pub immediate: Option<String>,
    #[serde(default)]
    pub options: Vec<EventOptionEntry>,
    #[serde(default)]
    pub extra_assignments: Vec<ScriptAssignment>,
    #[serde(default)]
    pub extra_blocks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DecisionEntry {
    pub id: String,
    #[serde(alias = "decision_icon", alias = "sprite")]
    pub icon: Option<String>,
    pub cost: Option<i32>,
    pub fire_only_once: Option<bool>,
    pub days_remove: Option<i32>,
    pub days_mission_timeout: Option<i32>,
    pub visible: Option<String>,
    pub available: Option<String>,
    pub target_trigger: Option<String>,
    pub cancel_trigger: Option<String>,
    pub remove_trigger: Option<String>,
    #[serde(alias = "completion_effect", alias = "effect", alias = "effects")]
    pub complete_effect: Option<String>,
    pub timeout_effect: Option<String>,
    pub remove_effect: Option<String>,
    pub ai_will_do: Option<String>,
    #[serde(default)]
    pub extra_assignments: Vec<ScriptAssignment>,
    #[serde(default)]
    pub extra_blocks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LayoutPosition {
    pub x: Option<i32>,
    pub y: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EventOptionEntry {
    pub name: String,
    pub trigger: Option<String>,
    pub ai_chance: Option<String>,
    pub effects: Option<String>,
    pub hidden_effect: Option<String>,
    #[serde(default)]
    pub extra_assignments: Vec<ScriptAssignment>,
    #[serde(default)]
    pub extra_blocks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ScriptAssignment {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalisationBatchRequest {
    pub language: String,
    pub file_stem: String,
    pub key_prefix: Option<String>,
    pub entries: Vec<LocalisationEntry>,
    pub dry_run: bool,
    pub output_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FocusBatchRequest {
    pub country_tag: String,
    pub tree_id: String,
    pub focuses: Vec<FocusEntry>,
    pub dry_run: bool,
    pub output_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventBatchRequest {
    pub namespace: String,
    pub events: Vec<EventEntry>,
    pub dry_run: bool,
    pub output_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecisionBatchRequest {
    pub category_id: String,
    pub icon: Option<String>,
    pub visible: Option<String>,
    pub allowed: Option<String>,
    pub decisions: Vec<DecisionEntry>,
    pub dry_run: bool,
    pub output_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchHoi4KnowledgeRequest {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidateHoi4PathsRequest {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormatParadoxScriptRequest {
    pub script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
    pub encoding: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionResult {
    pub dry_run: bool,
    pub files: Vec<GeneratedFile>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvalidPath {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathValidationResult {
    pub valid_paths: Vec<String>,
    pub invalid_paths: Vec<InvalidPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormatParadoxScriptResult {
    pub formatted: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnowledgeSearchMatch {
    pub id: String,
    pub uri: String,
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnowledgeSearchResult {
    pub query: String,
    pub matches: Vec<KnowledgeSearchMatch>,
}

#[derive(Debug, Clone, Copy)]
pub struct ToolCatalog {
    tools: &'static [ToolSpec],
}

#[derive(Debug, Clone, Copy)]
struct ToolSpec {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    required: &'static [&'static str],
    handler: ToolHandler,
}

type ToolHandler = fn(JsonObject) -> Result<CallToolResult, ToolError>;

#[derive(Debug)]
pub enum ToolError {
    UnknownTool(String),
    InvalidArguments(serde_json::Error),
    InvalidRequest(String),
    WriteFailed(std::io::Error),
}

pub struct ToolEngine;

impl ToolCatalog {
    pub fn builtin() -> Self {
        Self { tools: TOOL_SPECS }
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.tools.iter().map(|tool| tool.name).collect()
    }

    pub fn to_mcp_tools(&self) -> Vec<Tool> {
        self.tools.iter().map(ToolSpec::as_mcp_tool).collect()
    }

    pub fn call(&self, name: &str, arguments: JsonObject) -> Result<CallToolResult, ToolError> {
        let arguments_for_log = Value::Object(arguments.clone());
        let result = self.call_without_logging(name, arguments);
        self.record_tool_log(name, arguments_for_log, &result);
        result
    }

    fn call_without_logging(
        &self,
        name: &str,
        arguments: JsonObject,
    ) -> Result<CallToolResult, ToolError> {
        let tool = self
            .tools
            .iter()
            .find(|tool| tool.name == name)
            .ok_or_else(|| ToolError::UnknownTool(name.to_string()))?;
        (tool.handler)(arguments)
    }

    fn record_tool_log(
        &self,
        name: &str,
        arguments: Value,
        result: &Result<CallToolResult, ToolError>,
    ) {
        let store_path = arguments
            .as_object()
            .and_then(tool_logs::tool_log_store_path_from_arguments);
        let (success, result, error) = tool_log_outcome(result);
        let _ = tool_logs::record_tool_call(
            store_path.as_deref(),
            tool_logs::ToolLogAppend {
                tool_name: name.to_string(),
                arguments,
                success,
                result,
                error,
            },
        );
    }
}

impl ToolSpec {
    fn as_mcp_tool(&self) -> Tool {
        Tool::new(
            Cow::Borrowed(self.name),
            Cow::Borrowed(self.description),
            input_schema(self.name, self.required),
        )
        .with_title(self.title)
        .with_annotations(
            ToolAnnotations::with_title(self.title)
                .open_world(false)
                .destructive(false),
        )
    }
}

impl ToolEngine {
    pub fn generate_localisation_batch(
        request: LocalisationBatchRequest,
    ) -> Result<ToolExecutionResult, ToolError> {
        let language = localisation_language_key(&request.language);
        let language_dir = language_directory(&language);
        let path = localisation_path(&language_dir, &request.file_stem, &language);
        let mut content = format!("{}:\n", language);

        for entry in &request.entries {
            let key = localised_key(&request.key_prefix, &entry.key);
            content.push_str(&format!(" {}:0 \"{}\"\n", key, entry.value));
        }

        finish_generation(
            request.dry_run,
            request.output_root.as_deref(),
            vec![GeneratedFile {
                path,
                content,
                encoding: Some("utf-8-bom".to_string()),
                summary: "HOI4 localisation file".to_string(),
            }],
        )
    }

    pub fn generate_focus_batch(
        request: FocusBatchRequest,
    ) -> Result<ToolExecutionResult, ToolError> {
        let mut content = format!(
            "focus_tree = {{\n\tid = {}\n\tcountry = {{ factor = 0 modifier = {{ add = 10 tag = {} }} }}\n",
            request.tree_id, request.country_tag
        );

        for (index, focus) in request.focuses.iter().enumerate() {
            content.push_str(&render_focus_entry(focus, index));
        }

        content.push_str("}\n");

        finish_generation(
            request.dry_run,
            request.output_root.as_deref(),
            vec![GeneratedFile {
                path: format!("common/national_focus/{}.txt", request.tree_id),
                content,
                encoding: None,
                summary: "HOI4 national focus tree file".to_string(),
            }],
        )
    }

    pub fn generate_event_batch(
        request: EventBatchRequest,
    ) -> Result<ToolExecutionResult, ToolError> {
        let mut content = format!("namespace = {}\n\n", request.namespace);

        for (index, event) in request.events.iter().enumerate() {
            content.push_str(&render_event_entry(&request.namespace, event, index));
        }

        finish_generation(
            request.dry_run,
            request.output_root.as_deref(),
            vec![GeneratedFile {
                path: format!("events/{}.txt", request.namespace),
                content,
                encoding: None,
                summary: "HOI4 country event file".to_string(),
            }],
        )
    }

    pub fn generate_decision_batch(
        request: DecisionBatchRequest,
    ) -> Result<ToolExecutionResult, ToolError> {
        let mut content = format!("{} = {{\n", request.category_id);
        push_assignment(
            &mut content,
            1,
            "icon",
            request.icon.as_deref().unwrap_or("generic_decisions"),
        );
        push_optional_block(&mut content, 1, "visible", request.visible.as_deref());
        push_optional_block(&mut content, 1, "allowed", request.allowed.as_deref());
        content.push('\n');

        for decision in &request.decisions {
            content.push_str(&render_decision_entry(decision));
        }

        content.push_str("}\n");

        finish_generation(
            request.dry_run,
            request.output_root.as_deref(),
            vec![GeneratedFile {
                path: format!("common/decisions/{}.txt", request.category_id),
                content,
                encoding: None,
                summary: "HOI4 decision category file".to_string(),
            }],
        )
    }

    pub fn search_hoi4_knowledge(
        request: SearchHoi4KnowledgeRequest,
    ) -> Result<KnowledgeSearchResult, ToolError> {
        let catalog = KnowledgeCatalog::load_embedded()
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?;
        let limit = request.limit.unwrap_or(8).clamp(1, 20);
        let matches = catalog
            .search(&request.query)
            .into_iter()
            .take(limit)
            .map(|topic| KnowledgeSearchMatch {
                id: topic.id.clone(),
                uri: format!("{}{}", KNOWLEDGE_TOPIC_URI_PREFIX, topic.id),
                title: topic.title.clone(),
                category: topic.category.clone(),
                tags: topic.tags.clone(),
                summary: topic.body.clone(),
            })
            .collect();

        Ok(KnowledgeSearchResult {
            query: request.query,
            matches,
        })
    }

    pub fn scan_unique_identifiers(
        request: UniqueIdentifierScanRequest,
    ) -> Result<UniqueIdentifierScanResult, ToolError> {
        unique_scan::scan_unique_identifiers(request).map_err(ToolError::InvalidRequest)
    }

    pub fn discover_hoi4_environment(
        request: DiscoverHoi4EnvironmentRequest,
    ) -> Result<Hoi4EnvironmentResult, ToolError> {
        environment::discover_hoi4_environment(request).map_err(ToolError::InvalidRequest)
    }

    pub fn validate_hoi4_debug_run(request: Hoi4DebugRunRequest) -> Hoi4DebugRunResult {
        environment::validate_hoi4_debug_run(request)
    }

    pub fn launch_hoi4_debug_with_rchadow(
        request: RchadowDebugLaunchRequest,
    ) -> Result<RchadowDebugLaunchResult, ToolError> {
        rchadow_debug::launch_hoi4_debug_with_rchadow(request).map_err(ToolError::InvalidRequest)
    }

    pub fn classify_error_log(
        request: ClassifyErrorLogRequest,
    ) -> Result<ErrorLogClassificationResult, ToolError> {
        error_log::classify_error_log(request).map_err(ToolError::InvalidRequest)
    }

    pub fn list_agent_preferences(
        request: ListAgentPreferencesRequest,
    ) -> Result<AgentPreferencesResult, ToolError> {
        preferences::list_agent_preferences(request).map_err(ToolError::InvalidRequest)
    }

    pub fn set_agent_preference(
        request: SetAgentPreferenceRequest,
    ) -> Result<AgentPreferenceMutationResult, ToolError> {
        preferences::set_agent_preference(request).map_err(ToolError::InvalidRequest)
    }

    pub fn delete_agent_preference(
        request: DeleteAgentPreferenceRequest,
    ) -> Result<AgentPreferenceMutationResult, ToolError> {
        preferences::delete_agent_preference(request).map_err(ToolError::InvalidRequest)
    }

    pub fn query_tool_logs(request: ToolLogQueryRequest) -> Result<ToolLogQueryResult, ToolError> {
        tool_logs::query_tool_logs(request).map_err(ToolError::InvalidRequest)
    }

    pub fn export_tool_logs(
        request: ToolLogExportRequest,
    ) -> Result<ToolLogExportResult, ToolError> {
        tool_logs::export_tool_logs(request).map_err(ToolError::InvalidRequest)
    }

    pub fn index_hoi4_project(
        request: ProjectIndexRequest,
    ) -> Result<ProjectIndexResult, ToolError> {
        project_index::index_hoi4_project(request).map_err(ToolError::InvalidRequest)
    }

    pub fn validate_hoi4_project(
        request: ProjectValidationRequest,
    ) -> Result<ProjectValidationResult, ToolError> {
        project_validation::validate_hoi4_project(request).map_err(ToolError::InvalidRequest)
    }

    pub fn repair_hoi4_project(
        request: RepairHoi4ProjectRequest,
    ) -> Result<RepairHoi4ProjectResult, ToolError> {
        project_repair::repair_hoi4_project(request).map_err(ToolError::InvalidRequest)
    }

    pub fn edit_hoi4_script_file(
        request: EditHoi4ScriptFileRequest,
    ) -> Result<EditHoi4ScriptFileResult, ToolError> {
        script_edit::edit_hoi4_script_file(request).map_err(ToolError::InvalidRequest)
    }

    pub fn generate_gui_gfx_asset(
        request: GenerateGuiGfxAssetRequest,
    ) -> Result<GenerateGuiGfxAssetResult, ToolError> {
        gui_gfx_asset::generate_gui_gfx_asset(request).map_err(ToolError::InvalidRequest)
    }

    pub fn setup_hoi4_mod_skeleton(
        request: Hoi4ModSkeletonRequest,
    ) -> Result<ToolExecutionResult, ToolError> {
        mod_skeleton::setup_hoi4_mod_skeleton(request)
    }

    pub fn validate_hoi4_paths(request: ValidateHoi4PathsRequest) -> PathValidationResult {
        let mut valid_paths = Vec::new();
        let mut invalid_paths = Vec::new();

        for path in request.paths {
            if let Some(reason) = invalid_path_reason(&path) {
                invalid_paths.push(InvalidPath { path, reason });
            } else {
                valid_paths.push(path);
            }
        }

        PathValidationResult {
            valid_paths,
            invalid_paths,
        }
    }

    pub fn format_paradox_script(request: FormatParadoxScriptRequest) -> FormatParadoxScriptResult {
        FormatParadoxScriptResult {
            formatted: format_paradox_script(&request.script),
        }
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::UnknownTool(name) => write!(formatter, "unknown tool `{}`", name),
            ToolError::InvalidArguments(error) => write!(formatter, "invalid arguments: {}", error),
            ToolError::InvalidRequest(message) => write!(formatter, "invalid request: {}", message),
            ToolError::WriteFailed(error) => write!(formatter, "write failed: {}", error),
        }
    }
}

impl Error for ToolError {}

impl From<std::io::Error> for ToolError {
    fn from(error: std::io::Error) -> Self {
        ToolError::WriteFailed(error)
    }
}

fn parse_arguments<T>(arguments: JsonObject) -> Result<T, ToolError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(Value::Object(arguments)).map_err(ToolError::InvalidArguments)
}

fn structured_result<T: Serialize>(result: T) -> CallToolResult {
    CallToolResult::structured(json!(result))
}

fn tool_log_outcome(
    result: &Result<CallToolResult, ToolError>,
) -> (bool, Option<Value>, Option<String>) {
    match result {
        Ok(result) => (true, serde_json::to_value(result).ok(), None),
        Err(error) => (false, None, Some(error.to_string())),
    }
}

fn call_generate_localisation_batch(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<LocalisationBatchRequest>(arguments)?;
    Ok(structured_result(ToolEngine::generate_localisation_batch(
        request,
    )?))
}

fn call_generate_focus_batch(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<FocusBatchRequest>(arguments)?;
    Ok(structured_result(ToolEngine::generate_focus_batch(
        request,
    )?))
}

fn call_generate_event_batch(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<EventBatchRequest>(arguments)?;
    Ok(structured_result(ToolEngine::generate_event_batch(
        request,
    )?))
}

fn call_generate_decision_batch(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<DecisionBatchRequest>(arguments)?;
    Ok(structured_result(ToolEngine::generate_decision_batch(
        request,
    )?))
}

fn call_search_hoi4_knowledge(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<SearchHoi4KnowledgeRequest>(arguments)?;
    Ok(structured_result(ToolEngine::search_hoi4_knowledge(
        request,
    )?))
}

fn call_scan_unique_identifiers(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<UniqueIdentifierScanRequest>(arguments)?;
    Ok(structured_result(ToolEngine::scan_unique_identifiers(
        request,
    )?))
}

fn call_discover_hoi4_environment(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<DiscoverHoi4EnvironmentRequest>(arguments)?;
    Ok(structured_result(ToolEngine::discover_hoi4_environment(
        request,
    )?))
}

fn call_validate_hoi4_debug_run(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<Hoi4DebugRunRequest>(arguments)?;
    Ok(structured_result(ToolEngine::validate_hoi4_debug_run(
        request,
    )))
}

fn call_launch_hoi4_debug_with_rchadow(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<RchadowDebugLaunchRequest>(arguments)?;
    Ok(structured_result(
        ToolEngine::launch_hoi4_debug_with_rchadow(request)?,
    ))
}

fn call_classify_error_log(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<ClassifyErrorLogRequest>(arguments)?;
    Ok(structured_result(ToolEngine::classify_error_log(request)?))
}

fn call_list_agent_preferences(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<ListAgentPreferencesRequest>(arguments)?;
    Ok(structured_result(ToolEngine::list_agent_preferences(
        request,
    )?))
}

fn call_set_agent_preference(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<SetAgentPreferenceRequest>(arguments)?;
    Ok(structured_result(ToolEngine::set_agent_preference(
        request,
    )?))
}

fn call_delete_agent_preference(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<DeleteAgentPreferenceRequest>(arguments)?;
    Ok(structured_result(ToolEngine::delete_agent_preference(
        request,
    )?))
}

fn call_query_tool_logs(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<ToolLogQueryRequest>(arguments)?;
    Ok(structured_result(ToolEngine::query_tool_logs(request)?))
}

fn call_export_tool_logs(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<ToolLogExportRequest>(arguments)?;
    Ok(structured_result(ToolEngine::export_tool_logs(request)?))
}

fn call_index_hoi4_project(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<ProjectIndexRequest>(arguments)?;
    Ok(structured_result(ToolEngine::index_hoi4_project(request)?))
}

fn call_validate_hoi4_project(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<ProjectValidationRequest>(arguments)?;
    Ok(structured_result(ToolEngine::validate_hoi4_project(
        request,
    )?))
}

fn call_repair_hoi4_project(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<RepairHoi4ProjectRequest>(arguments)?;
    Ok(structured_result(ToolEngine::repair_hoi4_project(request)?))
}

fn call_edit_hoi4_script_file(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<EditHoi4ScriptFileRequest>(arguments)?;
    Ok(structured_result(ToolEngine::edit_hoi4_script_file(
        request,
    )?))
}

fn call_generate_gui_gfx_asset(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<GenerateGuiGfxAssetRequest>(arguments)?;
    Ok(structured_result(ToolEngine::generate_gui_gfx_asset(
        request,
    )?))
}

fn call_setup_hoi4_mod_skeleton(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<Hoi4ModSkeletonRequest>(arguments)?;
    Ok(structured_result(ToolEngine::setup_hoi4_mod_skeleton(
        request,
    )?))
}

fn call_validate_hoi4_paths(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<ValidateHoi4PathsRequest>(arguments)?;
    Ok(structured_result(ToolEngine::validate_hoi4_paths(request)))
}

fn call_format_paradox_script(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<FormatParadoxScriptRequest>(arguments)?;
    Ok(structured_result(ToolEngine::format_paradox_script(
        request,
    )))
}

fn input_schema(tool_name: &str, required: &[&str]) -> JsonObject {
    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert(
        "required".to_string(),
        Value::Array(
            required
                .iter()
                .map(|name| Value::String((*name).to_string()))
                .collect(),
        ),
    );
    schema.insert("additionalProperties".to_string(), Value::Bool(true));
    let properties = tool_properties(tool_name);
    if !properties.is_empty() {
        schema.insert("properties".to_string(), Value::Object(properties));
    }
    schema
}

fn tool_properties(tool_name: &str) -> Map<String, Value> {
    match tool_name {
        "generate_focus_batch" => focus_batch_properties(),
        "generate_decision_batch" => decision_batch_properties(),
        "query_tool_logs" => query_tool_logs_properties(),
        "export_tool_logs" => export_tool_logs_properties(),
        _ => Map::new(),
    }
}

fn focus_batch_properties() -> Map<String, Value> {
    Map::from_iter([
        text_property(
            "country_tag",
            "Country TAG used by the focus_tree country selector.",
        ),
        text_property("tree_id", "Focus tree id and output filename stem."),
        array_property(
            "focuses",
            "Focus objects. Each object supports id, icon, x, y, position:{x,y}, cost, prerequisite, mutually_exclusive, available, bypass, will_lead_to_war_with, select_effect, complete_tooltip, completion_reward, effect/effects as completion_reward aliases, ai_will_do, extra_assignments, and extra_blocks.",
        ),
        bool_property(
            "dry_run",
            "true returns generated files without writing them.",
        ),
        text_property(
            "output_root",
            "Required when dry_run=false; use the current mod workspace root or the user-requested output root.",
        ),
    ])
}

fn decision_batch_properties() -> Map<String, Value> {
    Map::from_iter([
        text_property(
            "category_id",
            "Decision category block id and output filename stem.",
        ),
        text_property(
            "icon",
            "Category icon. Per-decision icons belong inside decisions[].",
        ),
        text_property("visible", "Optional category-level visible block body."),
        text_property("allowed", "Optional category-level allowed block body."),
        array_property(
            "decisions",
            "Decision objects. Each object supports id, icon, decision_icon as an icon alias, cost, fire_only_once, days_remove, days_mission_timeout, visible, available, target_trigger, cancel_trigger, remove_trigger, complete_effect, effect/effects/completion_effect as complete_effect aliases, timeout_effect, remove_effect, ai_will_do, extra_assignments, and extra_blocks.",
        ),
        bool_property(
            "dry_run",
            "true returns generated files without writing them.",
        ),
        text_property(
            "output_root",
            "Required when dry_run=false; use the current mod workspace root or the user-requested output root.",
        ),
    ])
}

fn query_tool_logs_properties() -> Map<String, Value> {
    Map::from_iter([
        text_property(
            "store_path",
            "Optional RNMDB store path. Omit to use the shared .rhoiscribe preferences and logs database.",
        ),
        text_property(
            "pattern",
            "Optional Rust regex matched against each complete log entry serialized as JSON.",
        ),
        integer_property(
            "limit",
            "Maximum entries to return, clamped to the latest 32767 retained entries.",
        ),
    ])
}

fn export_tool_logs_properties() -> Map<String, Value> {
    Map::from_iter([
        text_property(
            "store_path",
            "Optional RNMDB store path. Omit to use the shared .rhoiscribe preferences and logs database.",
        ),
        text_property("output_path", "JSON file path to write the exported logs."),
        text_property(
            "pattern",
            "Optional Rust regex matched against each complete log entry serialized as JSON.",
        ),
        integer_property(
            "limit",
            "Maximum entries to export, clamped to the latest 32767 retained entries.",
        ),
    ])
}

fn text_property(name: &str, description: &str) -> (String, Value) {
    described_property(name, "string", description)
}

fn integer_property(name: &str, description: &str) -> (String, Value) {
    described_property(name, "integer", description)
}

fn array_property(name: &str, description: &str) -> (String, Value) {
    described_property(name, "array", description)
}

fn bool_property(name: &str, description: &str) -> (String, Value) {
    described_property(name, "boolean", description)
}

fn described_property(name: &str, property_type: &str, description: &str) -> (String, Value) {
    (
        name.to_string(),
        json!({
            "type": property_type,
            "description": description
        }),
    )
}

fn finish_generation(
    dry_run: bool,
    output_root: Option<&str>,
    files: Vec<GeneratedFile>,
) -> Result<ToolExecutionResult, ToolError> {
    if !dry_run {
        let root = output_root.ok_or_else(|| {
            ToolError::InvalidRequest("output_root is required when dry_run is false".to_string())
        })?;
        write_generated_files(root, &files)?;
    }

    Ok(ToolExecutionResult {
        dry_run,
        files,
        messages: vec![if dry_run {
            "dry-run only; no files were written".to_string()
        } else {
            "files written under output_root".to_string()
        }],
    })
}

fn write_generated_files(output_root: &str, files: &[GeneratedFile]) -> Result<(), ToolError> {
    for file in files {
        if let Some(reason) = invalid_path_reason(&file.path) {
            return Err(ToolError::InvalidRequest(format!(
                "refusing to write {}: {}",
                file.path, reason
            )));
        }

        let full_path = Path::new(output_root).join(&file.path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if file.encoding.as_deref() == Some("utf-8-bom") {
            let mut bytes = vec![0xEF, 0xBB, 0xBF];
            bytes.extend_from_slice(file.content.as_bytes());
            fs::write(full_path, bytes)?;
        } else {
            fs::write(full_path, file.content.as_bytes())?;
        }
    }

    Ok(())
}

fn invalid_path_reason(path: &str) -> Option<String> {
    if path.trim().is_empty() {
        return Some("path is empty".to_string());
    }

    let normalized = path.replace('\\', "/");

    if normalized.starts_with('/') || normalized.contains("../") || normalized.starts_with("../") {
        return Some("path must stay inside the mod root".to_string());
    }

    if normalized.contains(':') {
        return Some("path must be relative and must not contain a drive prefix".to_string());
    }

    if normalized == "descriptor.mod" {
        return None;
    }

    let allowed = [
        "common/",
        "events/",
        "gfx/",
        "history/",
        "interface/",
        "localisation/",
    ];

    if !allowed.iter().any(|prefix| normalized.starts_with(prefix)) {
        return Some("path is not in a supported HOI4 mod folder".to_string());
    }

    None
}

fn language_directory(language: &str) -> String {
    language.strip_prefix("l_").unwrap_or(language).to_string()
}

fn localisation_language_key(language: &str) -> String {
    if language.starts_with("l_") {
        language.to_string()
    } else {
        format!("l_{}", language)
    }
}

fn localisation_path(language_dir: &str, file_stem: &str, language: &str) -> String {
    let normalized_stem = file_stem
        .replace('\\', "/")
        .trim_matches('/')
        .trim_end_matches(".yml")
        .to_string();

    let localized_stem = with_language_suffix(&normalized_stem, language);

    if localized_stem.starts_with("localisation/") {
        format!("{}.yml", localized_stem)
    } else {
        format!("localisation/{}/{}.yml", language_dir, localized_stem)
    }
}

fn with_language_suffix(stem: &str, language: &str) -> String {
    if stem.ends_with(&format!("_{}", language)) {
        stem.to_string()
    } else {
        format!("{}_{}", stem, language)
    }
}

fn localised_key(prefix: &Option<String>, id: &str) -> String {
    match prefix {
        Some(prefix) if !prefix.is_empty() => format!("{}_{}", prefix, id),
        _ => id.to_string(),
    }
}

fn render_focus_entry(focus: &FocusEntry, index: usize) -> String {
    let mut content = "\tfocus = {\n".to_string();
    push_assignment(&mut content, 2, "id", &focus.id);
    push_assignment(
        &mut content,
        2,
        "icon",
        focus
            .icon
            .as_deref()
            .unwrap_or(&format!("GFX_focus_{}", focus.id)),
    );
    push_assignment(&mut content, 2, "x", &focus_x(focus, index).to_string());
    push_assignment(&mut content, 2, "y", &focus_y(focus).to_string());
    push_assignment(
        &mut content,
        2,
        "cost",
        &focus.cost.unwrap_or(10).to_string(),
    );
    push_focus_links(&mut content, "prerequisite", &focus.prerequisite);
    push_focus_links(
        &mut content,
        "mutually_exclusive",
        &focus.mutually_exclusive,
    );
    push_optional_bool(
        &mut content,
        2,
        "cancel_if_invalid",
        focus.cancel_if_invalid,
    );
    push_optional_bool(
        &mut content,
        2,
        "continue_if_invalid",
        focus.continue_if_invalid,
    );
    push_optional_bool(
        &mut content,
        2,
        "available_if_capitulated",
        focus.available_if_capitulated,
    );
    push_optional_assignment(
        &mut content,
        2,
        "will_lead_to_war_with",
        focus.will_lead_to_war_with.as_deref(),
    );
    push_optional_block(&mut content, 2, "available", focus.available.as_deref());
    push_optional_block(&mut content, 2, "bypass", focus.bypass.as_deref());
    push_optional_block(
        &mut content,
        2,
        "select_effect",
        focus.select_effect.as_deref(),
    );
    push_optional_block(
        &mut content,
        2,
        "complete_tooltip",
        focus.complete_tooltip.as_deref(),
    );
    push_optional_block(
        &mut content,
        2,
        "completion_reward",
        focus
            .completion_reward
            .as_deref()
            .or(Some("add_political_power = 50")),
    );
    push_optional_block(&mut content, 2, "ai_will_do", focus.ai_will_do.as_deref());
    push_script_assignments(&mut content, 2, &focus.extra_assignments);
    push_extra_blocks(&mut content, 2, &focus.extra_blocks);
    content.push_str("\t}\n");
    content
}

fn focus_x(focus: &FocusEntry, index: usize) -> i32 {
    focus
        .x
        .or_else(|| focus.position.as_ref().and_then(|position| position.x))
        .unwrap_or((index * 2) as i32)
}

fn focus_y(focus: &FocusEntry) -> i32 {
    focus
        .y
        .or_else(|| focus.position.as_ref().and_then(|position| position.y))
        .unwrap_or(0)
}

fn render_event_entry(namespace: &str, event: &EventEntry, index: usize) -> String {
    let id = event
        .id
        .clone()
        .unwrap_or_else(|| format!("{}.{}", namespace, index + 1));
    let event_type = event.event_type.as_deref().unwrap_or("country_event");
    let mut content = format!("{} = {{\n", event_type);
    push_assignment(&mut content, 1, "id", &id);
    push_assignment(
        &mut content,
        1,
        "title",
        event.title.as_deref().unwrap_or(&format!("{}.t", id)),
    );
    push_assignment(
        &mut content,
        1,
        "desc",
        event.desc.as_deref().unwrap_or(&format!("{}.d", id)),
    );
    push_optional_assignment(&mut content, 1, "picture", event.picture.as_deref());
    push_optional_bool(&mut content, 1, "major", event.major);
    push_optional_bool(
        &mut content,
        1,
        "is_triggered_only",
        event.is_triggered_only.or(Some(true)),
    );
    push_optional_bool(&mut content, 1, "fire_only_once", event.fire_only_once);
    push_optional_block(&mut content, 1, "trigger", event.trigger.as_deref());
    push_optional_block(
        &mut content,
        1,
        "mean_time_to_happen",
        event.mean_time_to_happen.as_deref(),
    );
    push_optional_block(&mut content, 1, "immediate", event.immediate.as_deref());
    if event.options.is_empty() {
        content.push_str(&format!("\toption = {{\n\t\tname = {}.a\n\t}}\n", id));
    } else {
        for option in &event.options {
            content.push_str(&render_event_option(option));
        }
    }
    push_script_assignments(&mut content, 1, &event.extra_assignments);
    push_extra_blocks(&mut content, 1, &event.extra_blocks);
    content.push_str("}\n\n");
    content
}

fn render_event_option(option: &EventOptionEntry) -> String {
    let mut content = "\toption = {\n".to_string();
    push_assignment(&mut content, 2, "name", &option.name);
    push_optional_block(&mut content, 2, "trigger", option.trigger.as_deref());
    push_optional_block(&mut content, 2, "ai_chance", option.ai_chance.as_deref());
    push_raw_block_body(&mut content, 2, option.effects.as_deref());
    push_optional_block(
        &mut content,
        2,
        "hidden_effect",
        option.hidden_effect.as_deref(),
    );
    push_script_assignments(&mut content, 2, &option.extra_assignments);
    push_extra_blocks(&mut content, 2, &option.extra_blocks);
    content.push_str("\t}\n");
    content
}

fn render_decision_entry(decision: &DecisionEntry) -> String {
    let mut content = format!("\t{} = {{\n", decision.id);
    push_assignment(
        &mut content,
        2,
        "icon",
        decision.icon.as_deref().unwrap_or("generic_decision"),
    );
    push_assignment(
        &mut content,
        2,
        "cost",
        &decision.cost.unwrap_or(25).to_string(),
    );
    push_optional_bool(&mut content, 2, "fire_only_once", decision.fire_only_once);
    push_optional_assignment(
        &mut content,
        2,
        "days_remove",
        decision
            .days_remove
            .map(|value| value.to_string())
            .as_deref(),
    );
    push_optional_assignment(
        &mut content,
        2,
        "days_mission_timeout",
        decision
            .days_mission_timeout
            .map(|value| value.to_string())
            .as_deref(),
    );
    push_optional_block(&mut content, 2, "visible", decision.visible.as_deref());
    push_optional_block(&mut content, 2, "available", decision.available.as_deref());
    push_optional_block(
        &mut content,
        2,
        "target_trigger",
        decision.target_trigger.as_deref(),
    );
    push_optional_block(
        &mut content,
        2,
        "cancel_trigger",
        decision.cancel_trigger.as_deref(),
    );
    push_optional_block(
        &mut content,
        2,
        "remove_trigger",
        decision.remove_trigger.as_deref(),
    );
    push_optional_block(
        &mut content,
        2,
        "complete_effect",
        decision
            .complete_effect
            .as_deref()
            .or(Some("add_political_power = -25")),
    );
    push_optional_block(
        &mut content,
        2,
        "timeout_effect",
        decision.timeout_effect.as_deref(),
    );
    push_optional_block(
        &mut content,
        2,
        "remove_effect",
        decision.remove_effect.as_deref(),
    );
    push_optional_block(
        &mut content,
        2,
        "ai_will_do",
        decision.ai_will_do.as_deref(),
    );
    push_script_assignments(&mut content, 2, &decision.extra_assignments);
    push_extra_blocks(&mut content, 2, &decision.extra_blocks);
    content.push_str("\t}\n");
    content
}

fn push_focus_links(content: &mut String, key: &str, focus_ids: &[String]) {
    for focus_id in focus_ids {
        push_named_block(content, 2, key, &[format!("focus = {}", focus_id)]);
    }
}

fn push_script_assignments(content: &mut String, indent: usize, assignments: &[ScriptAssignment]) {
    for assignment in assignments {
        push_assignment(content, indent, &assignment.key, &assignment.value);
    }
}

fn push_extra_blocks(content: &mut String, indent: usize, blocks: &[String]) {
    for block in blocks {
        push_raw_block_body(content, indent, Some(block));
    }
}

fn push_optional_bool(content: &mut String, indent: usize, key: &str, value: Option<bool>) {
    if let Some(value) = value {
        push_assignment(content, indent, key, if value { "yes" } else { "no" });
    }
}

fn push_optional_assignment(content: &mut String, indent: usize, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        push_assignment(content, indent, key, value);
    }
}

fn push_assignment(content: &mut String, indent: usize, key: &str, value: &str) {
    content.push_str(&format!("{}{} = {}\n", "\t".repeat(indent), key, value));
}

fn push_optional_block(content: &mut String, indent: usize, key: &str, body: Option<&str>) {
    if let Some(body) = body {
        push_named_block(content, indent, key, &body_lines(body));
    }
}

fn push_named_block(content: &mut String, indent: usize, key: &str, lines: &[String]) {
    content.push_str(&format!("{}{} = {{\n", "\t".repeat(indent), key));
    push_indented_lines(content, indent + 1, lines);
    content.push_str(&format!("{}}}\n", "\t".repeat(indent)));
}

fn push_raw_block_body(content: &mut String, indent: usize, body: Option<&str>) {
    if let Some(body) = body {
        push_indented_lines(content, indent, &body_lines(body));
    }
}

fn push_indented_lines(content: &mut String, indent: usize, lines: &[String]) {
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        content.push_str(&format!("{}{}\n", "\t".repeat(indent), line.trim()));
    }
}

fn body_lines(body: &str) -> Vec<String> {
    let formatted = format_paradox_script(body);
    formatted
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect()
}

fn format_paradox_script(script: &str) -> String {
    let tokens = format_tokens(script);
    let mut lines = Vec::new();
    let mut indent = 0usize;
    let mut current = Vec::new();

    for token in tokens {
        apply_format_token(token, &mut lines, &mut indent, &mut current);
    }

    flush_format_line(&mut lines, indent, &mut current);
    lines.join("\n") + "\n"
}

fn apply_format_token(
    token: FormatToken,
    lines: &mut Vec<String>,
    indent: &mut usize,
    current: &mut Vec<String>,
) {
    match token {
        FormatToken::Word(text) | FormatToken::Quoted(text) => {
            push_format_value(lines, *indent, current, text)
        }
        FormatToken::Equals => current.push("=".to_string()),
        FormatToken::Open => open_format_block(lines, indent, current),
        FormatToken::Close => close_format_block(lines, indent, current),
        FormatToken::Comment(text) => finish_comment_line(lines, *indent, current, text),
        FormatToken::Newline => flush_format_line(lines, *indent, current),
    }
}

fn push_format_value(
    lines: &mut Vec<String>,
    indent: usize,
    current: &mut Vec<String>,
    text: String,
) {
    flush_completed_assignment(lines, indent, current);
    current.push(text);
}

fn open_format_block(lines: &mut Vec<String>, indent: &mut usize, current: &mut Vec<String>) {
    current.push("{".to_string());
    flush_format_line(lines, *indent, current);
    *indent += 1;
}

fn close_format_block(lines: &mut Vec<String>, indent: &mut usize, current: &mut Vec<String>) {
    flush_format_line(lines, *indent, current);
    *indent = indent.saturating_sub(1);
    lines.push(format!("{}}}", "\t".repeat(*indent)));
}

fn finish_comment_line(
    lines: &mut Vec<String>,
    indent: usize,
    current: &mut Vec<String>,
    text: String,
) {
    current.push(text);
    flush_format_line(lines, indent, current);
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FormatToken {
    Word(String),
    Quoted(String),
    Equals,
    Open,
    Close,
    Comment(String),
    Newline,
}

fn format_tokens(script: &str) -> Vec<FormatToken> {
    let mut chars = script.chars().peekable();
    let mut tokens = Vec::new();

    while let Some(character) = chars.next() {
        if let Some(token) = next_format_token(character, &mut chars) {
            tokens.push(token);
        }
    }

    tokens
}

fn next_format_token<I>(character: char, chars: &mut std::iter::Peekable<I>) -> Option<FormatToken>
where
    I: Iterator<Item = char>,
{
    if character.is_whitespace() {
        return whitespace_format_token(character);
    }

    structural_format_token(character)
        .or_else(|| quoted_format_token(character, chars))
        .or_else(|| comment_format_token(character, chars))
        .or_else(|| Some(FormatToken::Word(read_format_word(character, chars))))
}

fn whitespace_format_token(character: char) -> Option<FormatToken> {
    (character == '\n').then_some(FormatToken::Newline)
}

fn structural_format_token(character: char) -> Option<FormatToken> {
    match character {
        '=' => Some(FormatToken::Equals),
        '{' => Some(FormatToken::Open),
        '}' => Some(FormatToken::Close),
        _ => None,
    }
}

fn quoted_format_token<I>(
    character: char,
    chars: &mut std::iter::Peekable<I>,
) -> Option<FormatToken>
where
    I: Iterator<Item = char>,
{
    (character == '"').then(|| FormatToken::Quoted(read_format_string(chars)))
}

fn comment_format_token<I>(
    character: char,
    chars: &mut std::iter::Peekable<I>,
) -> Option<FormatToken>
where
    I: Iterator<Item = char>,
{
    (character == '#').then(|| FormatToken::Comment(read_format_comment(chars)))
}

fn read_format_string<I>(chars: &mut std::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    let mut value = String::from("\"");
    let mut escaped = false;

    for character in chars.by_ref() {
        value.push(character);
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            break;
        }
    }

    value
}

fn read_format_comment<I>(chars: &mut std::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    let mut value = String::from("#");

    while let Some(character) = chars.peek().copied() {
        if character == '\n' {
            break;
        }
        chars.next();
        value.push(character);
    }

    value.trim_end().to_string()
}

fn read_format_word<I>(first: char, chars: &mut std::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    let mut value = String::from(first);

    while let Some(character) = chars.peek().copied() {
        if character.is_whitespace() || matches!(character, '=' | '{' | '}' | '"' | '#') {
            break;
        }
        chars.next();
        value.push(character);
    }

    value
}

fn flush_completed_assignment(lines: &mut Vec<String>, indent: usize, current: &mut Vec<String>) {
    if current.len() >= 3 && current.get(1).is_some_and(|token| token == "=") {
        flush_format_line(lines, indent, current);
    }
}

fn flush_format_line(lines: &mut Vec<String>, indent: usize, current: &mut Vec<String>) {
    if current.is_empty() {
        return;
    }

    lines.push(format!("{}{}", "\t".repeat(indent), current.join(" ")));
    current.clear();
}
