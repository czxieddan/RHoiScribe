//------------------------------------------------------------------------------------
// cwt_intelligence.rs -- Part of RHoiScribe
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

use std::{collections::BTreeSet, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{
    RhoiScribeRuntime,
    cwt::rules::{HOI4_CWT_CONFIG_CONTENT_SHA256, HOI4_CWT_CONFIG_REVISION},
};

use super::{
    ProjectIndexItem, ScanRoot, ToolError,
    cwt_indexing::{CwtIndexQuery, index_project},
    cwt_profiles::{
        diagnostic_explanation, embedded_source_path, normalized_code, rule_profile_for_path,
        string_vec,
    },
};

const CWT_INTELLIGENCE_TOOL_NAMES: &[&str] = &[
    "explain_hoi4_diagnostic",
    "list_hoi4_workspace_symbols",
    "find_hoi4_definition",
    "find_hoi4_references",
    "suggest_hoi4_completion",
    "inspect_hoi4_scope",
    "inspect_hoi4_type_rule",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExplainHoi4DiagnosticRequest {
    pub code: Option<String>,
    pub message: Option<String>,
    pub path: Option<String>,
    pub line: Option<usize>,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListHoi4WorkspaceSymbolsRequest {
    pub handle_id: Option<String>,
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub roots: Vec<ScanRoot>,
    pub include_game_roots: Option<bool>,
    #[serde(default)]
    pub kinds: Vec<String>,
    pub query: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindHoi4DefinitionRequest {
    pub handle_id: Option<String>,
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub roots: Vec<ScanRoot>,
    pub include_game_roots: Option<bool>,
    pub identifier: String,
    pub kind: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindHoi4ReferencesRequest {
    pub handle_id: Option<String>,
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub roots: Vec<ScanRoot>,
    pub include_game_roots: Option<bool>,
    pub identifier: String,
    pub kind: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InspectHoi4ScopeRequest {
    pub handle_id: Option<String>,
    pub workspace_root: Option<String>,
    pub path: String,
    #[serde(default)]
    pub node_path: Vec<String>,
    pub diagnostic_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InspectHoi4TypeRuleRequest {
    pub handle_id: Option<String>,
    pub workspace_root: Option<String>,
    pub path: String,
    #[serde(default)]
    pub node_path: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExplainHoi4DiagnosticResult {
    pub code: Option<String>,
    pub path: Option<String>,
    pub line: Option<usize>,
    pub severity: String,
    pub meaning: String,
    pub repair_guidance: String,
    pub related_tools: Vec<String>,
    pub source: String,
    pub confidence: String,
    pub rule_source_revision: String,
    pub runtime_disk_entities: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListHoi4WorkspaceSymbolsResult {
    pub symbols: Vec<Hoi4LanguageSymbol>,
    pub indexed_file_count: usize,
    pub source: String,
    pub rule_source_revision: String,
    pub rule_content_sha256: String,
    pub runtime_disk_entities: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindHoi4DefinitionResult {
    pub identifier: String,
    pub definitions: Vec<Hoi4LanguageSymbol>,
    pub source: String,
    pub rule_source_revision: String,
    pub runtime_disk_entities: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindHoi4ReferencesResult {
    pub identifier: String,
    pub references: Vec<Hoi4LanguageSymbol>,
    pub source: String,
    pub rule_source_revision: String,
    pub runtime_disk_entities: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InspectHoi4ScopeResult {
    pub path: String,
    pub node_path: Vec<String>,
    pub scope_context: String,
    pub allowed_effects: Vec<String>,
    pub allowed_triggers: Vec<String>,
    pub rule_source_path: Option<String>,
    pub rule_source_revision: String,
    pub source: String,
    pub confidence: String,
    pub runtime_disk_entities: bool,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InspectHoi4TypeRuleResult {
    pub path: String,
    pub node_path: Vec<String>,
    pub rule_name: String,
    pub type_name: String,
    pub path_kind: String,
    pub scope_context: String,
    pub rule_source_path: Option<String>,
    pub rule_source_revision: String,
    pub source: String,
    pub confidence: String,
    pub runtime_disk_entities: bool,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hoi4LanguageSymbol {
    pub kind: String,
    pub name: String,
    pub root: String,
    pub root_role: Option<String>,
    pub path: String,
    pub line: usize,
    pub context: String,
    pub rule_name: Option<String>,
    pub scope_context: Option<String>,
    pub source: String,
    pub confidence: String,
}

pub fn is_cwt_intelligence_tool(name: &str) -> bool {
    CWT_INTELLIGENCE_TOOL_NAMES.contains(&name)
}

pub fn explain_diagnostic(
    request: ExplainHoi4DiagnosticRequest,
) -> Result<ExplainHoi4DiagnosticResult, ToolError> {
    let code = normalized_code(request.code.as_deref(), request.message.as_deref());
    let explanation = diagnostic_explanation(code.as_deref(), request.message.as_deref());

    Ok(ExplainHoi4DiagnosticResult {
        code,
        path: request.path,
        line: request.line,
        severity: explanation.severity.to_string(),
        meaning: explanation.meaning.to_string(),
        repair_guidance: explanation.repair_guidance.to_string(),
        related_tools: vec![
            "validate_hoi4_file".to_string(),
            "inspect_hoi4_type_rule".to_string(),
            "inspect_hoi4_scope".to_string(),
        ],
        source: explanation.source.to_string(),
        confidence: explanation.confidence.to_string(),
        rule_source_revision: HOI4_CWT_CONFIG_REVISION.to_string(),
        runtime_disk_entities: false,
    })
}

pub fn list_workspace_symbols(
    runtime: Arc<RhoiScribeRuntime>,
    request: ListHoi4WorkspaceSymbolsRequest,
) -> Result<ListHoi4WorkspaceSymbolsResult, ToolError> {
    let limit = bounded_limit(request.limit, 500);
    let kinds = request
        .kinds
        .iter()
        .map(|kind| kind.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let query = request.query.as_deref().map(str::to_ascii_lowercase);
    let index = index_project(
        &runtime,
        CwtIndexQuery {
            handle_id: request.handle_id.as_deref(),
            workspace_root: request.workspace_root.as_deref(),
            roots: &request.roots,
            include_game_roots: request.include_game_roots,
            missing_roots_message: "handle_id, workspace_root, or roots is required for workspace language queries",
        },
    )?;
    let indexed_file_count = index.files.len();
    let symbols = index
        .definitions
        .iter()
        .filter(|item| kinds.is_empty() || kinds.contains(&item.kind.to_ascii_lowercase()))
        .filter(|item| {
            query
                .as_deref()
                .is_none_or(|query| item.name.to_ascii_lowercase().contains(query))
        })
        .take(limit)
        .map(language_symbol_from_index_item)
        .collect::<Vec<_>>();

    Ok(ListHoi4WorkspaceSymbolsResult {
        symbols,
        indexed_file_count,
        source: "rhoiscribe_project_index_with_embedded_cwt_profiles".to_string(),
        rule_source_revision: HOI4_CWT_CONFIG_REVISION.to_string(),
        rule_content_sha256: HOI4_CWT_CONFIG_CONTENT_SHA256.to_string(),
        runtime_disk_entities: false,
        message: "listed workspace symbols from the in-process workspace root and RHoiScribe index; CWT TypeIndex integration remains incremental".to_string(),
    })
}

pub fn find_definition(
    runtime: Arc<RhoiScribeRuntime>,
    request: FindHoi4DefinitionRequest,
) -> Result<FindHoi4DefinitionResult, ToolError> {
    let limit = bounded_limit(request.limit, 100);
    let index = index_project(
        &runtime,
        CwtIndexQuery {
            handle_id: request.handle_id.as_deref(),
            workspace_root: request.workspace_root.as_deref(),
            roots: &request.roots,
            include_game_roots: request.include_game_roots,
            missing_roots_message: "handle_id, workspace_root, or roots is required for workspace language queries",
        },
    )?;
    let definitions = matching_items(
        &index.definitions,
        &request.identifier,
        request.kind.as_deref(),
    )
    .into_iter()
    .take(limit)
    .map(language_symbol_from_index_item)
    .collect::<Vec<_>>();

    Ok(FindHoi4DefinitionResult {
        identifier: request.identifier,
        definitions,
        source: "rhoiscribe_project_index".to_string(),
        rule_source_revision: HOI4_CWT_CONFIG_REVISION.to_string(),
        runtime_disk_entities: false,
        message: "resolved definitions from process-local analysis without CWT disk caches"
            .to_string(),
    })
}

pub fn find_references(
    runtime: Arc<RhoiScribeRuntime>,
    request: FindHoi4ReferencesRequest,
) -> Result<FindHoi4ReferencesResult, ToolError> {
    let limit = bounded_limit(request.limit, 100);
    let index = index_project(
        &runtime,
        CwtIndexQuery {
            handle_id: request.handle_id.as_deref(),
            workspace_root: request.workspace_root.as_deref(),
            roots: &request.roots,
            include_game_roots: request.include_game_roots,
            missing_roots_message: "handle_id, workspace_root, or roots is required for workspace language queries",
        },
    )?;
    let references = matching_items(
        &index.references,
        &request.identifier,
        request.kind.as_deref(),
    )
    .into_iter()
    .take(limit)
    .map(language_symbol_from_index_item)
    .collect::<Vec<_>>();

    Ok(FindHoi4ReferencesResult {
        identifier: request.identifier,
        references,
        source: "rhoiscribe_project_index".to_string(),
        rule_source_revision: HOI4_CWT_CONFIG_REVISION.to_string(),
        runtime_disk_entities: false,
        message: "resolved references from process-local analysis without CWT disk caches"
            .to_string(),
    })
}

pub fn inspect_scope(
    _runtime: Arc<RhoiScribeRuntime>,
    request: InspectHoi4ScopeRequest,
) -> Result<InspectHoi4ScopeResult, ToolError> {
    let profile = rule_profile_for_path(&request.path);
    Ok(InspectHoi4ScopeResult {
        path: normalize_path(&request.path),
        node_path: request.node_path,
        scope_context: profile.scope_context.to_string(),
        allowed_effects: string_vec(&profile.allowed_effects),
        allowed_triggers: string_vec(&profile.allowed_triggers),
        rule_source_path: embedded_source_path(profile.source_hint),
        rule_source_revision: HOI4_CWT_CONFIG_REVISION.to_string(),
        source: "embedded_cwt_rule_profile".to_string(),
        confidence: profile.confidence.to_string(),
        runtime_disk_entities: false,
        limitations: vec![
            "Scope output is currently a stable CWT-profile summary; deeper cwtools TypeIndex/InfoService scope tracing is planned for a later increment.".to_string(),
        ],
    })
}

pub fn inspect_type_rule(
    _runtime: Arc<RhoiScribeRuntime>,
    request: InspectHoi4TypeRuleRequest,
) -> Result<InspectHoi4TypeRuleResult, ToolError> {
    let profile = rule_profile_for_path(&request.path);
    Ok(InspectHoi4TypeRuleResult {
        path: normalize_path(&request.path),
        node_path: request.node_path,
        rule_name: profile.rule_name.to_string(),
        type_name: profile.type_name.to_string(),
        path_kind: profile.path_kind.to_string(),
        scope_context: profile.scope_context.to_string(),
        rule_source_path: embedded_source_path(profile.source_hint),
        rule_source_revision: HOI4_CWT_CONFIG_REVISION.to_string(),
        source: "embedded_cwt_rule_profile".to_string(),
        confidence: profile.confidence.to_string(),
        runtime_disk_entities: false,
        limitations: vec![
            "Type rule output is derived from embedded CWT rule-file profiles and path context; exact cwtools TypeIndex nodes are not exposed yet.".to_string(),
        ],
    })
}

fn matching_items<'a>(
    items: &'a [ProjectIndexItem],
    identifier: &str,
    kind: Option<&str>,
) -> Vec<&'a ProjectIndexItem> {
    items
        .iter()
        .filter(|item| item.name == identifier)
        .filter(|item| kind.is_none_or(|kind| item.kind.eq_ignore_ascii_case(kind)))
        .collect()
}

fn language_symbol_from_index_item(item: &ProjectIndexItem) -> Hoi4LanguageSymbol {
    let profile = rule_profile_for_path(&item.path);
    Hoi4LanguageSymbol {
        kind: item.kind.clone(),
        name: item.name.clone(),
        root: normalize_path(&item.root),
        root_role: item.root_role.clone(),
        path: normalize_path(&item.path),
        line: item.line,
        context: item.context.clone(),
        rule_name: Some(profile.rule_name.to_string()),
        scope_context: Some(profile.scope_context.to_string()),
        source: "rhoiscribe_project_index".to_string(),
        confidence: "medium".to_string(),
    }
}

fn bounded_limit(limit: Option<usize>, default: usize) -> usize {
    limit.unwrap_or(default).clamp(1, default)
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}
