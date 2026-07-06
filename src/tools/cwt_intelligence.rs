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

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
    RhoiScribeRuntime,
    cwt::rules::{
        HOI4_CWT_CONFIG_CONTENT_SHA256, HOI4_CWT_CONFIG_REVISION, read_embedded_hoi4_cwt_sources,
    },
};

use super::{
    ProjectIndexItem, ScanRoot, ToolError,
    project_index::{self, ProjectIndexRequest, ProjectIndexResult},
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
pub struct SuggestHoi4CompletionRequest {
    pub handle_id: Option<String>,
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub roots: Vec<ScanRoot>,
    pub include_game_roots: Option<bool>,
    pub path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub prefix: Option<String>,
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
pub struct SuggestHoi4CompletionResult {
    pub path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub suggestions: Vec<Hoi4CompletionSuggestion>,
    pub scope_context: String,
    pub rule_name: String,
    pub source: String,
    pub rule_source_revision: String,
    pub runtime_disk_entities: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hoi4CompletionSuggestion {
    pub label: String,
    pub insert_text: String,
    pub kind: String,
    pub detail: String,
    pub source: String,
    pub confidence: String,
}

struct IndexQuery<'a> {
    handle_id: Option<&'a str>,
    workspace_root: Option<&'a str>,
    roots: &'a [ScanRoot],
    include_game_roots: Option<bool>,
}

#[derive(Debug, Clone)]
struct RuleProfile {
    rule_name: &'static str,
    type_name: &'static str,
    path_kind: &'static str,
    scope_context: &'static str,
    source_hint: &'static str,
    allowed_effects: Vec<&'static str>,
    allowed_triggers: Vec<&'static str>,
    completions: Vec<CompletionProfile>,
    confidence: &'static str,
}

#[derive(Debug, Clone)]
struct CompletionProfile {
    label: &'static str,
    kind: &'static str,
    detail: &'static str,
}

struct CompletionCandidate<'a> {
    label: &'a str,
    insert_text: &'a str,
    kind: &'a str,
    detail: &'a str,
    source: &'a str,
    confidence: &'a str,
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
        IndexQuery {
            handle_id: request.handle_id.as_deref(),
            workspace_root: request.workspace_root.as_deref(),
            roots: &request.roots,
            include_game_roots: request.include_game_roots,
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
        IndexQuery {
            handle_id: request.handle_id.as_deref(),
            workspace_root: request.workspace_root.as_deref(),
            roots: &request.roots,
            include_game_roots: request.include_game_roots,
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
        IndexQuery {
            handle_id: request.handle_id.as_deref(),
            workspace_root: request.workspace_root.as_deref(),
            roots: &request.roots,
            include_game_roots: request.include_game_roots,
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

pub fn suggest_completion(
    runtime: Arc<RhoiScribeRuntime>,
    request: SuggestHoi4CompletionRequest,
) -> Result<SuggestHoi4CompletionResult, ToolError> {
    let limit = bounded_limit(request.limit, 100);
    let prefix = request.prefix.as_deref().unwrap_or_default();
    let profile = rule_profile_for_path(&request.path);
    let mut seen = BTreeSet::new();
    let mut suggestions = Vec::new();

    for completion in profile.completions.iter() {
        push_completion_if_matches(
            &mut suggestions,
            &mut seen,
            CompletionCandidate {
                label: completion.label,
                insert_text: completion.label,
                kind: completion.kind,
                detail: completion.detail,
                source: "embedded_cwt_rule_profile",
                confidence: profile.confidence,
            },
            prefix,
        );
    }

    if suggestions.len() < limit
        && (request.handle_id.is_some()
            || request.workspace_root.is_some()
            || !request.roots.is_empty())
        && let Ok(index) = index_project(
            &runtime,
            IndexQuery {
                handle_id: request.handle_id.as_deref(),
                workspace_root: request.workspace_root.as_deref(),
                roots: &request.roots,
                include_game_roots: request.include_game_roots,
            },
        )
    {
        for item in index.definitions.iter() {
            push_completion_if_matches(
                &mut suggestions,
                &mut seen,
                CompletionCandidate {
                    label: &item.name,
                    insert_text: &item.name,
                    kind: &item.kind,
                    detail: &format!("workspace {}", item.context),
                    source: "rhoiscribe_project_index",
                    confidence: "medium",
                },
                prefix,
            );
            if suggestions.len() >= limit {
                break;
            }
        }
    }

    suggestions.truncate(limit);

    Ok(SuggestHoi4CompletionResult {
        path: normalize_path(&request.path),
        line: request.line,
        column: request.column,
        suggestions,
        scope_context: profile.scope_context.to_string(),
        rule_name: profile.rule_name.to_string(),
        source: "embedded_cwt_rule_profile_with_workspace_symbols".to_string(),
        rule_source_revision: HOI4_CWT_CONFIG_REVISION.to_string(),
        runtime_disk_entities: false,
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
        allowed_effects: strings(profile.allowed_effects),
        allowed_triggers: strings(profile.allowed_triggers),
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

fn index_project(
    runtime: &Arc<RhoiScribeRuntime>,
    query: IndexQuery<'_>,
) -> Result<ProjectIndexResult, ToolError> {
    let roots = resolve_scan_roots(runtime, &query)?;
    project_index::index_hoi4_project(ProjectIndexRequest {
        roots,
        include_game_roots: query.include_game_roots,
    })
    .map_err(ToolError::InvalidRequest)
}

fn resolve_scan_roots(
    runtime: &Arc<RhoiScribeRuntime>,
    query: &IndexQuery<'_>,
) -> Result<Vec<ScanRoot>, ToolError> {
    if !query.roots.is_empty() {
        return Ok(query.roots.to_vec());
    }
    if let Some(workspace_root) = query.workspace_root {
        return Ok(vec![ScanRoot {
            path: workspace_root.to_string(),
            role: Some("mod".to_string()),
        }]);
    }
    if let Some(handle_id) = query.handle_id {
        let workspace_root = workspace_root_from_handle(runtime, handle_id)?;
        return Ok(vec![ScanRoot {
            path: path_to_string(&workspace_root),
            role: Some("mod".to_string()),
        }]);
    }

    Err(ToolError::InvalidRequest(
        "handle_id, workspace_root, or roots is required for workspace language queries"
            .to_string(),
    ))
}

fn workspace_root_from_handle(
    runtime: &Arc<RhoiScribeRuntime>,
    handle_id: &str,
) -> Result<PathBuf, ToolError> {
    let handle = runtime
        .cwt_language()
        .get_workspace(handle_id)
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
        .ok_or_else(|| ToolError::InvalidRequest(format!("unknown CWT workspace `{handle_id}`")))?;

    if handle
        .snapshot()
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
        .is_none()
    {
        handle
            .refresh_blocking()
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?;
    }

    handle
        .snapshot()
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
        .map(|snapshot| snapshot.workspace_root.clone())
        .ok_or_else(|| ToolError::InvalidRequest("CWT workspace has no warm snapshot".to_string()))
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

fn push_completion_if_matches(
    suggestions: &mut Vec<Hoi4CompletionSuggestion>,
    seen: &mut BTreeSet<String>,
    candidate: CompletionCandidate<'_>,
    prefix: &str,
) {
    if !prefix.is_empty()
        && !candidate
            .label
            .to_ascii_lowercase()
            .starts_with(&prefix.to_ascii_lowercase())
    {
        return;
    }
    if !seen.insert(candidate.label.to_string()) {
        return;
    }

    suggestions.push(Hoi4CompletionSuggestion {
        label: candidate.label.to_string(),
        insert_text: candidate.insert_text.to_string(),
        kind: candidate.kind.to_string(),
        detail: candidate.detail.to_string(),
        source: candidate.source.to_string(),
        confidence: candidate.confidence.to_string(),
    });
}

fn rule_profile_for_path(path: &str) -> RuleProfile {
    let normalized = normalize_path(path);
    if normalized.starts_with("events/") {
        return RuleProfile {
            rule_name: "events",
            type_name: "event",
            path_kind: "event script",
            scope_context: "country/event scope",
            source_hint: "events.cwt",
            allowed_effects: vec![
                "add_political_power",
                "country_event",
                "news_event",
                "set_country_flag",
            ],
            allowed_triggers: vec!["has_country_flag", "tag", "exists"],
            completions: completion_profiles(&[
                ("add_namespace", "keyword", "event namespace declaration"),
                ("country_event", "block", "country event definition"),
                ("news_event", "block", "news event definition"),
                ("state_event", "block", "state event definition"),
                ("unit_event", "block", "unit event definition"),
                ("id", "property", "event id"),
                ("title", "property", "event title localisation key"),
                ("desc", "property", "event description localisation key"),
                ("picture", "property", "event picture"),
                ("is_triggered_only", "property", "event trigger mode"),
                ("trigger", "block", "event trigger block"),
                ("immediate", "block", "event immediate effect block"),
                ("option", "block", "event option block"),
                ("name", "property", "option localisation key"),
                ("ai_chance", "block", "option AI chance block"),
            ]),
            confidence: "medium",
        };
    }
    if normalized.starts_with("common/national_focus/") {
        return RuleProfile {
            rule_name: "national_focus",
            type_name: "focus_tree",
            path_kind: "national focus script",
            scope_context: "country/focus scope",
            source_hint: "national_focus.cwt",
            allowed_effects: vec!["add_political_power", "add_stability", "add_war_support"],
            allowed_triggers: vec!["has_completed_focus", "has_country_flag", "tag"],
            completions: completion_profiles(&[
                ("focus_tree", "block", "focus tree definition"),
                ("focus", "block", "focus definition"),
                ("id", "property", "focus id"),
                ("icon", "property", "focus icon sprite"),
                ("x", "property", "focus x position"),
                ("y", "property", "focus y position"),
                ("cost", "property", "focus cost"),
                ("prerequisite", "block", "focus prerequisite"),
                ("mutually_exclusive", "block", "mutually exclusive focus"),
                ("available", "block", "focus availability trigger"),
                ("completion_reward", "block", "focus reward effect"),
                ("ai_will_do", "block", "AI weighting"),
            ]),
            confidence: "medium",
        };
    }
    if normalized.starts_with("common/scripted_effects/") {
        return RuleProfile {
            rule_name: "scripted_effects",
            type_name: "scripted_effect",
            path_kind: "scripted effect script",
            scope_context: "effect scope",
            source_hint: "effects.cwt",
            allowed_effects: vec!["add_political_power", "set_country_flag", "hidden_effect"],
            allowed_triggers: vec!["has_country_flag", "always"],
            completions: completion_profiles(&[
                (
                    "add_political_power",
                    "effect",
                    "country political power effect",
                ),
                ("set_country_flag", "effect", "set country flag effect"),
                ("hidden_effect", "block", "hidden effect block"),
                ("if", "block", "conditional effect"),
                ("limit", "block", "condition block inside an effect"),
            ]),
            confidence: "medium",
        };
    }
    if normalized.starts_with("common/on_actions/") {
        return RuleProfile {
            rule_name: "on_actions",
            type_name: "on_action",
            path_kind: "on_action script",
            scope_context: "on_action effect scope",
            source_hint: "on_actions.cwt",
            allowed_effects: vec!["country_event", "news_event", "random_events"],
            allowed_triggers: vec!["always", "has_country_flag"],
            completions: completion_profiles(&[
                ("on_startup", "block", "startup on_action"),
                ("effect", "block", "effect payload"),
                ("country_event", "block", "fire country event"),
                ("news_event", "block", "fire news event"),
                ("random_events", "block", "weighted event list"),
            ]),
            confidence: "medium",
        };
    }
    if normalized.starts_with("localisation/") {
        return RuleProfile {
            rule_name: "localisation",
            type_name: "localisation_key",
            path_kind: "localisation",
            scope_context: "localisation key/value table",
            source_hint: "localisation.cwt",
            allowed_effects: Vec::new(),
            allowed_triggers: Vec::new(),
            completions: completion_profiles(&[
                (
                    "l_english:",
                    "keyword",
                    "English localisation language header",
                ),
                (":0", "property", "localisation version suffix"),
            ]),
            confidence: "medium",
        };
    }
    if normalized.starts_with("interface/") || normalized.starts_with("gfx/") {
        return RuleProfile {
            rule_name: "interface",
            type_name: "gui_or_gfx",
            path_kind: "interface asset",
            scope_context: "GUI/GFX asset scope",
            source_hint: "interface.cwt",
            allowed_effects: Vec::new(),
            allowed_triggers: Vec::new(),
            completions: completion_profiles(&[
                ("spriteType", "block", "sprite definition"),
                ("name", "property", "GUI or sprite name"),
                ("texturefile", "property", "sprite texture path"),
                ("quadTextureSprite", "property", "GUI sprite reference"),
            ]),
            confidence: "medium",
        };
    }

    RuleProfile {
        rule_name: "generic_hoi4_script",
        type_name: "script",
        path_kind: "HOI4 script",
        scope_context: "unknown or generic script scope",
        source_hint: "settings.cwt",
        allowed_effects: vec!["add_political_power", "set_country_flag"],
        allowed_triggers: vec!["always", "has_country_flag"],
        completions: completion_profiles(&[
            ("if", "block", "conditional block"),
            ("limit", "block", "condition block"),
            (
                "add_political_power",
                "effect",
                "country political power effect",
            ),
            ("set_country_flag", "effect", "set country flag effect"),
        ]),
        confidence: "low",
    }
}

fn completion_profiles(
    items: &[(&'static str, &'static str, &'static str)],
) -> Vec<CompletionProfile> {
    items
        .iter()
        .map(|(label, kind, detail)| CompletionProfile {
            label,
            kind,
            detail,
        })
        .collect()
}

fn embedded_source_path(source_hint: &str) -> Option<String> {
    read_embedded_hoi4_cwt_sources()
        .ok()?
        .into_iter()
        .find(|source| source.path.ends_with(source_hint) || source.path.contains(source_hint))
        .map(|source| source.path)
}

struct DiagnosticExplanation {
    severity: &'static str,
    meaning: &'static str,
    repair_guidance: &'static str,
    source: &'static str,
    confidence: &'static str,
}

fn diagnostic_explanation(code: Option<&str>, message: Option<&str>) -> DiagnosticExplanation {
    match code {
        Some("CW263") => DiagnosticExplanation {
            severity: "error",
            meaning: "CWT schema validation found an unexpected field or key for the rule that applies at this path.",
            repair_guidance: "Compare the field against the CWT type rule with inspect_hoi4_type_rule, move it to an allowed block, rename it to a valid HOI4 key, or remove it if it is not supported.",
            source: "cwt_diagnostic_code",
            confidence: "high",
        },
        Some("CW262") => DiagnosticExplanation {
            severity: "error",
            meaning: "CWT schema validation found a value shape or type that does not match the rule for this key.",
            repair_guidance: "Inspect the type rule, then change the value to the expected scalar, enum, block, scope, or reference form.",
            source: "cwt_diagnostic_code",
            confidence: "high",
        },
        Some("CW242") => DiagnosticExplanation {
            severity: "warning",
            meaning: "CWT validation reported a missing or unresolved required reference or localisation-style value.",
            repair_guidance: "Use find_hoi4_definition or list_hoi4_workspace_symbols to confirm the referenced identifier exists, then add the missing definition or correct the key.",
            source: "cwt_diagnostic_code",
            confidence: "medium",
        },
        Some("cwt_parse_error") => DiagnosticExplanation {
            severity: "error",
            meaning: "The Paradox script parser could not build an AST, so schema validation cannot run for this content.",
            repair_guidance: "Fix unmatched braces, missing equals signs, malformed quoted strings, or other script syntax first, then rerun validate_hoi4_file.",
            source: "rhoiscribe_cwt_mapping",
            confidence: "high",
        },
        _ if message
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("unexpected") =>
        {
            DiagnosticExplanation {
                severity: "error",
                meaning: "The diagnostic text indicates the key or value is outside the CWT rule for this context.",
                repair_guidance: "Inspect the applicable type rule and scope, then adjust the key or move it under a valid parent block.",
                source: "diagnostic_message_heuristic",
                confidence: "medium",
            }
        }
        _ => DiagnosticExplanation {
            severity: "warning",
            meaning: "The diagnostic came from CWT or RHoiScribe language analysis, but this code does not yet have a specialized explanation.",
            repair_guidance: "Use validate_hoi4_file together with inspect_hoi4_type_rule and inspect_hoi4_scope to identify the expected structure.",
            source: "generic_cwt_diagnostic_mapping",
            confidence: "low",
        },
    }
}

fn normalized_code(code: Option<&str>, message: Option<&str>) -> Option<String> {
    code.filter(|code| !code.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            let message = message?;
            message
                .split_whitespace()
                .find(|part| {
                    part.starts_with("CW") && part[2..].chars().all(|c| c.is_ascii_digit())
                })
                .map(str::to_string)
        })
}

fn bounded_limit(limit: Option<usize>, default: usize) -> usize {
    limit.unwrap_or(default).clamp(1, default)
}

fn strings(values: Vec<&str>) -> Vec<String> {
    values.into_iter().map(str::to_string).collect()
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
