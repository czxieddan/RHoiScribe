//------------------------------------------------------------------------------------
// cwt.rs -- Part of RHoiScribe
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

use std::fmt::Write as _;

use crate::cwt::{
    hoi4_config::HOI4_CWT_CONFIG,
    rules::{
        HOI4_CWT_CONFIG_CONTENT_SHA256, HOI4_CWT_CONFIG_SOURCE_COUNT, HOI4_CWT_CONFIG_TOTAL_BYTES,
    },
};

pub const CWT_CATALOG_URI: &str = "rhoiscribe://hoi4/cwt/catalog";
pub const CWT_METADATA_URI: &str = "rhoiscribe://hoi4/cwt/metadata";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CwtResourceCatalog {
    catalog_index: String,
    metadata: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CwtResourceEntry {
    pub(crate) uri: String,
    pub(crate) name: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) mime_type: &'static str,
    pub(crate) size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CwtResourceText {
    pub(crate) text: String,
    pub(crate) mime_type: &'static str,
}

enum TomlValue {
    String(String),
    Integer(usize),
    Bool(bool),
}

impl CwtResourceCatalog {
    pub fn load_embedded() -> Self {
        Self {
            catalog_index: catalog_index_toml(),
            metadata: metadata_markdown(),
        }
    }

    pub(crate) fn resource_entries(&self) -> Vec<CwtResourceEntry> {
        vec![
            CwtResourceEntry {
                uri: CWT_CATALOG_URI.to_string(),
                name: "hoi4_cwt_catalog".to_string(),
                title: "HOI4 CWT resource catalog".to_string(),
                description: "Pinned Cargo git dependency source for in-memory HOI4 CWT rules."
                    .to_string(),
                mime_type: "application/toml",
                size: self.catalog_index.len(),
            },
            CwtResourceEntry {
                uri: CWT_METADATA_URI.to_string(),
                name: "hoi4_cwt_metadata".to_string(),
                title: "HOI4 CWT source metadata".to_string(),
                description: "Traceability and runtime no-disk policy for HOI4 CWT config."
                    .to_string(),
                mime_type: "text/markdown",
                size: self.metadata.len(),
            },
        ]
    }

    pub(crate) fn read_text(&self, uri: &str) -> Option<CwtResourceText> {
        match uri {
            CWT_CATALOG_URI => Some(CwtResourceText {
                text: self.catalog_index.clone(),
                mime_type: "application/toml",
            }),
            CWT_METADATA_URI => Some(CwtResourceText {
                text: self.metadata.clone(),
                mime_type: "text/markdown",
            }),
            _ => None,
        }
    }
}

pub(crate) fn is_cwt_resource_uri(uri: &str) -> bool {
    uri == CWT_CATALOG_URI || uri == CWT_METADATA_URI
}

fn catalog_index_toml() -> String {
    let mut output = String::new();
    for (key, value) in catalog_index_entries() {
        write_toml_entry(&mut output, key, value);
    }
    output
}

fn catalog_index_entries() -> Vec<(&'static str, TomlValue)> {
    let source_slug = HOI4_CWT_CONFIG.source_slug();
    let repository_url = HOI4_CWT_CONFIG.repository_url();
    let upstream_url = HOI4_CWT_CONFIG.upstream_url();
    let git_url = HOI4_CWT_CONFIG.git_url();
    let embedded_source_id = HOI4_CWT_CONFIG.embedded_source_id();
    let virtual_source_prefix = HOI4_CWT_CONFIG.virtual_source_prefix();

    vec![
        string_entry("source_format", HOI4_CWT_CONFIG.source_format),
        string_entry("runtime_storage", HOI4_CWT_CONFIG.runtime_storage),
        string_entry("source_slug", source_slug),
        string_entry("source_directory", HOI4_CWT_CONFIG.source_directory),
        string_entry("repository_url", repository_url),
        string_entry("git_url", git_url),
        string_entry("upstream_url", upstream_url),
        string_entry("revision", HOI4_CWT_CONFIG.revision),
        string_entry("upstream_revision", HOI4_CWT_CONFIG.upstream_revision),
        string_entry("license", HOI4_CWT_CONFIG.license),
        string_entry("embedded_source_id", embedded_source_id),
        string_entry("content_sha256", HOI4_CWT_CONFIG_CONTENT_SHA256),
        integer_entry("rule_source_count", HOI4_CWT_CONFIG_SOURCE_COUNT),
        integer_entry("rule_source_bytes", HOI4_CWT_CONFIG_TOTAL_BYTES),
        string_entry("virtual_source_prefix", virtual_source_prefix),
        bool_entry("embedded_rule_files_in_repo", false),
        bool_entry("embedded_archive_bytes_in_binary", false),
        bool_entry("embedded_static_sources_in_binary", true),
        bool_entry("runtime_disk_entities", false),
    ]
}

fn string_entry(key: &'static str, value: impl Into<String>) -> (&'static str, TomlValue) {
    (key, TomlValue::String(value.into()))
}

fn integer_entry(key: &'static str, value: usize) -> (&'static str, TomlValue) {
    (key, TomlValue::Integer(value))
}

fn bool_entry(key: &'static str, value: bool) -> (&'static str, TomlValue) {
    (key, TomlValue::Bool(value))
}

fn write_toml_entry(output: &mut String, key: &str, value: TomlValue) {
    match value {
        TomlValue::String(value) => writeln!(output, "{} = {}", key, toml_string(&value)),
        TomlValue::Integer(value) => writeln!(output, "{key} = {value}"),
        TomlValue::Bool(value) => writeln!(output, "{key} = {value}"),
    }
    .expect("writing to String cannot fail");
}

fn metadata_markdown() -> String {
    let repository_url = HOI4_CWT_CONFIG.repository_url();
    let upstream_url = HOI4_CWT_CONFIG.upstream_url();
    let virtual_source_prefix = HOI4_CWT_CONFIG.virtual_source_prefix();

    format!(
        "# HOI4 CWT config source\n\n\
         - Rules crate: {}\n\
         - Upstream rules: {}\n\
         - Revision: `{}`\n\
         - Upstream revision: `{}`\n\
         - License: {}\n\
         - Rule sources: {}\n\
         - Rule source bytes: {}\n\
         - Content SHA-256: `{}`\n\
         - Runtime storage: {}\n\
         - Embedded RHoiScribe rule files: none\n\n\
         RHoiScribe consumes the pinned rules crate as a Cargo git dependency and reads its \
         static `.cwt` source table in process memory, reporting virtual paths under `{}`. \
         It does not extract, copy, cache, lock, or rewrite these rules on disk.\n\n\
         ## Runtime language support\n\n\
         Use `open_hoi4_language_workspace` early in MCP sessions, then poll \
         `get_hoi4_language_status` until the workspace is warm. Project validation defaults to \
         hybrid CWT plus legacy checks through `validate_hoi4_project`; pass \
         `validation_mode = \"legacy\"` only when legacy-only behavior is required. Use \
         `validate_hoi4_file`, `explain_hoi4_diagnostic`, symbol/definition/reference/completion \
         tools, `inspect_hoi4_scope`, and `inspect_hoi4_type_rule` for model-facing language \
         support. Use `generate_missing_localisation` for reviewable dry-run localisation \
         candidates, then write approved entries through `generate_localisation_batch`.\n\n\
         CWT rules, diagnostics, workspace snapshots, symbols, completions, and localisation \
         candidates stay in process memory. CWT language tools skip RNMDB tool-call logging so \
         CWT analysis state is not written to the `.rhoiscribe` log store.\n",
        repository_url,
        upstream_url,
        HOI4_CWT_CONFIG.revision,
        HOI4_CWT_CONFIG.upstream_revision,
        HOI4_CWT_CONFIG.license,
        HOI4_CWT_CONFIG_SOURCE_COUNT,
        HOI4_CWT_CONFIG_TOTAL_BYTES,
        HOI4_CWT_CONFIG_CONTENT_SHA256,
        HOI4_CWT_CONFIG.runtime_storage,
        virtual_source_prefix,
    )
}

fn toml_string(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                write!(&mut output, "\\u{{{:x}}}", character as u32)
                    .expect("writing to String cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}
