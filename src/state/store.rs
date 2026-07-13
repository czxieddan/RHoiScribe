//------------------------------------------------------------------------------------
// state/store.rs -- Part of RHoiScribe
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

mod records;
mod schema;
mod sql;

use std::path::{Path, PathBuf};

use rnmdb_cli::{CommandOutput, LocalSession};
use rnmdb_storage::PageCryptoKey;

use self::{
    records::{
        decode_count, decode_log_search_row, decode_preference, optional_u64, required_u64,
        row_count, rows_affected, validate_import, validate_log, validate_preference,
    },
    sql::{log_filter_where_clause, log_insert_sql, preference_insert_sql, sql_text, sql_usize},
};

use super::{
    StateMigrationReport, StateScope, StoredPreferenceRecord, StoredToolLogFilter,
    StoredToolLogRecord, StoredToolLogSearchRow, legacy,
    path::{StateMutationLock, ensure_parent, page_crypto_key},
    state_database_error,
};

pub(crate) struct RnmdbStateStore {
    canonical_path: PathBuf,
    migration_report: Option<StateMigrationReport>,
    session: LocalSession,
    _mutation_lock: Option<StateMutationLock>,
}

impl RnmdbStateStore {
    pub(crate) fn open(path: &Path) -> Result<Self, String> {
        let mut mutation_lock = StateMutationLock::acquire(path)
            .map_err(|error| state_database_error(path, "open", error))?;
        let migration_report = legacy::prepare_state_database(path, &mut mutation_lock)?;
        mutation_lock
            .bind_existing_database(path)
            .map_err(|error| state_database_error(path, "open", error))?;
        let mut store = Self::open_ready(path, path, Some(&mut mutation_lock))?;
        store.migration_report = migration_report;
        store._mutation_lock = Some(mutation_lock);
        Ok(store)
    }

    pub(super) fn create_migration(
        path: &Path,
        canonical_path: &Path,
        mutation_lock: &mut StateMutationLock,
    ) -> Result<Self, String> {
        Self::open_ready(path, canonical_path, Some(mutation_lock))
    }

    pub(super) fn open_existing_migration(
        path: &Path,
        canonical_path: &Path,
        key: PageCryptoKey,
        mutation_lock: &mut StateMutationLock,
    ) -> Result<Self, String> {
        mutation_lock
            .bind_existing_database(path)
            .map_err(|error| state_database_error(canonical_path, "open", error))?;
        let session = LocalSession::single_file_with_key(path, key)
            .map_err(|error| state_database_error(canonical_path, "open", error.to_string()))?;
        mutation_lock
            .bind_existing_database(path)
            .map_err(|error| state_database_error(canonical_path, "open", error))?;
        let mut store = Self {
            canonical_path: canonical_path.to_path_buf(),
            migration_report: None,
            session,
            _mutation_lock: None,
        };
        store.validate_schema_version()?;
        Ok(store)
    }

    fn open_ready(
        path: &Path,
        canonical_path: &Path,
        mutation_lock: Option<&mut StateMutationLock>,
    ) -> Result<Self, String> {
        ensure_parent(path).map_err(|error| state_database_error(canonical_path, "open", error))?;
        let key = page_crypto_key()
            .map_err(|error| state_database_error(canonical_path, "open", error))?;
        let session = LocalSession::single_file_with_key(path, key)
            .map_err(|error| state_database_error(canonical_path, "open", error.to_string()))?;
        if let Some(mutation_lock) = mutation_lock {
            mutation_lock
                .bind_existing_database(path)
                .map_err(|error| state_database_error(canonical_path, "open", error))?;
        }
        let mut store = Self {
            canonical_path: canonical_path.to_path_buf(),
            migration_report: None,
            session,
            _mutation_lock: None,
        };
        store.ensure_schema()?;
        Ok(store)
    }

    pub(crate) fn take_migration_report(&mut self) -> Option<StateMigrationReport> {
        self.migration_report.take()
    }

    pub(crate) fn list_preferences(
        &mut self,
        scope: &StateScope,
    ) -> Result<Vec<StoredPreferenceRecord>, String> {
        let scope_kind = sql_text(scope.kind());
        let scope_key = sql_text(scope.key());
        let sql = format!(
            "SELECT record_key, scope_kind, scope_key, mod_root, preference_key, value_json, updated_at_unix_seconds FROM agent_preferences WHERE scope_kind = {scope_kind} AND scope_key = {scope_key} ORDER BY preference_key;"
        );
        self.preference_rows(&sql)
    }

    pub(crate) fn upsert_preference(
        &mut self,
        record: &StoredPreferenceRecord,
    ) -> Result<(), String> {
        validate_preference(record)
            .map_err(|error| state_database_error(&self.canonical_path, "query", error))?;
        self.transaction("transaction", |store| {
            store.delete_preference_record(&record.record_key)?;
            store.insert_preference_record(record)
        })
    }

    pub(crate) fn delete_preference(&mut self, record_key: &str) -> Result<bool, String> {
        self.transaction("transaction", |store| {
            let affected = store.delete_preference_record(record_key)?;
            Ok(affected > 0)
        })
    }

    pub(crate) fn count_logs(&mut self) -> Result<usize, String> {
        let output = self.execute("SELECT COUNT(*) FROM tool_logs;", "query")?;
        decode_count(output, "tool log count")
            .map_err(|error| state_database_error(&self.canonical_path, "query", error))
    }

    pub(crate) fn search_logs(
        &mut self,
        filter: &StoredToolLogFilter,
    ) -> Result<Vec<StoredToolLogSearchRow>, String> {
        let where_clause = log_filter_where_clause(filter)
            .map_err(|error| state_database_error(&self.canonical_path, "query", error))?;
        let sql = format!(
            "SELECT sequence, timestamp_unix_seconds, scope_kind, scope_key, mod_root, tool_name, arguments_json, success, result_json, error_text, search_text FROM tool_logs{where_clause} ORDER BY sequence DESC;"
        );
        self.log_search_rows(&sql)
    }

    pub(crate) fn append_log(
        &mut self,
        mut record: StoredToolLogRecord,
        max_entries: usize,
    ) -> Result<u64, String> {
        validate_log(&record)
            .map_err(|error| state_database_error(&self.canonical_path, "query", error))?;
        self.transaction("transaction", |store| {
            record.sequence = store.next_log_sequence()?;
            store.insert_log_record(&record)?;
            store.trim_logs(max_entries)?;
            Ok(record.sequence)
        })
    }

    pub(super) fn import_legacy(
        &mut self,
        preferences: &[StoredPreferenceRecord],
        logs: &[StoredToolLogRecord],
        source_path: &Path,
        backup_path: &Path,
    ) -> Result<(), String> {
        validate_import(preferences, logs)
            .map_err(|error| state_database_error(&self.canonical_path, "migrate", error))?;
        self.transaction("migrate", |store| {
            store.insert_preferences(preferences)?;
            store.insert_logs(logs)?;
            store.set_migration_identity(source_path, backup_path)
        })
    }

    pub(super) fn verify_import(
        &mut self,
        preference_count: usize,
        log_count: usize,
        source_path: &Path,
        backup_path: &Path,
    ) -> Result<(), String> {
        let actual_preferences = self.preference_record_count()?;
        let actual_logs = self.log_record_count()?;
        if (actual_preferences, actual_logs) != (preference_count, log_count) {
            return Err(state_database_error(
                &self.canonical_path,
                "verify",
                format!(
                    "imported row counts differ: preferences {actual_preferences}/{preference_count}, logs {actual_logs}/{log_count}"
                ),
            ));
        }
        self.verify_migration_identity(source_path, backup_path)
    }

    pub(super) fn persist_migration_identity(
        &mut self,
        source_path: &Path,
        backup_path: &Path,
    ) -> Result<(), String> {
        self.transaction("migrate", |store| {
            store.set_migration_identity(source_path, backup_path)
        })
    }

    fn preference_rows(&mut self, sql: &str) -> Result<Vec<StoredPreferenceRecord>, String> {
        let output = self.execute(sql, "query")?;
        let CommandOutput::Rows(batch) = output else {
            return Err(self.unexpected_rows("preference query"));
        };
        batch
            .rows()
            .iter()
            .map(|row| decode_preference(row.values()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| state_database_error(&self.canonical_path, "query", error))
    }

    fn log_search_rows(&mut self, sql: &str) -> Result<Vec<StoredToolLogSearchRow>, String> {
        let output = self.execute(sql, "query")?;
        let CommandOutput::Rows(batch) = output else {
            return Err(self.unexpected_rows("tool log query"));
        };
        batch
            .rows()
            .iter()
            .map(|row| decode_log_search_row(row.values()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| state_database_error(&self.canonical_path, "query", error))
    }

    fn delete_preference_record(&mut self, record_key: &str) -> Result<u64, String> {
        let record_key = sql_text(record_key);
        let output = self.execute(
            &format!("DELETE FROM agent_preferences WHERE record_key = {record_key};"),
            "query",
        )?;
        rows_affected(output).ok_or_else(|| self.unexpected_rows("preference delete"))
    }

    fn insert_preference_record(&mut self, record: &StoredPreferenceRecord) -> Result<(), String> {
        let sql = preference_insert_sql(record)
            .map_err(|error| state_database_error(&self.canonical_path, "query", error))?;
        self.execute(&sql, "query")?;
        Ok(())
    }

    fn insert_preferences(&mut self, records: &[StoredPreferenceRecord]) -> Result<(), String> {
        for record in records {
            self.insert_preference_record(record)?;
        }
        Ok(())
    }

    fn insert_log_record(&mut self, record: &StoredToolLogRecord) -> Result<(), String> {
        let sql = log_insert_sql(record)
            .map_err(|error| state_database_error(&self.canonical_path, "query", error))?;
        self.execute(&sql, "query")?;
        Ok(())
    }

    fn insert_logs(&mut self, records: &[StoredToolLogRecord]) -> Result<(), String> {
        for record in records {
            self.insert_log_record(record)?;
        }
        Ok(())
    }

    fn next_log_sequence(&mut self) -> Result<u64, String> {
        let output = self.execute("SELECT MAX(sequence) FROM tool_logs;", "query")?;
        let CommandOutput::Rows(batch) = output else {
            return Err(self.unexpected_rows("tool log maximum sequence query"));
        };
        let maximum = batch
            .rows()
            .first()
            .map(|row| optional_u64(row.values(), 0, "MAX(tool_logs.sequence)"))
            .transpose()
            .map_err(|error| state_database_error(&self.canonical_path, "query", error))?
            .flatten()
            .unwrap_or(0);
        maximum.checked_add(1).ok_or_else(|| {
            state_database_error(&self.canonical_path, "query", "tool log sequence overflow")
        })
    }

    fn trim_logs(&mut self, max_entries: usize) -> Result<(), String> {
        let overflow = self.count_logs()?.saturating_sub(max_entries);
        if overflow == 0 {
            return Ok(());
        }
        let cutoff = self.log_trim_cutoff(overflow - 1)?;
        self.execute(
            &format!("DELETE FROM tool_logs WHERE sequence <= {cutoff};"),
            "query",
        )?;
        Ok(())
    }

    fn log_trim_cutoff(&mut self, offset: usize) -> Result<u64, String> {
        let offset = sql_usize(offset, "tool log trim offset")?;
        let sql =
            format!("SELECT sequence FROM tool_logs ORDER BY sequence LIMIT 1 OFFSET {offset};");
        let output = self.execute(&sql, "query")?;
        let CommandOutput::Rows(batch) = output else {
            return Err(self.unexpected_rows("tool log trim cutoff query"));
        };
        batch
            .rows()
            .first()
            .ok_or_else(|| "tool log trim cutoff query returned no row".to_string())
            .and_then(|row| required_u64(row.values(), 0, "tool_logs.sequence"))
            .map_err(|error| state_database_error(&self.canonical_path, "query", error))
    }

    fn preference_record_count(&mut self) -> Result<usize, String> {
        let output = self.execute("SELECT record_key FROM agent_preferences;", "verify")?;
        row_count(output).ok_or_else(|| self.unexpected_rows("preference verification"))
    }

    fn log_record_count(&mut self) -> Result<usize, String> {
        let output = self.execute("SELECT sequence FROM tool_logs;", "verify")?;
        row_count(output).ok_or_else(|| self.unexpected_rows("tool log verification"))
    }

    fn transaction<T>(
        &mut self,
        stage: &str,
        operation: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        self.execute("BEGIN;", stage)?;
        match operation(self) {
            Ok(value) => self.commit_value(stage, value),
            Err(error) => Err(self.rollback_error(stage, error)),
        }
    }

    fn commit_value<T>(&mut self, stage: &str, value: T) -> Result<T, String> {
        match self.execute("COMMIT;", "commit") {
            Ok(_) => Ok(value),
            Err(error) => Err(self.rollback_error(stage, error)),
        }
    }

    fn rollback_error(&mut self, stage: &str, error: String) -> String {
        match self.execute("ROLLBACK;", "transaction") {
            Ok(_) => error,
            Err(rollback) => state_database_error(
                &self.canonical_path,
                stage,
                format!("{error}; rollback also failed: {rollback}"),
            ),
        }
    }

    fn execute(&mut self, sql: &str, stage: &str) -> Result<CommandOutput, String> {
        self.session
            .execute(sql)
            .map_err(|error| state_database_error(&self.canonical_path, stage, error.to_string()))
    }

    fn unexpected_rows(&self, operation: &str) -> String {
        state_database_error(
            &self.canonical_path,
            "query",
            format!("{operation} returned an unexpected command result"),
        )
    }
}
