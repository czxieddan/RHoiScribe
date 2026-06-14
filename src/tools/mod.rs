mod environment;
mod error_log;
mod gui_gfx_asset;
mod hoi4_keys;
mod paradox_lexer;
mod project_files;
mod project_index;
mod project_repair;
mod project_validation;
mod script_edit;
#[cfg(test)]
mod test_support;
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
pub use project_index::{IndexedFile, ProjectIndexItem, ProjectIndexRequest, ProjectIndexResult};
pub use project_repair::{
    FfmpegStatus, RepairChange, RepairCheck, RepairHoi4ProjectRequest, RepairHoi4ProjectResult,
};
pub use project_validation::{
    ProjectValidationCheck, ProjectValidationRequest, ProjectValidationResult,
};
pub use script_edit::{EditHoi4ScriptFileRequest, EditHoi4ScriptFileResult, ScriptEditOperation};
pub use unique_scan::{
    CandidateScanResult, IdentifierCandidate, IdentifierMatch, PathRisk, ScanRoot,
    UniqueIdentifierScanRequest, UniqueIdentifierScanResult,
};

pub const MODULE_PURPOSE: &str = "batch generation and validation tools";

const TOOL_SPECS: &[ToolSpec] = &[
    ToolSpec {
        name: "generate_localisation_batch",
        title: "Generate localisation batch",
        description: "Generate a HOI4 localisation yml file with UTF-8 BOM. file_stem may include nested subdirectories or a mod-relative localisation/<language>/ path; filenames are normalized to the usual _l_<language>.yml suffix.",
        required: &["language", "file_stem", "entries", "dry_run"],
        handler: call_generate_localisation_batch,
    },
    ToolSpec {
        name: "generate_focus_batch",
        title: "Generate focus batch",
        description: "Generate a minimal national focus file and matching localisation dry-run.",
        required: &["country_tag", "tree_id", "focuses", "dry_run"],
        handler: call_generate_focus_batch,
    },
    ToolSpec {
        name: "generate_event_batch",
        title: "Generate event batch",
        description: "Generate a minimal HOI4 country event file and matching localisation dry-run.",
        required: &["namespace", "events", "dry_run"],
        handler: call_generate_event_batch,
    },
    ToolSpec {
        name: "generate_decision_batch",
        title: "Generate decision batch",
        description: "Generate a minimal decision category file and matching localisation dry-run.",
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
        name: "classify_error_log",
        title: "Classify HOI4 error log",
        description: "Group error.log lines by likely HOI4 subsystem and link messages back to changed files when paths are provided.",
        required: &["error_log_path"],
        handler: call_classify_error_log,
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
        description: "Run red/yellow/green static checks over indexed HOI4 roots for duplicate definitions, brace balance, missing GFX textures or sprites, localisation references, and replace_path risks.",
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
        description: "Modify an existing HOI4 txt/gui/gfx/lua script file inside workspace_root by replacing a named block or inserting a new named block, with dry-run preview, brace checks, formatting, and encoding preservation.",
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
pub struct BatchEntry {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalisationBatchRequest {
    pub language: String,
    pub file_stem: String,
    pub key_prefix: Option<String>,
    pub entries: Vec<BatchEntry>,
    pub dry_run: bool,
    pub output_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FocusBatchRequest {
    pub country_tag: String,
    pub tree_id: String,
    pub focuses: Vec<BatchEntry>,
    pub dry_run: bool,
    pub output_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventBatchRequest {
    pub namespace: String,
    pub events: Vec<BatchEntry>,
    pub dry_run: bool,
    pub output_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecisionBatchRequest {
    pub category_id: String,
    pub decisions: Vec<BatchEntry>,
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
        let tool = self
            .tools
            .iter()
            .find(|tool| tool.name == name)
            .ok_or_else(|| ToolError::UnknownTool(name.to_string()))?;
        (tool.handler)(arguments)
    }
}

impl ToolSpec {
    fn as_mcp_tool(&self) -> Tool {
        Tool::new(
            Cow::Borrowed(self.name),
            Cow::Borrowed(self.description),
            input_schema(self.required),
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
        let language_dir = language_directory(&request.language);
        let path = localisation_path(&language_dir, &request.file_stem, &request.language);
        let mut content = format!("{}:\n", request.language);

        for entry in &request.entries {
            let key = localised_key(&request.key_prefix, &entry.id);
            content.push_str(&format!(" {}:0 \"{}\"\n", key, entry.title));
            if let Some(description) = &entry.description {
                content.push_str(&format!(" {}_desc:0 \"{}\"\n", key, description));
            }
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
            content.push_str(&format!(
                "\tfocus = {{\n\t\tid = {}\n\t\ticon = GFX_focus_{}\n\t\tx = {}\n\t\ty = 0\n\t\tcost = 10\n\t\tcompletion_reward = {{ add_political_power = 50 }}\n\t}}\n",
                focus.id,
                focus.id,
                index * 2
            ));
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

        for (index, _event) in request.events.iter().enumerate() {
            content.push_str(&format!(
                "country_event = {{\n\tid = {}.{}\n\ttitle = {}.{}.t\n\tdesc = {}.{}.d\n\tis_triggered_only = yes\n\toption = {{\n\t\tname = {}.{}.a\n\t}}\n}}\n\n",
                request.namespace,
                index + 1,
                request.namespace,
                index + 1,
                request.namespace,
                index + 1,
                request.namespace,
                index + 1
            ));
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
        let mut content = format!(
            "{} = {{\n\ticon = generic_decisions\n\n",
            request.category_id
        );

        for decision in &request.decisions {
            content.push_str(&format!(
                "\t{} = {{\n\t\ticon = generic_decision\n\t\tcost = 25\n\t\tavailable = {{ always = yes }}\n\t\tcomplete_effect = {{ add_political_power = -25 }}\n\t}}\n",
                decision.id
            ));
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

    pub fn classify_error_log(
        request: ClassifyErrorLogRequest,
    ) -> Result<ErrorLogClassificationResult, ToolError> {
        error_log::classify_error_log(request).map_err(ToolError::InvalidRequest)
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

fn call_classify_error_log(arguments: JsonObject) -> Result<CallToolResult, ToolError> {
    let request = parse_arguments::<ClassifyErrorLogRequest>(arguments)?;
    Ok(structured_result(ToolEngine::classify_error_log(request)?))
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

fn input_schema(required: &[&str]) -> JsonObject {
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
    schema
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
