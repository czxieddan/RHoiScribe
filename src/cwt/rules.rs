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

use std::{error::Error, fmt, path::PathBuf};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualCwtSource<'a> {
    pub path: &'a str,
    pub content: &'a str,
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
    ScriptParse {
        path: String,
        line: u32,
        column: u16,
        message: String,
    },
}

pub struct LoadedCwtRules {
    source_count: usize,
    table: StringTable,
    ruleset: RuleSet,
    rule_diagnostics: Vec<CwtRuleDiagnostic>,
}

pub fn load_virtual_cwt_rules(
    sources: &[VirtualCwtSource<'_>],
) -> Result<LoadedCwtRules, CwtRuleLoadError> {
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

impl fmt::Display for CwtRuleLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
