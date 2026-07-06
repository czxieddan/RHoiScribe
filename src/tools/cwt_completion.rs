//------------------------------------------------------------------------------------
// cwt_completion.rs -- Part of RHoiScribe
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

use crate::{RhoiScribeRuntime, cwt::rules::HOI4_CWT_CONFIG_REVISION};

use super::{
    ProjectIndexItem, ScanRoot, ToolError,
    cwt_common::{bounded_limit, normalize_path},
    cwt_indexing::{CwtIndexQuery, index_project},
    cwt_profiles::{RuleProfile, rule_profile_for_path},
};

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
pub struct Hoi4CompletionSuggestion {
    pub label: String,
    pub insert_text: String,
    pub kind: String,
    pub detail: String,
    pub source: String,
    pub confidence: String,
}

struct CompletionCandidate<'a> {
    label: &'a str,
    insert_text: &'a str,
    kind: &'a str,
    detail: &'a str,
    source: &'a str,
    confidence: &'a str,
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

    add_profile_completions(&profile, prefix, &mut suggestions, &mut seen);
    add_workspace_completions(
        &runtime,
        &request,
        prefix,
        limit,
        &mut suggestions,
        &mut seen,
    )?;
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

fn add_profile_completions(
    profile: &RuleProfile,
    prefix: &str,
    suggestions: &mut Vec<Hoi4CompletionSuggestion>,
    seen: &mut BTreeSet<String>,
) {
    for completion in profile.completions.iter() {
        push_completion_if_matches(
            suggestions,
            seen,
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
}

fn add_workspace_completions(
    runtime: &Arc<RhoiScribeRuntime>,
    request: &SuggestHoi4CompletionRequest,
    prefix: &str,
    limit: usize,
    suggestions: &mut Vec<Hoi4CompletionSuggestion>,
    seen: &mut BTreeSet<String>,
) -> Result<(), ToolError> {
    if suggestions.len() >= limit || !has_workspace_completion_source(request) {
        return Ok(());
    }

    let index = index_project(runtime, completion_index_query(request))?;
    for item in index.definitions.iter() {
        push_workspace_completion(suggestions, seen, item, prefix);
        if suggestions.len() >= limit {
            break;
        }
    }
    Ok(())
}

fn has_workspace_completion_source(request: &SuggestHoi4CompletionRequest) -> bool {
    request.handle_id.is_some() || request.workspace_root.is_some() || !request.roots.is_empty()
}

fn completion_index_query(request: &SuggestHoi4CompletionRequest) -> CwtIndexQuery<'_> {
    CwtIndexQuery {
        handle_id: request.handle_id.as_deref(),
        workspace_root: request.workspace_root.as_deref(),
        roots: &request.roots,
        include_game_roots: request.include_game_roots,
        missing_roots_message: "handle_id, workspace_root, or roots is required for workspace language queries",
    }
}

fn push_workspace_completion(
    suggestions: &mut Vec<Hoi4CompletionSuggestion>,
    seen: &mut BTreeSet<String>,
    item: &ProjectIndexItem,
    prefix: &str,
) {
    push_completion_if_matches(
        suggestions,
        seen,
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
}

fn push_completion_if_matches(
    suggestions: &mut Vec<Hoi4CompletionSuggestion>,
    seen: &mut BTreeSet<String>,
    candidate: CompletionCandidate<'_>,
    prefix: &str,
) {
    if !matches_prefix(candidate.label, prefix) {
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

fn matches_prefix(label: &str, prefix: &str) -> bool {
    prefix.is_empty()
        || label
            .to_ascii_lowercase()
            .starts_with(&prefix.to_ascii_lowercase())
}
