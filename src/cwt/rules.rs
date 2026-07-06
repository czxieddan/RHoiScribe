//------------------------------------------------------------------------------------
// rules.rs -- Part of RHoiScribe
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
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

use cwtools_parser::{ast::ParseError, parser::parse_string};
use cwtools_rules::{
    config_validation::validate_ruleset_references,
    post_process::post_process,
    rules_converter::ast_to_ruleset,
    rules_types::RuleSet,
    ruleset_loader::{RuleParseError, merge_ruleset},
};
use cwtools_string_table::string_table::StringTable;
use cwtools_validation::{ValidationError, validate_ast};

use super::hoi4_config::HOI4_CWT_CONFIG;

mod embedded_hoi4_cwt_config {
    include!(concat!(env!("OUT_DIR"), "/hoi4_cwt_config.rs"));
}

pub const HOI4_CWT_CONFIG_UPSTREAM_URL: &str = HOI4_CWT_CONFIG.upstream_url;
pub const HOI4_CWT_CONFIG_REVISION: &str = HOI4_CWT_CONFIG.revision;
pub const HOI4_CWT_CONFIG_LICENSE: &str = HOI4_CWT_CONFIG.license;
pub const HOI4_CWT_CONFIG_CONTENT_SHA256: &str =
    embedded_hoi4_cwt_config::HOI4_CWT_CONFIG_CONTENT_SHA256;
pub const HOI4_CWT_CONFIG_SOURCE_COUNT: usize =
    embedded_hoi4_cwt_config::HOI4_CWT_CONFIG_SOURCE_COUNT;
pub const HOI4_CWT_CONFIG_TOTAL_BYTES: usize =
    embedded_hoi4_cwt_config::HOI4_CWT_CONFIG_TOTAL_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualCwtSource<'a> {
    pub path: &'a str,
    pub content: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedCwtSource {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CwtRuleDiagnostic {
    pub path: String,
    pub line: u32,
    pub column: u16,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CwtValidationDiagnostic {
    pub code: Option<String>,
    pub severity: String,
    pub path: String,
    pub line: u32,
    pub column: u16,
    pub message: String,
}

#[derive(Debug)]
pub enum CwtRuleLoadError {
    NoRuleSources {
        source: String,
    },
    ExternalRead {
        path: String,
        message: String,
    },
    EmbeddedArchive {
        message: String,
    },
    ScriptParse {
        path: String,
        line: u32,
        column: u16,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CwtRuleReloadFailure {
    pub generation: u64,
    pub message: String,
    pub kept_previous_good: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CwtRuleReloadReport {
    pub generation: u64,
    pub active_changed: bool,
    pub kept_previous_good: bool,
    pub active_source_count: Option<usize>,
    pub rule_diagnostic_count: Option<usize>,
    pub error: Option<String>,
}

pub struct LoadedCwtRules {
    source_count: usize,
    table: StringTable,
    ruleset: RuleSet,
    rule_diagnostics: Vec<CwtRuleDiagnostic>,
}

#[derive(Default)]
pub struct ReloadableCwtRules {
    generation: u64,
    active: Option<Arc<LoadedCwtRules>>,
    last_failure: Option<CwtRuleReloadFailure>,
}

impl OwnedCwtSource {
    fn as_virtual(&self) -> VirtualCwtSource<'_> {
        VirtualCwtSource {
            path: &self.path,
            content: &self.content,
        }
    }
}

pub fn hoi4_cwt_config_archive_url() -> String {
    HOI4_CWT_CONFIG.archive_url()
}

pub fn read_embedded_hoi4_cwt_sources() -> Result<Vec<OwnedCwtSource>, CwtRuleLoadError> {
    read_cwt_sources_from_embedded_archive()
}

pub fn load_embedded_hoi4_cwt_rules() -> Result<LoadedCwtRules, CwtRuleLoadError> {
    let sources = read_embedded_hoi4_cwt_sources()?;
    load_owned_cwt_rules(&sources)
}

pub fn load_owned_cwt_rules(
    sources: &[OwnedCwtSource],
) -> Result<LoadedCwtRules, CwtRuleLoadError> {
    if sources.is_empty() {
        return Err(CwtRuleLoadError::NoRuleSources {
            source: "owned CWT source list".to_string(),
        });
    }

    let sources = sources
        .iter()
        .map(OwnedCwtSource::as_virtual)
        .collect::<Vec<_>>();
    load_virtual_cwt_rules(&sources)
}

pub fn load_virtual_cwt_rules(
    sources: &[VirtualCwtSource<'_>],
) -> Result<LoadedCwtRules, CwtRuleLoadError> {
    if sources.is_empty() {
        return Err(CwtRuleLoadError::NoRuleSources {
            source: "virtual CWT source list".to_string(),
        });
    }

    let table = StringTable::new();
    let mut ruleset = RuleSet::new();
    let mut parsed_sources = Vec::new();
    let mut diagnostics = Vec::new();

    for source in sources {
        if source_file_name(source.path).eq_ignore_ascii_case("folders.cwt") {
            ruleset.folders.extend(parse_folders_list(source.content));
            continue;
        }

        match parse_string(source.content, &table) {
            Ok(parsed) => {
                merge_ruleset(&mut ruleset, ast_to_ruleset(&parsed, &table));
                parsed_sources.push((PathBuf::from(source.path), parsed));
            }
            Err(error) => diagnostics.push(parse_error_to_rule_diagnostic(source.path, error)),
        }
    }

    post_process(&mut ruleset);
    ruleset.reindex();
    diagnostics.extend(
        validate_ruleset_references(&parsed_sources, &ruleset, &table)
            .into_iter()
            .map(rule_parse_error_to_diagnostic),
    );

    Ok(LoadedCwtRules {
        source_count: sources.len(),
        table,
        ruleset,
        rule_diagnostics: diagnostics,
    })
}

pub fn read_external_cwt_sources(
    path: impl AsRef<Path>,
) -> Result<Vec<OwnedCwtSource>, CwtRuleLoadError> {
    let path = path.as_ref();
    let mut files = Vec::new();

    if path.is_file() {
        if is_cwt_file(path) {
            files.push(path.to_path_buf());
        }
    } else {
        collect_external_cwt_files(path, &mut files)?;
    }

    files.sort();
    if files.is_empty() {
        return Err(CwtRuleLoadError::NoRuleSources {
            source: path_to_string(path),
        });
    }

    files
        .into_iter()
        .map(|path| {
            fs::read_to_string(&path)
                .map(|content| OwnedCwtSource {
                    path: path_to_string(&path),
                    content,
                })
                .map_err(|error| CwtRuleLoadError::ExternalRead {
                    path: path_to_string(&path),
                    message: error.to_string(),
                })
        })
        .collect()
}

pub fn load_external_cwt_rules(path: impl AsRef<Path>) -> Result<LoadedCwtRules, CwtRuleLoadError> {
    let sources = read_external_cwt_sources(path)?;
    load_owned_cwt_rules(&sources)
}

impl LoadedCwtRules {
    pub fn source_count(&self) -> usize {
        self.source_count
    }

    pub fn rule_diagnostics(&self) -> &[CwtRuleDiagnostic] {
        &self.rule_diagnostics
    }

    pub fn validate_script(
        &self,
        path: &str,
        content: &str,
    ) -> Result<Vec<CwtValidationDiagnostic>, CwtRuleLoadError> {
        let parsed = parse_string(content, &self.table)
            .map_err(|error| parse_error_to_load_error(path, error))?;
        Ok(
            validate_ast(&parsed, &self.ruleset, &self.table, path, None, None, None)
                .into_iter()
                .map(validation_error_to_diagnostic)
                .collect(),
        )
    }
}

impl ReloadableCwtRules {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn active(&self) -> Option<Arc<LoadedCwtRules>> {
        self.active.clone()
    }

    pub fn last_failure(&self) -> Option<&CwtRuleReloadFailure> {
        self.last_failure.as_ref()
    }

    pub fn reload_embedded_hoi4_config(&mut self) -> CwtRuleReloadReport {
        self.apply_reload_result(load_embedded_hoi4_cwt_rules())
    }

    pub fn reload_virtual(&mut self, sources: &[VirtualCwtSource<'_>]) -> CwtRuleReloadReport {
        self.apply_reload_result(load_virtual_cwt_rules(sources))
    }

    pub fn reload_owned(&mut self, sources: &[OwnedCwtSource]) -> CwtRuleReloadReport {
        self.apply_reload_result(load_owned_cwt_rules(sources))
    }

    pub fn reload_external_path(&mut self, path: impl AsRef<Path>) -> CwtRuleReloadReport {
        self.apply_reload_result(load_external_cwt_rules(path))
    }

    fn apply_reload_result(
        &mut self,
        result: Result<LoadedCwtRules, CwtRuleLoadError>,
    ) -> CwtRuleReloadReport {
        self.generation += 1;

        match result {
            Ok(loaded) => {
                let source_count = loaded.source_count();
                let rule_diagnostic_count = loaded.rule_diagnostics().len();
                self.active = Some(Arc::new(loaded));
                self.last_failure = None;
                CwtRuleReloadReport {
                    generation: self.generation,
                    active_changed: true,
                    kept_previous_good: false,
                    active_source_count: Some(source_count),
                    rule_diagnostic_count: Some(rule_diagnostic_count),
                    error: None,
                }
            }
            Err(error) => {
                let kept_previous_good = self.active.is_some();
                let message = error.to_string();
                self.last_failure = Some(CwtRuleReloadFailure {
                    generation: self.generation,
                    message: message.clone(),
                    kept_previous_good,
                });
                CwtRuleReloadReport {
                    generation: self.generation,
                    active_changed: false,
                    kept_previous_good,
                    active_source_count: self.active.as_ref().map(|rules| rules.source_count()),
                    rule_diagnostic_count: self
                        .active
                        .as_ref()
                        .map(|rules| rules.rule_diagnostics().len()),
                    error: Some(message),
                }
            }
        }
    }
}

impl fmt::Display for CwtRuleLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CwtRuleLoadError::NoRuleSources { source } => {
                write!(formatter, "no CWT rule sources found in `{}`", source)
            }
            CwtRuleLoadError::ExternalRead { path, message } => {
                write!(
                    formatter,
                    "failed to read CWT source `{}`: {}",
                    path, message
                )
            }
            CwtRuleLoadError::EmbeddedArchive { message } => {
                write!(
                    formatter,
                    "failed to read embedded CWT rules archive in memory: {}",
                    message
                )
            }
            CwtRuleLoadError::ScriptParse {
                path,
                line,
                column,
                message,
            } => write!(
                formatter,
                "failed to parse CWT script `{}` at {}:{}: {}",
                path, line, column, message
            ),
        }
    }
}

impl Error for CwtRuleLoadError {}

fn read_cwt_sources_from_embedded_archive() -> Result<Vec<OwnedCwtSource>, CwtRuleLoadError> {
    let cursor = Cursor::new(embedded_hoi4_cwt_config::HOI4_CWT_CONFIG_ARCHIVE_BYTES);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|error| CwtRuleLoadError::EmbeddedArchive {
            message: error.to_string(),
        })?;
    let mut sources = Vec::new();

    for index in 0..archive.len() {
        let mut file =
            archive
                .by_index(index)
                .map_err(|error| CwtRuleLoadError::EmbeddedArchive {
                    message: error.to_string(),
                })?;
        if !file.is_file() {
            continue;
        }

        let entry_name = file.name().replace('\\', "/");
        let Some(relative_path) = github_archive_config_path(&entry_name) else {
            continue;
        };
        if !relative_path.ends_with(".cwt") {
            continue;
        }

        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|error| CwtRuleLoadError::EmbeddedArchive {
                message: format!("{}: {}", entry_name, error),
            })?;

        sources.push(OwnedCwtSource {
            path: HOI4_CWT_CONFIG.virtual_path(relative_path),
            content,
        });
    }

    sources.sort_by(|left, right| left.path.cmp(&right.path));
    if sources.is_empty() {
        return Err(CwtRuleLoadError::NoRuleSources {
            source: hoi4_cwt_config_archive_url().to_string(),
        });
    }

    Ok(sources)
}

fn github_archive_config_path(path: &str) -> Option<&str> {
    path.strip_prefix("config/").or_else(|| {
        path.split_once("/config/")
            .map(|(_, relative_path)| relative_path)
    })
}

fn collect_external_cwt_files(
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), CwtRuleLoadError> {
    let entries = fs::read_dir(directory).map_err(|error| CwtRuleLoadError::ExternalRead {
        path: path_to_string(directory),
        message: error.to_string(),
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| CwtRuleLoadError::ExternalRead {
            path: path_to_string(directory),
            message: error.to_string(),
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| CwtRuleLoadError::ExternalRead {
                path: path_to_string(&path),
                message: error.to_string(),
            })?;

        if file_type.is_dir() {
            collect_external_cwt_files(&path, files)?;
        } else if file_type.is_file() && is_cwt_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}

fn is_cwt_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("cwt"))
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn parse_folders_list(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn source_file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn parse_error_to_rule_diagnostic(path: &str, error: ParseError) -> CwtRuleDiagnostic {
    let (line, column, message) = parse_error_parts(error);
    CwtRuleDiagnostic {
        path: path.to_string(),
        line,
        column,
        message,
    }
}

fn rule_parse_error_to_diagnostic(error: RuleParseError) -> CwtRuleDiagnostic {
    CwtRuleDiagnostic {
        path: error.file.to_string_lossy().into_owned(),
        line: error.line,
        column: error.col,
        message: error.message,
    }
}

fn parse_error_to_load_error(path: &str, error: ParseError) -> CwtRuleLoadError {
    let (line, column, message) = parse_error_parts(error);
    CwtRuleLoadError::ScriptParse {
        path: path.to_string(),
        line,
        column,
        message,
    }
}

fn parse_error_parts(error: ParseError) -> (u32, u16, String) {
    match error {
        ParseError::Pos(_, line, column, message) => (line, column, message),
        ParseError::General(message) => (1, 0, message),
    }
}

fn validation_error_to_diagnostic(error: ValidationError) -> CwtValidationDiagnostic {
    CwtValidationDiagnostic {
        code: error.code.map(str::to_string),
        severity: format!("{:?}", error.severity),
        path: error.file,
        line: error.line,
        column: error.col,
        message: error.message,
    }
}
