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
    pub(crate) upstream_url: &'static str,
    pub(crate) git_url: &'static str,
    pub(crate) revision: &'static str,
    pub(crate) license: &'static str,
    pub(crate) source_format: &'static str,
    pub(crate) runtime_storage: &'static str,
    pub(crate) virtual_source_prefix: &'static str,
}

pub(crate) const HOI4_CWT_CONFIG: Hoi4CwtConfigSource = Hoi4CwtConfigSource {
    upstream_url: "https://github.com/NS9927/cwtools-hoi4-config",
    git_url: "https://github.com/NS9927/cwtools-hoi4-config.git",
    revision: "584e57ad975bb9b2408851cf440d75d2e58b2860",
    license: "MIT",
    source_format: "github_git_archive",
    runtime_storage: "compiled GitHub archive bytes; decompressed into process memory only",
    virtual_source_prefix: "github://NS9927/cwtools-hoi4-config/config/",
};

impl Hoi4CwtConfigSource {
    pub(crate) fn archive_url(&self) -> String {
        format!("{}/archive/{}.zip", self.upstream_url, self.revision)
    }

    pub(crate) fn embedded_source_id(&self) -> String {
        format!(
            "embedded-github:NS9927/cwtools-hoi4-config@{}",
            self.revision
        )
    }

    pub(crate) fn virtual_path(&self, relative_path: &str) -> String {
        format!("{}{}", self.virtual_source_prefix, relative_path)
    }
}
