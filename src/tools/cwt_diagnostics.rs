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
            LoadedCwtRules,
        },
        workspace::{
            CwtRulesSource, CwtWorkspaceConfig, CwtWorkspaceMode, CwtWorkspaceStatus,
            CwtWorkspaceWarmState,
        },
    },
};

use super::ToolError;

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
pub struct Hoi4LanguageWorkspaceStatus {
    pub handle_id: String,
    pub generation: u64,
    pub state: String,
    pub indexed_file_count: usize,
    pub workspace_file_count: usize,
    pub vanilla_file_count: usize,
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

    !arguments
        .as_object()
        .and_then(|arguments| arguments.get("validation_mode"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|mode| matches!(mode, "legacy" | "legacy_only" | "legacy-only"))
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

fn rules_source(path: Option<String>) -> CwtRulesSource {
    path.filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .map(CwtRulesSource::ExternalPath)
        .unwrap_or(CwtRulesSource::EmbeddedRulesCrate)
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

fn default_languages(languages: Vec<String>) -> Vec<String> {
    if languages.is_empty() {
        vec!["english".to_string()]
    } else {
        languages
    }
}

pub(super) fn validate_content(
    rules: &LoadedCwtRules,
    path: &str,
    content: &str,
) -> Vec<Hoi4Diagnostic> {
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

fn language_status(
    status: CwtWorkspaceStatus,
    vanilla_status: String,
) -> Hoi4LanguageWorkspaceStatus {
    let vanilla_status =
        if status.vanilla_file_count > 0 && vanilla_status.starts_with("configured:") {
            vanilla_status.replacen("configured:", "indexed:", 1)
        } else {
            vanilla_status
        };

    Hoi4LanguageWorkspaceStatus {
        handle_id: status.handle_id,
        generation: status.generation,
        state: warm_state(status.state),
        indexed_file_count: status.indexed_file_count,
        workspace_file_count: status.workspace_file_count,
        vanilla_file_count: status.vanilla_file_count,
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

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
