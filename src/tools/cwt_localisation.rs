//------------------------------------------------------------------------------------
// cwt_localisation.rs -- Part of RHoiScribe
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
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
    RhoiScribeRuntime,
    cwt::rules::{
        HOI4_CWT_CONFIG_CONTENT_SHA256, HOI4_CWT_CONFIG_REVISION, LoadedCwtRules,
        load_embedded_hoi4_cwt_rules,
    },
};

use super::{
    GeneratedFile, LocalisationBatchRequest, LocalisationEntry, ProjectIndexItem, ScanRoot,
    ToolEngine, ToolError,
    paradox_lexer::{Token, TokenKind, tokenize},
    project_index::{self, IndexedFile, ProjectIndexRequest, ProjectIndexResult},
};

const CWT_LOCALISATION_TOOL_NAMES: &[&str] = &["generate_missing_localisation"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerateMissingLocalisationRequest {
    pub handle_id: Option<String>,
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub roots: Vec<ScanRoot>,
    pub include_game_roots: Option<bool>,
    pub language: Option<String>,
    pub file_stem: Option<String>,
    pub limit: Option<usize>,
    pub dry_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerateMissingLocalisationResult {
    pub dry_run: bool,
    pub language: String,
    pub file_stem: String,
    pub candidates: Vec<MissingLocalisationCandidate>,
    pub files: Vec<GeneratedFile>,
    pub existing_key_count: usize,
    pub cwt_diagnostic_count: usize,
    pub source: String,
    pub rule_source_revision: String,
    pub rule_content_sha256: String,
    pub runtime_disk_entities: bool,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissingLocalisationCandidate {
    pub key: String,
    pub value: String,
    pub path: String,
    pub line: usize,
    pub reference_key: String,
    pub source: String,
    pub confidence: String,
}

struct IndexQuery<'a> {
    handle_id: Option<&'a str>,
    workspace_root: Option<&'a str>,
    roots: &'a [ScanRoot],
    include_game_roots: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalisationReference {
    key: String,
    reference_key: String,
    path: String,
    root: String,
    line: usize,
}

pub fn is_cwt_localisation_tool(name: &str) -> bool {
    CWT_LOCALISATION_TOOL_NAMES.contains(&name)
}

pub fn generate_missing_localisation(
    runtime: Arc<RhoiScribeRuntime>,
    request: GenerateMissingLocalisationRequest,
) -> Result<GenerateMissingLocalisationResult, ToolError> {
    if request.dry_run == Some(false) {
        return Err(ToolError::InvalidRequest(
            "generate_missing_localisation is dry-run only; call generate_localisation_batch with the returned entries when writing is approved".to_string(),
        ));
    }

    let language = request.language.unwrap_or_else(|| "english".to_string());
    let file_stem = request
        .file_stem
        .unwrap_or_else(|| "missing_localisation".to_string());
    let limit = request.limit.unwrap_or(200).clamp(1, 1000);
    let index = index_project(
        &runtime,
        IndexQuery {
            handle_id: request.handle_id.as_deref(),
            workspace_root: request.workspace_root.as_deref(),
            roots: &request.roots,
            include_game_roots: request.include_game_roots,
        },
    )?;
    let rules = rules_for_query(&runtime, request.handle_id.as_deref())?;
    let existing_keys = existing_localisation_keys(&index.definitions);
    let cwt_lines = cwt_localisation_diagnostic_lines(&index.files, rules.as_deref());
    let references = collect_missing_references(&index.files, &existing_keys)?;
    let candidates = references
        .into_values()
        .take(limit)
        .map(|reference| candidate_from_reference(reference, &cwt_lines))
        .collect::<Vec<_>>();
    let entries = candidates
        .iter()
        .map(|candidate| LocalisationEntry {
            key: candidate.key.clone(),
            value: candidate.value.clone(),
        })
        .collect::<Vec<_>>();
    let generated = ToolEngine::generate_localisation_batch(LocalisationBatchRequest {
        language: language.clone(),
        file_stem: file_stem.clone(),
        key_prefix: None,
        entries,
        dry_run: true,
        output_root: None,
    })?;

    Ok(GenerateMissingLocalisationResult {
        dry_run: true,
        language,
        file_stem,
        candidates,
        files: generated.files,
        existing_key_count: existing_keys.len(),
        cwt_diagnostic_count: cwt_lines.len(),
        source: "cwt_diagnostics_with_rhoiscribe_loc_index".to_string(),
        rule_source_revision: HOI4_CWT_CONFIG_REVISION.to_string(),
        rule_content_sha256: HOI4_CWT_CONFIG_CONTENT_SHA256.to_string(),
        runtime_disk_entities: false,
        messages: vec![
            "review candidates before writing; this tool never writes localisation files"
                .to_string(),
            "use generate_localisation_batch with returned entries when write approval is explicit"
                .to_string(),
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
        "handle_id, workspace_root, or roots is required for localisation generation".to_string(),
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

fn rules_for_query(
    runtime: &Arc<RhoiScribeRuntime>,
    handle_id: Option<&str>,
) -> Result<Option<Arc<LoadedCwtRules>>, ToolError> {
    if let Some(handle_id) = handle_id {
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
            return Ok(Some(Arc::clone(&snapshot.rules)));
        }
    }

    load_embedded_hoi4_cwt_rules()
        .map(Arc::new)
        .map(Some)
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))
}

fn existing_localisation_keys(definitions: &[ProjectIndexItem]) -> BTreeSet<String> {
    definitions
        .iter()
        .filter(|definition| definition.kind == "localisation_key")
        .map(|definition| definition.name.clone())
        .collect()
}

fn collect_missing_references(
    files: &[IndexedFile],
    existing_keys: &BTreeSet<String>,
) -> Result<BTreeMap<String, LocalisationReference>, ToolError> {
    let mut references = BTreeMap::new();

    for file in files
        .iter()
        .filter(|file| is_script_with_localisation_refs(&file.path))
    {
        let path = join_relative_path(Path::new(&file.root), &file.path);
        let content = fs::read_to_string(&path).map_err(|error| {
            ToolError::InvalidRequest(format!(
                "failed to read localisation reference file `{}`: {}",
                path_to_string(&path),
                error
            ))
        })?;
        let is_txt = extension_is(&file.path, "txt");
        for (reference_key, key, line) in localisation_references(&content, is_txt) {
            if existing_keys.contains(&key) || is_inline_or_builtin_loc_value(&key) {
                continue;
            }
            references
                .entry(key.clone())
                .or_insert_with(|| LocalisationReference {
                    key,
                    reference_key,
                    path: normalize_path(&file.path),
                    root: normalize_path(&file.root),
                    line,
                });
        }
    }

    Ok(references)
}

fn cwt_localisation_diagnostic_lines(
    files: &[IndexedFile],
    rules: Option<&LoadedCwtRules>,
) -> BTreeSet<(String, usize)> {
    let Some(rules) = rules else {
        return BTreeSet::new();
    };
    let mut lines = BTreeSet::new();

    for file in files.iter().filter(|file| is_cwt_script_path(&file.path)) {
        let path = join_relative_path(Path::new(&file.root), &file.path);
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(diagnostics) = rules.validate_script(&file.path, &content) else {
            continue;
        };
        for diagnostic in diagnostics {
            if is_localisation_diagnostic(diagnostic.code.as_deref(), &diagnostic.message) {
                lines.insert((normalize_path(&file.path), diagnostic.line as usize));
            }
        }
    }

    lines
}

fn candidate_from_reference(
    reference: LocalisationReference,
    cwt_lines: &BTreeSet<(String, usize)>,
) -> MissingLocalisationCandidate {
    let has_cwt_diagnostic = cwt_lines.contains(&(reference.path.clone(), reference.line));
    MissingLocalisationCandidate {
        value: suggested_value(&reference.key, &reference.reference_key),
        key: reference.key,
        path: reference.path,
        line: reference.line,
        reference_key: reference.reference_key,
        source: if has_cwt_diagnostic {
            "cwt_diagnostic+rhoiscribe_loc_index"
        } else {
            "rhoiscribe_loc_index"
        }
        .to_string(),
        confidence: if has_cwt_diagnostic { "high" } else { "medium" }.to_string(),
    }
}

fn localisation_references(content: &str, is_txt: bool) -> Vec<(String, String, usize)> {
    let tokens = tokenize(content);
    let mut references = Vec::new();

    for window in tokens.windows(3) {
        if is_localisation_reference_window(window, is_txt) {
            references.push((
                window[0].text.clone(),
                unquote(&window[2].text).to_string(),
                window[0].line,
            ));
        }
    }

    references
}

fn is_localisation_reference_window(window: &[Token], is_txt: bool) -> bool {
    window[1].kind == TokenKind::Equals
        && matches!(window[2].kind, TokenKind::Word | TokenKind::String)
        && is_localisation_reference_key(&window[0].text, is_txt)
}

fn is_localisation_reference_key(key: &str, is_txt: bool) -> bool {
    match key {
        "title"
        | "desc"
        | "description"
        | "custom_effect_tooltip"
        | "custom_trigger_tooltip"
        | "tooltip"
        | "delayed_event_text"
        | "major"
        | "minor" => true,
        "name" => is_txt,
        _ => false,
    }
}

fn is_script_with_localisation_refs(relative_path: &str) -> bool {
    let normalized = normalize_path(relative_path);
    if normalized.starts_with("localisation/") {
        return false;
    }

    extension_is(&normalized, "txt")
        || extension_is(&normalized, "gui")
        || extension_is(&normalized, "gfx")
}

fn is_cwt_script_path(path: &str) -> bool {
    let extension = path.rsplit('.').next().unwrap_or_default();
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "txt" | "gui" | "gfx" | "sfx" | "asset" | "map"
    )
}

fn extension_is(path: &str, expected: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn is_inline_or_builtin_loc_value(value: &str) -> bool {
    value.is_empty()
        || is_known_non_localisation_value(value)
        || is_numeric_value(value)
        || value.contains(' ')
}

fn is_known_non_localisation_value(value: &str) -> bool {
    value.starts_with("GFX_")
        || value.starts_with("generic_")
        || matches!(value, "yes" | "no" | "always" | "ROOT" | "FROM" | "THIS")
}

fn is_numeric_value(value: &str) -> bool {
    value
        .chars()
        .all(|character| character.is_ascii_digit() || matches!(character, '.' | '-' | '+' | '%'))
}

fn is_localisation_diagnostic(code: Option<&str>, message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    code == Some("CW242") || message.contains("localisation") || message.contains("localization")
}

fn suggested_value(key: &str, reference_key: &str) -> String {
    let base = key
        .rsplit_once('.')
        .map(|(_, suffix)| suffix)
        .unwrap_or(key)
        .replace(['_', '-'], " ");
    match reference_key {
        "desc" | "description" => format!("TODO: {} description", title_case(&base)),
        "name" => title_case(&base),
        "title" => title_case(&base),
        _ => format!("TODO: {}", title_case(&base)),
    }
}

fn title_case(value: &str) -> String {
    let title = value
        .split_whitespace()
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if title.is_empty() {
        "TODO".to_string()
    } else {
        title
    }
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn join_relative_path(root: &Path, path: &str) -> PathBuf {
    path.split('/')
        .filter(|part| !part.is_empty())
        .fold(root.to_path_buf(), |current, part| current.join(part))
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
