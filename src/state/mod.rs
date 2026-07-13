//------------------------------------------------------------------------------------
// state/mod.rs -- Part of RHoiScribe
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

use std::path::{Path, PathBuf};

pub(crate) mod legacy;
pub(crate) mod maintenance;
pub(crate) mod path;
pub(crate) mod scope;
pub(crate) mod store;

pub(crate) use path::{StateMutationLock, clean_display_path, state_store_path};
pub(crate) use scope::StateScope;
pub(crate) use store::RnmdbStateStore;

pub(crate) const STATE_SCHEMA_VERSION: u32 = 2;
pub(crate) const RNMDB_REVISION: &str = "8d2b65ad1ee3ee542e1307c1693bc4de4f7edbee";
pub(crate) const GLOBAL_SCOPE_KIND: &str = "global";
pub(crate) const GLOBAL_SCOPE_KEY: &str = "global";
pub(crate) const MOD_SCOPE_KIND: &str = "mod";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateMigrationReport {
    pub(crate) retained_backup_path: PathBuf,
    pub(crate) retained_artifact_paths: Vec<PathBuf>,
}

impl StateMigrationReport {
    pub(crate) fn retained_backup_message(&self) -> String {
        let mut message = format!(
            "legacy RNMDB state migrated; retained backup: {}",
            clean_display_path(&self.retained_backup_path)
        );
        if !self.retained_artifact_paths.is_empty() {
            let paths = self
                .retained_artifact_paths
                .iter()
                .map(|path| clean_display_path(path))
                .collect::<Vec<_>>()
                .join(", ");
            message.push_str(&format!("; retained compatibility artifacts: {paths}"));
        }
        message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredPreferenceRecord {
    pub(crate) record_key: String,
    pub(crate) scope_kind: String,
    pub(crate) scope_key: String,
    pub(crate) mod_root: Option<String>,
    pub(crate) preference_key: String,
    pub(crate) value_json: String,
    pub(crate) updated_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredToolLogRecord {
    pub(crate) sequence: u64,
    pub(crate) timestamp_unix_seconds: u64,
    pub(crate) scope_kind: String,
    pub(crate) scope_key: String,
    pub(crate) mod_root: Option<String>,
    pub(crate) tool_name: String,
    pub(crate) arguments_json: String,
    pub(crate) success: bool,
    pub(crate) result_json: Option<String>,
    pub(crate) error_text: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StoredToolLogFilter {
    pub(crate) scope: Option<StateScope>,
    pub(crate) tool_name: Option<String>,
    pub(crate) success: Option<bool>,
    pub(crate) since_unix_seconds: Option<u64>,
    pub(crate) until_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredToolLogSearchRow {
    pub(crate) record: StoredToolLogRecord,
    pub(crate) search_text: String,
}

pub(crate) fn state_database_error(path: &Path, stage: &str, detail: impl AsRef<str>) -> String {
    let detail = detail.as_ref();
    if is_state_database_error(detail) {
        return detail.to_string();
    }
    format!(
        "RHoiScribe state database `{}` failed during {stage}: {detail}",
        clean_display_path(path)
    )
}

pub(crate) fn is_state_database_error(error: &str) -> bool {
    error.starts_with("RHoiScribe state database `")
}

pub(crate) fn global_record_key(preference_key: &str) -> String {
    preference_record_key(&StateScope::Global, preference_key)
}

pub(crate) fn preference_record_key(scope: &StateScope, preference_key: &str) -> String {
    stored_preference_record_key(scope.kind(), scope.key(), preference_key)
}

pub(crate) fn stored_preference_record_key(
    scope_kind: &str,
    scope_key: &str,
    preference_key: &str,
) -> String {
    format!("{scope_kind}:{scope_key}:{preference_key}")
}
