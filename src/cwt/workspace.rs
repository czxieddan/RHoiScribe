//------------------------------------------------------------------------------------
// workspace.rs -- Part of RHoiScribe
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
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    thread,
};

use super::hoi4_config::HOI4_CWT_CONFIG;
use super::rules::{
    CwtRuleDiagnostic, LoadedCwtRules, load_embedded_hoi4_cwt_rules, load_external_cwt_rules,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CwtWorkspaceConfig {
    pub workspace_root: PathBuf,
    pub rules_source: CwtRulesSource,
    pub vanilla_root: Option<PathBuf>,
    pub ignore_globs: Vec<String>,
    pub localisation_languages: Vec<String>,
    pub mode: CwtWorkspaceMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CwtRulesSource {
    EmbeddedRulesCrate,
    ExternalPath(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CwtWorkspaceMode {
    ModOnly,
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CwtWorkspaceWarmState {
    Cold,
    Warming,
    Warm,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CwtWorkspaceStatus {
    pub handle_id: String,
    pub generation: u64,
    pub state: CwtWorkspaceWarmState,
    pub indexed_file_count: usize,
    pub workspace_file_count: usize,
    pub vanilla_file_count: usize,
    pub validation_diagnostic_count: usize,
    pub rule_diagnostic_count: usize,
    pub stale: bool,
    pub last_error: Option<String>,
}

pub struct CwtWorkspaceHandle {
    id: String,
    config: CwtWorkspaceConfig,
    state: Arc<RwLock<CwtWorkspaceState>>,
}

pub struct CwtWorkspaceSnapshot {
    pub generation: u64,
    pub workspace_root: PathBuf,
    pub rules: Arc<LoadedCwtRules>,
    pub files: Vec<CwtIndexedFile>,
    pub rule_diagnostics: Vec<CwtRuleDiagnostic>,
    pub validation_diagnostic_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CwtIndexedFile {
    pub path: String,
    pub absolute_path: PathBuf,
    pub root_role: String,
    pub byte_len: u64,
}

#[derive(Debug)]
pub enum CwtWorkspaceError {
    StateLockPoisoned,
    WorkspaceRead { path: String, message: String },
    Rules(String),
}

#[derive(Clone)]
struct CwtWorkspaceState {
    generation: u64,
    warm_state: CwtWorkspaceWarmState,
    snapshot: Option<Arc<CwtWorkspaceSnapshot>>,
    stale: bool,
    last_error: Option<String>,
}

impl Default for CwtWorkspaceConfig {
    fn default() -> Self {
        Self {
            workspace_root: PathBuf::new(),
            rules_source: CwtRulesSource::EmbeddedRulesCrate,
            vanilla_root: None,
            ignore_globs: Vec::new(),
            localisation_languages: vec!["english".to_string()],
            mode: CwtWorkspaceMode::ModOnly,
        }
    }
}

impl CwtWorkspaceHandle {
    pub fn new(id: String, config: CwtWorkspaceConfig) -> Self {
        Self {
            id,
            config,
            state: Arc::new(RwLock::new(CwtWorkspaceState::cold())),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn config(&self) -> &CwtWorkspaceConfig {
        &self.config
    }

    pub fn status(&self) -> Result<CwtWorkspaceStatus, CwtWorkspaceError> {
        let state = self
            .state
            .read()
            .map_err(|_| CwtWorkspaceError::StateLockPoisoned)?;
        Ok(state.status(&self.id))
    }

    pub fn snapshot(&self) -> Result<Option<Arc<CwtWorkspaceSnapshot>>, CwtWorkspaceError> {
        let state = self
            .state
            .read()
            .map_err(|_| CwtWorkspaceError::StateLockPoisoned)?;
        Ok(state.snapshot.clone())
    }

    pub fn refresh(&self) -> Result<u64, CwtWorkspaceError> {
        let generation = self.prepare_refresh()?;
        let config = self.config.clone();
        let state = Arc::clone(&self.state);

        thread::spawn(move || {
            let result = build_workspace_snapshot(generation, &config);
            apply_warm_result(&state, generation, result);
        });

        Ok(generation)
    }

    pub fn refresh_blocking(&self) -> Result<u64, CwtWorkspaceError> {
        let generation = self.prepare_refresh()?;
        let result = build_workspace_snapshot(generation, &self.config);
        apply_warm_result(&self.state, generation, result);
        Ok(generation)
    }

    fn prepare_refresh(&self) -> Result<u64, CwtWorkspaceError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| CwtWorkspaceError::StateLockPoisoned)?;
        state.generation += 1;
        state.warm_state = CwtWorkspaceWarmState::Warming;
        state.stale = state.snapshot.is_some();
        state.last_error = None;
        Ok(state.generation)
    }
}

impl CwtWorkspaceSnapshot {
    fn status(&self, handle_id: &str, state: &CwtWorkspaceState) -> CwtWorkspaceStatus {
        CwtWorkspaceStatus {
            handle_id: handle_id.to_string(),
            generation: state.generation,
            state: state.warm_state.clone(),
            indexed_file_count: self.files.len(),
            workspace_file_count: count_root_role(&self.files, "mod"),
            vanilla_file_count: count_root_role(&self.files, "game"),
            validation_diagnostic_count: self.validation_diagnostic_count,
            rule_diagnostic_count: self.rule_diagnostics.len(),
            stale: state.stale,
            last_error: state.last_error.clone(),
        }
    }
}

impl CwtWorkspaceState {
    fn cold() -> Self {
        Self {
            generation: 0,
            warm_state: CwtWorkspaceWarmState::Cold,
            snapshot: None,
            stale: false,
            last_error: None,
        }
    }

    fn status(&self, handle_id: &str) -> CwtWorkspaceStatus {
        match &self.snapshot {
            Some(snapshot) => snapshot.status(handle_id, self),
            None => CwtWorkspaceStatus {
                handle_id: handle_id.to_string(),
                generation: self.generation,
                state: self.warm_state.clone(),
                indexed_file_count: 0,
                workspace_file_count: 0,
                vanilla_file_count: 0,
                validation_diagnostic_count: 0,
                rule_diagnostic_count: 0,
                stale: self.stale,
                last_error: self.last_error.clone(),
            },
        }
    }
}

impl fmt::Display for CwtWorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CwtWorkspaceError::StateLockPoisoned => {
                write!(formatter, "CWT workspace state lock is poisoned")
            }
            CwtWorkspaceError::WorkspaceRead { path, message } => {
                write!(
                    formatter,
                    "failed to read workspace path `{}`: {}",
                    path, message
                )
            }
            CwtWorkspaceError::Rules(message) => {
                write!(formatter, "failed to load CWT rules: {}", message)
            }
        }
    }
}

impl Error for CwtWorkspaceError {}

pub fn workspace_handle_id(config: &CwtWorkspaceConfig) -> String {
    let mut parts = vec![
        path_to_string(&config.workspace_root),
        rules_source_id(&config.rules_source),
        config
            .vanilla_root
            .as_deref()
            .map(path_to_string)
            .unwrap_or_else(|| "no-vanilla".to_string()),
        format!("{:?}", config.mode),
    ];
    parts.extend(config.ignore_globs.iter().cloned());
    parts.extend(config.localisation_languages.iter().cloned());
    sanitize_handle_id(&parts.join("|"))
}

fn build_workspace_snapshot(
    generation: u64,
    config: &CwtWorkspaceConfig,
) -> Result<CwtWorkspaceSnapshot, CwtWorkspaceError> {
    let rules = match &config.rules_source {
        CwtRulesSource::EmbeddedRulesCrate => load_embedded_hoi4_cwt_rules(),
        CwtRulesSource::ExternalPath(path) => load_external_cwt_rules(path),
    }
    .map_err(|error| CwtWorkspaceError::Rules(error.to_string()))?;
    let rule_diagnostics = rules.rule_diagnostics().to_vec();
    let rules = Arc::new(rules);
    let files = discover_workspace_files(config)?;
    let validation_diagnostic_count = count_validation_diagnostics(&rules, &files);

    Ok(CwtWorkspaceSnapshot {
        generation,
        workspace_root: config.workspace_root.clone(),
        rules,
        files,
        rule_diagnostics,
        validation_diagnostic_count,
    })
}

fn apply_warm_result(
    state: &Arc<RwLock<CwtWorkspaceState>>,
    generation: u64,
    result: Result<CwtWorkspaceSnapshot, CwtWorkspaceError>,
) {
    let Ok(mut state) = state.write() else {
        return;
    };
    if state.generation != generation {
        return;
    }

    match result {
        Ok(snapshot) => {
            state.snapshot = Some(Arc::new(snapshot));
            state.warm_state = CwtWorkspaceWarmState::Warm;
            state.stale = false;
            state.last_error = None;
        }
        Err(error) => {
            state.warm_state = CwtWorkspaceWarmState::Failed;
            state.stale = state.snapshot.is_some();
            state.last_error = Some(error.to_string());
        }
    }
}

fn discover_workspace_files(
    config: &CwtWorkspaceConfig,
) -> Result<Vec<CwtIndexedFile>, CwtWorkspaceError> {
    let mut files = Vec::new();
    collect_workspace_files(
        &config.workspace_root,
        &config.workspace_root,
        "mod",
        config,
        &mut files,
    )?;
    if config.mode == CwtWorkspaceMode::Full
        && let Some(vanilla_root) = &config.vanilla_root
    {
        collect_workspace_files(vanilla_root, vanilla_root, "game", config, &mut files)?;
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_workspace_files(
    root: &Path,
    directory: &Path,
    root_role: &str,
    config: &CwtWorkspaceConfig,
    files: &mut Vec<CwtIndexedFile>,
) -> Result<(), CwtWorkspaceError> {
    let entries = fs::read_dir(directory).map_err(|error| CwtWorkspaceError::WorkspaceRead {
        path: path_to_string(directory),
        message: error.to_string(),
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| CwtWorkspaceError::WorkspaceRead {
            path: path_to_string(directory),
            message: error.to_string(),
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| CwtWorkspaceError::WorkspaceRead {
                path: path_to_string(&path),
                message: error.to_string(),
            })?;

        if file_type.is_dir() {
            if !is_ignored(root, &path, config) {
                collect_workspace_files(root, &path, root_role, config, files)?;
            }
        } else if file_type.is_file() && is_language_file(&path) {
            let relative_path = relative_path(root, &path);
            if !is_ignored_path(&relative_path, &config.ignore_globs) {
                let metadata =
                    fs::metadata(&path).map_err(|error| CwtWorkspaceError::WorkspaceRead {
                        path: path_to_string(&path),
                        message: error.to_string(),
                    })?;
                files.push(CwtIndexedFile {
                    path: relative_path,
                    absolute_path: path,
                    root_role: root_role.to_string(),
                    byte_len: metadata.len(),
                });
            }
        }
    }

    Ok(())
}

fn count_validation_diagnostics(rules: &LoadedCwtRules, files: &[CwtIndexedFile]) -> usize {
    files
        .iter()
        .filter(|file| file.root_role == "mod" && is_script_path(&file.path))
        .filter_map(|file| {
            fs::read_to_string(&file.absolute_path)
                .ok()
                .map(|content| (file, content))
        })
        .map(|(file, content)| {
            rules
                .validate_script(&file.path, &content)
                .map(|diagnostics| diagnostics.len())
                .unwrap_or(1)
        })
        .sum()
}

fn is_language_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "txt" | "gui" | "gfx" | "sfx" | "asset" | "map" | "yml" | "yaml" | "csv"
            )
        })
}

fn is_script_path(path: &str) -> bool {
    let extension = path.rsplit('.').next().unwrap_or_default();
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "txt" | "gui" | "gfx" | "sfx" | "asset" | "map"
    )
}

fn count_root_role(files: &[CwtIndexedFile], root_role: &str) -> usize {
    files
        .iter()
        .filter(|file| file.root_role == root_role)
        .count()
}

fn is_ignored(root: &Path, path: &Path, config: &CwtWorkspaceConfig) -> bool {
    is_ignored_path(&relative_path(root, path), &config.ignore_globs)
}

fn is_ignored_path(path: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|pattern| wildcard_match(pattern.trim(), path))
}

fn wildcard_match(pattern: &str, path: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    if pattern == "*" || pattern == "**" {
        return true;
    }
    if !pattern.contains('*') {
        return path == pattern || path.contains(pattern);
    }

    let mut remainder = path;
    for part in pattern.split('*').filter(|part| !part.is_empty()) {
        let Some(position) = remainder.find(part) else {
            return false;
        };
        remainder = &remainder[position + part.len()..];
    }
    true
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn rules_source_id(source: &CwtRulesSource) -> String {
    match source {
        CwtRulesSource::EmbeddedRulesCrate => HOI4_CWT_CONFIG.embedded_source_id(),
        CwtRulesSource::ExternalPath(path) => format!("external:{}", path_to_string(path)),
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn sanitize_handle_id(value: &str) -> String {
    let mut id = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            id.push(character.to_ascii_lowercase());
        } else {
            id.push('_');
        }
    }
    id
}
