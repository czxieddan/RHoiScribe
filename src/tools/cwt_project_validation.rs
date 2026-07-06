//------------------------------------------------------------------------------------
// cwt_project_validation.rs -- Part of RHoiScribe
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

use std::{fs, io, path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{
    RhoiScribeRuntime,
    cwt::workspace::{
        CwtIndexedFile, CwtRulesSource, CwtWorkspaceConfig, CwtWorkspaceMode, CwtWorkspaceSnapshot,
    },
};

use super::{
    ProjectValidationCheck, ProjectValidationRequest, ProjectValidationResult, ScanRoot, ToolError,
    cwt_common::{normalize_path, path_to_string, workspace_snapshot_from_handle},
    cwt_diagnostics::{Hoi4Diagnostic, validate_content},
    project_validation,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectValidationToolRequest {
    #[serde(default)]
    pub roots: Vec<ScanRoot>,
    pub include_game_roots: Option<bool>,
    pub validation_mode: Option<String>,
    pub handle_id: Option<String>,
}

struct CwtProjectSnapshot {
    handle_id: String,
    snapshot: Arc<CwtWorkspaceSnapshot>,
}

struct CwtProjectStats {
    indexed_file_count: usize,
    workspace_file_count: usize,
    diagnostic_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectValidationMode {
    Legacy,
    Cwt,
    Hybrid,
}

pub fn validate_project(
    runtime: Arc<RhoiScribeRuntime>,
    request: ProjectValidationToolRequest,
) -> Result<ProjectValidationResult, ToolError> {
    match validation_mode(request.validation_mode.as_deref())? {
        ProjectValidationMode::Legacy => legacy_project_validation(&request),
        ProjectValidationMode::Cwt => cwt_project_validation(&runtime, &request),
        ProjectValidationMode::Hybrid => hybrid_project_validation(runtime, request),
    }
}

fn legacy_project_validation(
    request: &ProjectValidationToolRequest,
) -> Result<ProjectValidationResult, ToolError> {
    project_validation::validate_hoi4_project(ProjectValidationRequest {
        roots: request.roots.clone(),
        include_game_roots: request.include_game_roots,
    })
    .map_err(ToolError::InvalidRequest)
}

fn hybrid_project_validation(
    runtime: Arc<RhoiScribeRuntime>,
    request: ProjectValidationToolRequest,
) -> Result<ProjectValidationResult, ToolError> {
    if request.handle_id.is_some() && request.roots.is_empty() {
        return cwt_project_validation(&runtime, &request);
    }

    let legacy = legacy_project_validation(&request)?;
    let cwt = cwt_project_validation(&runtime, &request)?;
    Ok(merge_project_validation(legacy, cwt))
}

fn cwt_project_validation(
    runtime: &Arc<RhoiScribeRuntime>,
    request: &ProjectValidationToolRequest,
) -> Result<ProjectValidationResult, ToolError> {
    validate_project_roots(request)?;

    let run = project_validation_snapshot(runtime, request)?;
    let mut checks = snapshot_checks(&run.snapshot);
    add_empty_validation_check(&mut checks);
    sort_project_checks(&mut checks);

    let stats = project_stats(&run.snapshot, &checks);
    Ok(ProjectValidationResult {
        status: project_status(&checks),
        index_summary: project_summary(&stats),
        messages: project_messages(&run.handle_id),
        checks,
    })
}

fn validate_project_roots(request: &ProjectValidationToolRequest) -> Result<(), ToolError> {
    if request.handle_id.is_none() && request.roots.is_empty() {
        return Err(ToolError::InvalidRequest(
            "at least one project root is required".to_string(),
        ));
    }
    Ok(())
}

fn project_validation_snapshot(
    runtime: &Arc<RhoiScribeRuntime>,
    request: &ProjectValidationToolRequest,
) -> Result<CwtProjectSnapshot, ToolError> {
    if let Some(handle_id) = &request.handle_id {
        return Ok(CwtProjectSnapshot {
            handle_id: handle_id.clone(),
            snapshot: workspace_snapshot_from_handle(runtime, handle_id)?,
        });
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
    Ok(CwtProjectSnapshot {
        handle_id: handle.id().to_string(),
        snapshot,
    })
}

fn snapshot_checks(snapshot: &CwtWorkspaceSnapshot) -> Vec<ProjectValidationCheck> {
    let mut checks = Vec::new();
    for file in snapshot.files.iter().filter(|file| should_validate(file)) {
        extend_file_checks(&mut checks, snapshot, file);
    }
    checks
}

fn extend_file_checks(
    checks: &mut Vec<ProjectValidationCheck>,
    snapshot: &CwtWorkspaceSnapshot,
    file: &CwtIndexedFile,
) {
    match fs::read_to_string(&file.absolute_path) {
        Ok(content) => checks.extend(
            validate_content(&snapshot.rules, &file.path, &content)
                .into_iter()
                .map(project_check_from_diagnostic),
        ),
        Err(error) => checks.push(file_read_error_check(file, error)),
    }
}

fn should_validate(file: &CwtIndexedFile) -> bool {
    file.root_role == "mod" && is_script_path(&file.path)
}

fn file_read_error_check(file: &CwtIndexedFile, error: io::Error) -> ProjectValidationCheck {
    ProjectValidationCheck {
        id: "file_read_error".to_string(),
        status: "yellow".to_string(),
        severity: "warning".to_string(),
        path: normalize_path(&file.path),
        line: 1,
        message: format!(
            "Failed to read indexed CWT validation file `{}`: {}",
            path_to_string(&file.absolute_path),
            error
        ),
        quick_fix: Some(
            "Refresh the CWT workspace after file moves, deletes, or lock changes.".to_string(),
        ),
    }
}

fn add_empty_validation_check(checks: &mut Vec<ProjectValidationCheck>) {
    if !checks.is_empty() {
        return;
    }
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

fn project_stats(
    snapshot: &CwtWorkspaceSnapshot,
    checks: &[ProjectValidationCheck],
) -> CwtProjectStats {
    CwtProjectStats {
        indexed_file_count: snapshot.files.len(),
        workspace_file_count: snapshot
            .files
            .iter()
            .filter(|file| file.root_role == "mod")
            .count(),
        diagnostic_count: checks
            .iter()
            .filter(|check| check.id != "cwt_diagnostics" || check.status != "green")
            .count(),
    }
}

fn project_summary(stats: &CwtProjectStats) -> String {
    format!(
        "CWT indexed {} file(s) and checked {} workspace file(s), {} diagnostic(s)",
        stats.indexed_file_count, stats.workspace_file_count, stats.diagnostic_count
    )
}

fn project_messages(handle_id: &str) -> Vec<String> {
    vec![
        "CWT validation mode uses embedded GitHub rules in process memory only.".to_string(),
        format!("CWT workspace handle: {handle_id}"),
    ]
}

fn workspace_config_from_project_request(
    request: &ProjectValidationToolRequest,
) -> Result<CwtWorkspaceConfig, ToolError> {
    let workspace_root = project_workspace_root(request)?;
    let vanilla_root = project_vanilla_root(request);
    let mode = project_workspace_mode(request, &vanilla_root);

    Ok(CwtWorkspaceConfig {
        workspace_root,
        rules_source: CwtRulesSource::EmbeddedRulesCrate,
        vanilla_root,
        ignore_globs: vec!["target".to_string(), "tmp".to_string(), ".git".to_string()],
        localisation_languages: vec!["english".to_string()],
        mode,
    })
}

fn project_workspace_root(request: &ProjectValidationToolRequest) -> Result<PathBuf, ToolError> {
    request
        .roots
        .iter()
        .find(|root| !is_game_or_dlc_root(root))
        .or_else(|| request.roots.first())
        .map(|root| PathBuf::from(&root.path))
        .ok_or_else(|| {
            ToolError::InvalidRequest("at least one project root is required".to_string())
        })
}

fn project_vanilla_root(request: &ProjectValidationToolRequest) -> Option<PathBuf> {
    request
        .roots
        .iter()
        .find(|root| {
            root.role
                .as_deref()
                .is_some_and(|role| role.eq_ignore_ascii_case("game"))
        })
        .map(|root| PathBuf::from(&root.path))
}

fn project_workspace_mode(
    request: &ProjectValidationToolRequest,
    vanilla_root: &Option<PathBuf>,
) -> CwtWorkspaceMode {
    if request.include_game_roots.unwrap_or(false) && vanilla_root.is_some() {
        CwtWorkspaceMode::Full
    } else {
        CwtWorkspaceMode::ModOnly
    }
}

fn is_game_or_dlc_root(root: &ScanRoot) -> bool {
    matches!(
        root.role.as_deref().map(str::to_ascii_lowercase).as_deref(),
        Some("game" | "dlc")
    )
}

fn validation_mode(mode: Option<&str>) -> Result<ProjectValidationMode, ToolError> {
    match mode.map(str::to_ascii_lowercase).as_deref() {
        Some("legacy") | Some("legacy_only") | Some("legacy-only") => {
            Ok(ProjectValidationMode::Legacy)
        }
        Some("cwt") | Some("cwt_only") | Some("cwt-only") => Ok(ProjectValidationMode::Cwt),
        None | Some("hybrid") | Some("cwt_legacy") | Some("cwt+legacy") => {
            Ok(ProjectValidationMode::Hybrid)
        }
        Some(other) => Err(ToolError::InvalidRequest(format!(
            "unsupported project validation mode `{other}`"
        ))),
    }
}

fn merge_project_validation(
    mut legacy: ProjectValidationResult,
    cwt: ProjectValidationResult,
) -> ProjectValidationResult {
    let cwt_parse_paths = cwt_parse_paths(&cwt);
    legacy.checks.retain(|check| {
        !(cwt_parse_paths.contains(&check.path)
            && matches!(check.id.as_str(), "brace_balance" | "unclosed_block"))
    });
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

fn cwt_parse_paths(cwt: &ProjectValidationResult) -> std::collections::BTreeSet<String> {
    cwt.checks
        .iter()
        .filter(|check| check.id == "cwt_parse_error")
        .map(|check| check.path.clone())
        .collect()
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
