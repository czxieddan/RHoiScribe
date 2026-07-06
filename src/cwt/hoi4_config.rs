//------------------------------------------------------------------------------------
// hoi4_config.rs -- Part of RHoiScribe
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hoi4CwtConfigSource {
    pub(crate) repository: &'static str,
    pub(crate) revision: &'static str,
    pub(crate) upstream_repository: &'static str,
    pub(crate) upstream_revision: &'static str,
    pub(crate) license: &'static str,
    pub(crate) source_directory: &'static str,
    pub(crate) virtual_prefix: &'static str,
    pub(crate) source_format: &'static str,
    pub(crate) runtime_storage: &'static str,
}

pub(crate) const HOI4_CWT_CONFIG: Hoi4CwtConfigSource = Hoi4CwtConfigSource {
    repository: cwtools_hoi4_config::METADATA.repository,
    revision: cwtools_hoi4_config::METADATA.revision,
    upstream_repository: cwtools_hoi4_config::METADATA.upstream_repository,
    upstream_revision: cwtools_hoi4_config::METADATA.upstream_revision,
    license: cwtools_hoi4_config::METADATA.license,
    source_directory: cwtools_hoi4_config::METADATA.source_directory,
    virtual_prefix: cwtools_hoi4_config::METADATA.virtual_prefix,
    source_format: "cargo_git_rev_crate",
    runtime_storage: "compiled Cargo git dependency static sources; process memory only",
};

impl Hoi4CwtConfigSource {
    pub(crate) fn source_slug(&self) -> String {
        self.repository
            .trim_end_matches(".git")
            .trim_start_matches("https://github.com/")
            .to_string()
    }

    pub(crate) fn repository_url(&self) -> String {
        self.repository.to_string()
    }

    pub(crate) fn git_url(&self) -> String {
        if self.repository.ends_with(".git") {
            self.repository.to_string()
        } else {
            format!("{}.git", self.repository)
        }
    }

    pub(crate) fn upstream_url(&self) -> String {
        self.upstream_repository.to_string()
    }

    pub(crate) fn virtual_source_prefix(&self) -> String {
        self.virtual_prefix.to_string()
    }

    pub(crate) fn embedded_source_id(&self) -> String {
        format!(
            "embedded-cargo-crate:{}@{}",
            self.source_slug(),
            self.revision
        )
    }
}
