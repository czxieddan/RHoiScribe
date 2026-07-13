//------------------------------------------------------------------------------------
// state/store/schema.rs -- Part of RHoiScribe
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

use rnmdb_cli::CommandOutput;

use super::{RnmdbStateStore, records::required_text, sql::sql_text};
use crate::state::{
    RNMDB_REVISION, STATE_SCHEMA_VERSION, path::clean_display_path, state_database_error,
};

const CREATE_METADATA_TABLE: &str =
    "CREATE TABLE IF NOT EXISTS state_metadata (name TEXT NOT NULL, value TEXT NOT NULL);";
const CREATE_METADATA_INDEX: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS state_metadata_name_uq ON state_metadata (name);";
const CREATE_PREFERENCES_TABLE: &str = "CREATE TABLE IF NOT EXISTS agent_preferences (record_key TEXT NOT NULL, scope_kind TEXT NOT NULL, scope_key TEXT NOT NULL, mod_root TEXT, preference_key TEXT NOT NULL, value_json TEXT NOT NULL, updated_at_unix_seconds INT64 NOT NULL);";
const CREATE_PREFERENCES_INDEX: &str = "CREATE UNIQUE INDEX IF NOT EXISTS agent_preferences_record_key_uq ON agent_preferences (record_key);";
const CREATE_LOGS_TABLE: &str = "CREATE TABLE IF NOT EXISTS tool_logs (sequence INT64 NOT NULL, timestamp_unix_seconds INT64 NOT NULL, scope_kind TEXT NOT NULL, scope_key TEXT NOT NULL, mod_root TEXT, tool_name TEXT NOT NULL, arguments_json TEXT NOT NULL, success BOOL NOT NULL, result_json TEXT, error_text TEXT, search_text TEXT);";
const ADD_LOG_SEARCH_TEXT_COLUMN: &str =
    "ALTER TABLE tool_logs ADD COLUMN IF NOT EXISTS search_text TEXT;";
const CREATE_LOGS_INDEX: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS tool_logs_sequence_uq ON tool_logs (sequence);";
const MIGRATION_SOURCE_METADATA: &str = "last_migration_source_path";
const MIGRATION_BACKUP_METADATA: &str = "last_migration_backup_path";

impl RnmdbStateStore {
    pub(super) fn ensure_schema(&mut self) -> Result<(), String> {
        self.transaction("schema", |store| {
            store.execute_schema_statements()?;
            store.ensure_metadata_value("schema_version", &STATE_SCHEMA_VERSION.to_string())?;
            store.ensure_metadata_value("rnmdb_revision", RNMDB_REVISION)?;
            store.validate_schema_version()
        })
    }

    fn execute_schema_statements(&mut self) -> Result<(), String> {
        for sql in [
            CREATE_METADATA_TABLE,
            CREATE_METADATA_INDEX,
            CREATE_PREFERENCES_TABLE,
            CREATE_PREFERENCES_INDEX,
            CREATE_LOGS_TABLE,
            ADD_LOG_SEARCH_TEXT_COLUMN,
            CREATE_LOGS_INDEX,
        ] {
            self.execute(sql, "schema")?;
        }
        Ok(())
    }

    fn ensure_metadata_value(&mut self, name: &str, value: &str) -> Result<(), String> {
        if self.metadata_value(name)?.is_some() {
            return Ok(());
        }
        let name = sql_text(name);
        let value = sql_text(value);
        self.execute(
            &format!("INSERT INTO state_metadata (name, value) VALUES ({name}, {value});"),
            "schema",
        )?;
        Ok(())
    }

    pub(super) fn set_migration_identity(
        &mut self,
        source: &Path,
        backup: &Path,
    ) -> Result<(), String> {
        self.set_metadata_value(MIGRATION_SOURCE_METADATA, &clean_display_path(source))?;
        self.set_metadata_value(MIGRATION_BACKUP_METADATA, &clean_display_path(backup))
    }

    fn set_metadata_value(&mut self, name: &str, value: &str) -> Result<(), String> {
        let escaped_name = sql_text(name);
        self.execute(
            &format!("DELETE FROM state_metadata WHERE name = {escaped_name};"),
            "migrate",
        )?;
        let escaped_value = sql_text(value);
        self.execute(
            &format!(
                "INSERT INTO state_metadata (name, value) VALUES ({escaped_name}, {escaped_value});"
            ),
            "migrate",
        )?;
        Ok(())
    }

    pub(super) fn verify_migration_identity(
        &mut self,
        source: &Path,
        backup: &Path,
    ) -> Result<(), String> {
        self.verify_metadata_value(MIGRATION_SOURCE_METADATA, &clean_display_path(source))?;
        self.verify_metadata_value(MIGRATION_BACKUP_METADATA, &clean_display_path(backup))
    }

    fn verify_metadata_value(&mut self, name: &str, expected: &str) -> Result<(), String> {
        if self.metadata_value(name)?.as_deref() == Some(expected) {
            return Ok(());
        }
        Err(state_database_error(
            &self.canonical_path,
            "verify",
            format!("migration metadata {name} does not match the verified swap identity"),
        ))
    }

    pub(super) fn validate_schema_version(&mut self) -> Result<(), String> {
        let expected = STATE_SCHEMA_VERSION.to_string();
        match self.metadata_value("schema_version")? {
            Some(version) if version == expected => Ok(()),
            Some(version) => Err(state_database_error(
                &self.canonical_path,
                "schema",
                format!("unsupported state schema version {version}"),
            )),
            None => Err(state_database_error(
                &self.canonical_path,
                "schema",
                "state schema version metadata is missing",
            )),
        }
    }

    fn metadata_value(&mut self, name: &str) -> Result<Option<String>, String> {
        let name = sql_text(name);
        let output = self.execute(
            &format!("SELECT value FROM state_metadata WHERE name = {name};"),
            "query",
        )?;
        let CommandOutput::Rows(batch) = output else {
            return Err(self.unexpected_rows("metadata query"));
        };
        batch
            .rows()
            .first()
            .map(|row| required_text(row.values(), 0, "state_metadata.value"))
            .transpose()
            .map_err(|error| state_database_error(&self.canonical_path, "query", error))
    }

    pub(super) fn required_metadata_value(&mut self, name: &str) -> Result<String, String> {
        self.metadata_value(name)?.ok_or_else(|| {
            state_database_error(
                &self.canonical_path,
                "recover",
                format!("migration metadata {name} is missing"),
            )
        })
    }

    pub(in crate::state) fn migration_identity(&mut self) -> Result<(PathBuf, PathBuf), String> {
        let source = self.required_metadata_value(MIGRATION_SOURCE_METADATA)?;
        let backup = self.required_metadata_value(MIGRATION_BACKUP_METADATA)?;
        Ok((PathBuf::from(source), PathBuf::from(backup)))
    }
}
