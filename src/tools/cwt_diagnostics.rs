//------------------------------------------------------------------------------------
// cwt_diagnostics.rs -- Part of RHoiScribe
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

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
    RhoiScribeRuntime,
    cwt::{
        rules::{
            CwtRuleLoadError, CwtValidationDiagnostic, HOI4_CWT_CONFIG_CONTENT_SHA256,
            HOI4_CWT_CONFIG_REVISION, HOI4_CWT_CONFIG_SOURCE_COUNT, HOI4_CWT_CONFIG_TOTAL_BYTES,
            LoadedCwtRules, load_embedded_hoi4_cwt_rules,
        },
        workspace::{
            CwtRulesSource, CwtWorkspaceConfig, CwtWorkspaceMode, CwtWorkspaceSnapshot,
            CwtWorkspaceStatus, CwtWorkspaceWarmState,
        },
    },
};

use super::{
    ProjectValidationCheck, ProjectValidationRequest, ProjectValidationResult, ScanRoot, ToolError,
    project_validation,
};

const CWT_TOOL_NAMES: &[&str] = &[
    "open_hoi4_language_workspace",
    "get_hoi4_language_status",
    "validate_hoi4_file",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenHoi4LanguageWorkspaceRequest {
    pub workspace_root: String,
    pub vanilla_root: Option<String>,
    #[serde(default)]
    pub ignore_globs: Vec<String>,
    #[serde(default)]
    pub localisation_languages: Vec<String>,
    pub mode: Option<String>,
    pub rules_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetHoi4LanguageStatusRequest {
    pub handle_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidateHoi4FileRequest {
    pub handle_id: Option<String>,
    pub workspace_root: Option<String>,
    pub path: String,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectValidationToolRequest {
    pub roots: Vec<ScanRoot>,
    pub include_game_roots: Option<bool>,
    pub validation_mode: Option<String>,
    pub handle_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenHoi4LanguageWorkspaceResult {
    pub handle_id: String,
    pub status: Hoi4LanguageWorkspaceStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetHoi4LanguageStatusResult {
    pub workspaces: Vec<Hoi4LanguageWorkspaceStatus>,
    pub runtime_disk_entities: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidateHoi4FileResult {
    pub path: String,
    pub handle_id: Option<String>,
    pub diagnostics: Vec<Hoi4Diagnostic>,
    pub status: String,
    pub rule_revision: String,
    pub rule_content_sha256: String,
    pub runtime_disk_entities: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hoi4LanguageWorkspaceStatus {
    pub handle_id: String,
    pub generation: u64,
    pub state: String,
    pub indexed_file_count: usize,
    pub validation_diagnostic_count: usize,
    pub rule_diagnostic_count: usize,
    pub stale: bool,
    pub last_error: Option<String>,
    pub memory_mode: String,
    pub rules_revision: String,
    pub rule_content_sha256: String,
    pub rule_source_count: usize,
    pub rule_source_bytes: usize,
    pub runtime_disk_entities: bool,
    pub vanilla_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hoi4Diagnostic {
    pub id: String,
    pub code: Option<String>,
    pub status: String,
    pub severity: String,
    pub source: String,
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub quick_fix: Option<String>,
}

struct FileValidationContext {
    rules: Arc<LoadedCwtRules>,
    handle_id: Option<String>,
    workspace_root: Option<PathBuf>,
}

pub fn is_cwt_diagnostics_tool(name: &str) -> bool {
    CWT_TOOL_NAMES.contains(&name)
}

pub fn should_skip_tool_log(name: &str, arguments: &serde_json::Value) -> bool {
    if is_cwt_diagnostics_tool(name) {
        return true;
    }

    if name != "validate_hoi4_project" {
        return false;
    }

    arguments
        .as_object()
        .and_then(|arguments| arguments.get("validation_mode"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|mode| !matches!(mode, "legacy" | "legacy_only" | "legacy-only"))
}

pub fn open_language_workspace(
    runtime: Arc<RhoiScribeRuntime>,
    request: OpenHoi4LanguageWorkspaceRequest,
) -> Result<OpenHoi4LanguageWorkspaceResult, ToolError> {
    let config = workspace_config_from_open_request(request)?;
    let vanilla_status = vanilla_status(&config);
    let handle = runtime
        .cwt_language()
        .open_workspace(config)
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?;
    let status = handle
        .status()
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?;

    Ok(OpenHoi4LanguageWorkspaceResult {
        handle_id: handle.id().to_string(),
        status: language_status(status, vanilla_status),
        message: "scheduled in-memory CWT workspace warm-up".to_string(),
    })
}

pub fn get_language_status(
    runtime: Arc<RhoiScribeRuntime>,
    request: GetHoi4LanguageStatusRequest,
) -> Result<GetHoi4LanguageStatusResult, ToolError> {
    let statuses = if let Some(handle_id) = request.handle_id {
        let handle = runtime
            .cwt_language()
            .get_workspace(&handle_id)
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
            .ok_or_else(|| {
                ToolError::InvalidRequest(format!("unknown CWT workspace `{handle_id}`"))
            })?;
        let vanilla_status = vanilla_status(handle.config());
        vec![language_status(
            handle
                .status()
                .map_err(|error| ToolError::InvalidRequest(error.to_string()))?,
            vanilla_status,
        )]
    } else {
        runtime
            .cwt_language()
            .list_workspace_statuses()
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
            .into_iter()
            .map(|status| language_status(status, "not_indexed".to_string()))
            .collect()
    };

    Ok(GetHoi4LanguageStatusResult {
        workspaces: statuses,
        runtime_disk_entities: false,
        message: "CWT language state is process memory only".to_string(),
    })
}

pub fn validate_file(
    runtime: Arc<RhoiScribeRuntime>,
    request: ValidateHoi4FileRequest,
) -> Result<ValidateHoi4FileResult, ToolError> {
    let validation_context = rules_for_file_validation(&runtime, &request)?;
    let content = file_content(&request, validation_context.workspace_root.as_deref())?;
    let diagnostics = validate_content(&validation_context.rules, &request.path, &content);

    Ok(ValidateHoi4FileResult {
        path: normalize_path(&request.path),
        handle_id: validation_context.handle_id,
        status: diagnostics_status(&diagnostics),
        diagnostics,
        rule_revision: HOI4_CWT_CONFIG_REVISION.to_string(),
        rule_content_sha256: HOI4_CWT_CONFIG_CONTENT_SHA256.to_string(),
        runtime_disk_entities: false,
    })
}

pub fn validate_project(
    runtime: Arc<RhoiScribeRuntime>,
    request: ProjectValidationToolRequest,
) -> Result<ProjectValidationResult, ToolError> {
    match validation_mode(request.validation_mode.as_deref())? {
        ProjectValidationMode::Legacy => {
            project_validation::validate_hoi4_project(ProjectValidationRequest {
                roots: request.roots,
                include_game_roots: request.include_game_roots,
            })
            .map_err(ToolError::InvalidRequest)
        }
        ProjectValidationMode::Cwt => cwt_project_validation(runtime, request),
        ProjectValidationMode::Hybrid => {
            let legacy = project_validation::validate_hoi4_project(ProjectValidationRequest {
                roots: request.roots.clone(),
                include_game_roots: request.include_game_roots,
            })
            .map_err(ToolError::InvalidRequest)?;
            let cwt = cwt_project_validation(runtime, request)?;
            Ok(merge_project_validation(legacy, cwt))
        }
    }
}

fn cwt_project_validation(
    runtime: Arc<RhoiScribeRuntime>,
    request: ProjectValidationToolRequest,
) -> Result<ProjectValidationResult, ToolError> {
    if request.roots.is_empty() {
        return Err(ToolError::InvalidRequest(
            "at least one project root is required".to_string(),
        ));
    }

    let (handle_id, snapshot) = project_validation_snapshot(runtime, &request)?;

    let mut checks = Vec::new();
    for file in &snapshot.files {
        if !is_script_path(&file.path) {
            continue;
        }
        let full_path = join_relative_path(&snapshot.workspace_root, &file.path);
        let content = fs::read_to_string(&full_path).map_err(|error| {
            ToolError::InvalidRequest(format!(
                "failed to read CWT validation file `{}`: {}",
                path_to_string(&full_path),
                error
            ))
        })?;
        checks.extend(
            validate_content(&snapshot.rules, &file.path, &content)
                .into_iter()
                .map(project_check_from_diagnostic),
        );
    }

    if checks.is_empty() {
        checks.push(ProjectValidationCheck {
            id: "cwt_diagnostics".to_string(),
            status: "green".to_string(),
            severity: "info".to_string(),
            path: String::new(),
            line: 0,
            message: "CWT validation returned no diagnostics for scanned script files.".to_string(),
            quick_fix: None,
        });
    }
    sort_project_checks(&mut checks);

    let status = project_status(&checks);
    let indexed_file_count = snapshot.files.len();
    let diagnostic_count = checks
        .iter()
        .filter(|check| check.id != "cwt_diagnostics" || check.status != "green")
        .count();

    Ok(ProjectValidationResult {
        status,
        index_summary: format!(
            "CWT checked {indexed_file_count} indexed file(s), {diagnostic_count} diagnostic(s)"
        ),
        messages: vec![
            "CWT validation mode uses embedded GitHub rules in process memory only.".to_string(),
            format!("CWT workspace handle: {handle_id}"),
        ],
        checks,
    })
}

fn project_validation_snapshot(
    runtime: Arc<RhoiScribeRuntime>,
    request: &ProjectValidationToolRequest,
) -> Result<(String, Arc<CwtWorkspaceSnapshot>), ToolError> {
    if let Some(handle_id) = &request.handle_id {
        let handle = runtime
            .cwt_language()
            .get_workspace(handle_id)
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
            .ok_or_else(|| {
                ToolError::InvalidRequest(format!("unknown CWT workspace `{handle_id}`"))
            })?;
        if handle
            .snapshot()
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
            .is_none()
        {
            handle
                .refresh_blocking()
                .map_err(|error| ToolError::InvalidRequest(error.to_string()))?;
        }
        let snapshot = handle
            .snapshot()
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
            .ok_or_else(|| {
                ToolError::InvalidRequest("CWT workspace has no warm snapshot".to_string())
            })?;
        return Ok((handle.id().to_string(), snapshot));
    }

    let config = workspace_config_from_project_request(request)?;
    let handle = runtime
        .cwt_language()
        .open_workspace_blocking(config)
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?;
    let snapshot = handle
        .snapshot()
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
        .ok_or_else(|| {
            ToolError::InvalidRequest("CWT workspace has no warm snapshot".to_string())
        })?;
    Ok((handle.id().to_string(), snapshot))
}

fn workspace_config_from_open_request(
    request: OpenHoi4LanguageWorkspaceRequest,
) -> Result<CwtWorkspaceConfig, ToolError> {
    let mode = parse_workspace_mode(request.mode.as_deref())?;
    Ok(CwtWorkspaceConfig {
        workspace_root: PathBuf::from(request.workspace_root),
        rules_source: rules_source(request.rules_path),
        vanilla_root: request.vanilla_root.map(PathBuf::from),
        ignore_globs: request.ignore_globs,
        localisation_languages: default_languages(request.localisation_languages),
        mode,
    })
}

fn workspace_config_from_project_request(
    request: &ProjectValidationToolRequest,
) -> Result<CwtWorkspaceConfig, ToolError> {
    let workspace_root = request
        .roots
        .iter()
        .find(|root| {
            !matches!(
                root.role.as_deref().map(str::to_ascii_lowercase).as_deref(),
                Some("game" | "dlc")
            )
        })
        .or_else(|| request.roots.first())
        .map(|root| PathBuf::from(&root.path))
        .ok_or_else(|| {
            ToolError::InvalidRequest("at least one project root is required".to_string())
        })?;
    let vanilla_root = request
        .roots
        .iter()
        .find(|root| {
            root.role
                .as_deref()
                .is_some_and(|role| role.eq_ignore_ascii_case("game"))
        })
        .map(|root| PathBuf::from(&root.path));
    let mode = if request.include_game_roots.unwrap_or(false) && vanilla_root.is_some() {
        CwtWorkspaceMode::Full
    } else {
        CwtWorkspaceMode::ModOnly
    };

    Ok(CwtWorkspaceConfig {
        workspace_root,
        rules_source: CwtRulesSource::EmbeddedGithubConfig,
        vanilla_root,
        ignore_globs: vec!["target".to_string(), "tmp".to_string(), ".git".to_string()],
        localisation_languages: vec!["english".to_string()],
        mode,
    })
}

fn rules_source(path: Option<String>) -> CwtRulesSource {
    path.filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .map(CwtRulesSource::ExternalPath)
        .unwrap_or(CwtRulesSource::EmbeddedGithubConfig)
}

fn parse_workspace_mode(mode: Option<&str>) -> Result<CwtWorkspaceMode, ToolError> {
    match mode.map(str::to_ascii_lowercase).as_deref() {
        None | Some("mod_only" | "mod-only" | "mod") => Ok(CwtWorkspaceMode::ModOnly),
        Some("full") => Ok(CwtWorkspaceMode::Full),
        Some(other) => Err(ToolError::InvalidRequest(format!(
            "unsupported CWT workspace mode `{other}`"
        ))),
    }
}

fn validation_mode(mode: Option<&str>) -> Result<ProjectValidationMode, ToolError> {
    match mode.map(str::to_ascii_lowercase).as_deref() {
        None | Some("legacy") | Some("legacy_only") | Some("legacy-only") => {
            Ok(ProjectValidationMode::Legacy)
        }
        Some("cwt") | Some("cwt_only") | Some("cwt-only") => Ok(ProjectValidationMode::Cwt),
        Some("hybrid") | Some("cwt_legacy") | Some("cwt+legacy") => {
            Ok(ProjectValidationMode::Hybrid)
        }
        Some(other) => Err(ToolError::InvalidRequest(format!(
            "unsupported project validation mode `{other}`"
        ))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectValidationMode {
    Legacy,
    Cwt,
    Hybrid,
}

fn default_languages(languages: Vec<String>) -> Vec<String> {
    if languages.is_empty() {
        vec!["english".to_string()]
    } else {
        languages
    }
}

fn rules_for_file_validation(
    runtime: &Arc<RhoiScribeRuntime>,
    request: &ValidateHoi4FileRequest,
) -> Result<FileValidationContext, ToolError> {
    if let Some(handle_id) = &request.handle_id {
        let handle = runtime
            .cwt_language()
            .get_workspace(handle_id)
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
            .ok_or_else(|| {
                ToolError::InvalidRequest(format!("unknown CWT workspace `{handle_id}`"))
            })?;
        if let Some(snapshot) = handle
            .snapshot()
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
        {
            return Ok(FileValidationContext {
                rules: Arc::clone(&snapshot.rules),
                handle_id: Some(handle_id.clone()),
                workspace_root: Some(snapshot.workspace_root.clone()),
            });
        }
    }

    let rules = Arc::new(
        load_embedded_hoi4_cwt_rules()
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?,
    );
    Ok(FileValidationContext {
        rules,
        handle_id: request.handle_id.clone(),
        workspace_root: request.workspace_root.as_ref().map(PathBuf::from),
    })
}

fn file_content(
    request: &ValidateHoi4FileRequest,
    workspace_root: Option<&Path>,
) -> Result<String, ToolError> {
    if let Some(content) = &request.content {
        return Ok(content.clone());
    }

    let path = match workspace_root {
        Some(root) if !Path::new(&request.path).is_absolute() => {
            join_relative_path(root, &request.path)
        }
        _ => PathBuf::from(&request.path),
    };
    fs::read_to_string(&path).map_err(|error| {
        ToolError::InvalidRequest(format!(
            "failed to read CWT validation file `{}`: {}",
            path_to_string(&path),
            error
        ))
    })
}

fn validate_content(rules: &LoadedCwtRules, path: &str, content: &str) -> Vec<Hoi4Diagnostic> {
    match rules.validate_script(path, content) {
        Ok(diagnostics) => diagnostics.into_iter().map(validation_diagnostic).collect(),
        Err(error) => vec![load_error_diagnostic(path, error)],
    }
}

fn validation_diagnostic(diagnostic: CwtValidationDiagnostic) -> Hoi4Diagnostic {
    let status = status_from_severity(&diagnostic.severity);
    Hoi4Diagnostic {
        id: diagnostic
            .code
            .as_deref()
            .filter(|code| !code.trim().is_empty())
            .unwrap_or("cwt_validation")
            .to_string(),
        code: diagnostic.code,
        status: status.to_string(),
        severity: diagnostic.severity,
        source: "cwt".to_string(),
        path: normalize_path(&diagnostic.path),
        line: diagnostic.line as usize,
        column: diagnostic.column as usize,
        message: diagnostic.message,
        quick_fix: None,
    }
}

fn load_error_diagnostic(path: &str, error: CwtRuleLoadError) -> Hoi4Diagnostic {
    match error {
        CwtRuleLoadError::ScriptParse {
            line,
            column,
            message,
            ..
        } => Hoi4Diagnostic {
            id: "cwt_parse_error".to_string(),
            code: None,
            status: "red".to_string(),
            severity: "error".to_string(),
            source: "cwt".to_string(),
            path: normalize_path(path),
            line: line as usize,
            column: column as usize,
            message,
            quick_fix: Some("Fix the script syntax before running schema validation.".to_string()),
        },
        other => Hoi4Diagnostic {
            id: "cwt_rules_unavailable".to_string(),
            code: None,
            status: "red".to_string(),
            severity: "error".to_string(),
            source: "cwt".to_string(),
            path: normalize_path(path),
            line: 1,
            column: 0,
            message: other.to_string(),
            quick_fix: Some(
                "Reload the embedded CWT rules or inspect the source metadata.".to_string(),
            ),
        },
    }
}

fn project_check_from_diagnostic(diagnostic: Hoi4Diagnostic) -> ProjectValidationCheck {
    ProjectValidationCheck {
        id: diagnostic.id,
        status: diagnostic.status,
        severity: diagnostic.severity,
        path: diagnostic.path,
        line: diagnostic.line,
        message: diagnostic.message,
        quick_fix: diagnostic.quick_fix,
    }
}

fn status_from_severity(severity: &str) -> &'static str {
    let severity = severity.to_ascii_lowercase();
    if severity.contains("error") {
        "red"
    } else if severity.contains("warning") {
        "yellow"
    } else {
        "green"
    }
}

fn diagnostics_status(diagnostics: &[Hoi4Diagnostic]) -> String {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.status == "red")
    {
        "red".to_string()
    } else if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.status == "yellow")
    {
        "yellow".to_string()
    } else {
        "green".to_string()
    }
}

fn language_status(
    status: CwtWorkspaceStatus,
    vanilla_status: String,
) -> Hoi4LanguageWorkspaceStatus {
    Hoi4LanguageWorkspaceStatus {
        handle_id: status.handle_id,
        generation: status.generation,
        state: warm_state(status.state),
        indexed_file_count: status.indexed_file_count,
        validation_diagnostic_count: status.validation_diagnostic_count,
        rule_diagnostic_count: status.rule_diagnostic_count,
        stale: status.stale,
        last_error: status.last_error,
        memory_mode: "process".to_string(),
        rules_revision: HOI4_CWT_CONFIG_REVISION.to_string(),
        rule_content_sha256: HOI4_CWT_CONFIG_CONTENT_SHA256.to_string(),
        rule_source_count: HOI4_CWT_CONFIG_SOURCE_COUNT,
        rule_source_bytes: HOI4_CWT_CONFIG_TOTAL_BYTES,
        runtime_disk_entities: false,
        vanilla_status,
    }
}

fn warm_state(state: CwtWorkspaceWarmState) -> String {
    match state {
        CwtWorkspaceWarmState::Cold => "cold",
        CwtWorkspaceWarmState::Warming => "warming",
        CwtWorkspaceWarmState::Warm => "warm",
        CwtWorkspaceWarmState::Failed => "failed",
    }
    .to_string()
}

fn vanilla_status(config: &CwtWorkspaceConfig) -> String {
    match (&config.mode, &config.vanilla_root) {
        (CwtWorkspaceMode::Full, Some(path)) => format!("configured:{}", path_to_string(path)),
        (CwtWorkspaceMode::Full, None) => "missing".to_string(),
        _ => "not_indexed".to_string(),
    }
}

fn merge_project_validation(
    mut legacy: ProjectValidationResult,
    cwt: ProjectValidationResult,
) -> ProjectValidationResult {
    legacy.checks.extend(cwt.checks);
    sort_project_checks(&mut legacy.checks);
    legacy.status = project_status(&legacy.checks);
    legacy
        .messages
        .push("Hybrid validation included CWT in-memory diagnostics.".to_string());
    legacy.messages.extend(cwt.messages);
    legacy.index_summary = format!("{}; {}", legacy.index_summary, cwt.index_summary);
    legacy
}

fn sort_project_checks(checks: &mut [ProjectValidationCheck]) {
    checks.sort_by(|left, right| {
        (
            status_rank(&left.status),
            &left.id,
            &left.path,
            left.line,
            &left.message,
        )
            .cmp(&(
                status_rank(&right.status),
                &right.id,
                &right.path,
                right.line,
                &right.message,
            ))
    });
}

fn project_status(checks: &[ProjectValidationCheck]) -> String {
    if checks.iter().any(|check| check.status == "red") {
        "red"
    } else if checks.iter().any(|check| check.status == "yellow") {
        "yellow"
    } else {
        "green"
    }
    .to_string()
}

fn status_rank(status: &str) -> u8 {
    match status {
        "red" => 0,
        "yellow" => 1,
        "green" => 2,
        _ => 3,
    }
}

fn is_script_path(path: &str) -> bool {
    let extension = path.rsplit('.').next().unwrap_or_default();
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "txt" | "gui" | "gfx" | "sfx" | "asset" | "map"
    )
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn join_relative_path(root: &Path, path: &str) -> PathBuf {
    path.split('/')
        .filter(|part| !part.is_empty())
        .fold(root.to_path_buf(), |current, part| current.join(part))
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
