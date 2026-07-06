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
        hoi4_cwt_config_archive_url,
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
                description: "Pinned build-time GitHub source for in-memory HOI4 CWT rules."
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
    let archive_url = hoi4_cwt_config_archive_url();

    writeln!(
        &mut output,
        "source_format = {}",
        toml_string(HOI4_CWT_CONFIG.source_format)
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "runtime_storage = {}",
        toml_string(HOI4_CWT_CONFIG.runtime_storage)
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "upstream_url = {}",
        toml_string(HOI4_CWT_CONFIG.upstream_url)
    )
    .expect("writing to String cannot fail");
    writeln!(&mut output, "archive_url = {}", toml_string(&archive_url))
        .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "revision = {}",
        toml_string(HOI4_CWT_CONFIG.revision)
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "license = {}",
        toml_string(HOI4_CWT_CONFIG.license)
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "content_sha256 = {}",
        toml_string(HOI4_CWT_CONFIG_CONTENT_SHA256)
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "rule_source_count = {}",
        HOI4_CWT_CONFIG_SOURCE_COUNT
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "rule_source_bytes = {}",
        HOI4_CWT_CONFIG_TOTAL_BYTES
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "virtual_source_prefix = {}",
        toml_string(HOI4_CWT_CONFIG.virtual_source_prefix)
    )
    .expect("writing to String cannot fail");
    writeln!(&mut output, "embedded_rule_files_in_repo = false")
        .expect("writing to String cannot fail");
    writeln!(&mut output, "embedded_archive_bytes_in_binary = true")
        .expect("writing to String cannot fail");
    writeln!(&mut output, "runtime_disk_entities = false").expect("writing to String cannot fail");

    output
}

fn metadata_markdown() -> String {
    format!(
        "# HOI4 CWT config source\n\n\
         - Upstream: {}\n\
         - Revision: `{}`\n\
         - Archive: {}\n\
         - License: {}\n\
         - Rule sources: {}\n\
         - Rule source bytes: {}\n\
         - Content SHA-256: `{}`\n\
         - Runtime storage: {}\n\
         - Embedded RHoiScribe rule files: none\n\n\
         RHoiScribe embeds the pinned GitHub archive into the compiled binary at build time, \
         decompresses `.cwt` files from static bytes in memory, and reports virtual paths under `{}`. \
         It does not extract, copy, cache, lock, or rewrite these rules on disk.\n",
        HOI4_CWT_CONFIG.upstream_url,
        HOI4_CWT_CONFIG.revision,
        hoi4_cwt_config_archive_url(),
        HOI4_CWT_CONFIG.license,
        HOI4_CWT_CONFIG_SOURCE_COUNT,
        HOI4_CWT_CONFIG_TOTAL_BYTES,
        HOI4_CWT_CONFIG_CONTENT_SHA256,
        HOI4_CWT_CONFIG.runtime_storage,
        HOI4_CWT_CONFIG.virtual_source_prefix,
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
