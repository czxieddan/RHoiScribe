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

use std::path::{Path, PathBuf};

use rnmdb_cli::{CommandOutput, LocalSession};
use rnmdb_storage::PageCryptoKey;
use rnmdb_types::SqlValue;
use serde_json::Value;

use super::{
    RNMDB_REVISION, STATE_SCHEMA_VERSION, StateMigrationReport, StateScope, StoredPreferenceRecord,
    StoredToolLogFilter, StoredToolLogRecord, StoredToolLogSearchRow, legacy,
    path::{StateMutationLock, clean_display_path, ensure_parent, page_crypto_key},
    scope::validate_stored_scope,
    state_database_error, stored_preference_record_key,
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

    pub(super) fn migration_identity(&mut self) -> Result<(PathBuf, PathBuf), String> {
        let source = self.required_metadata_value(MIGRATION_SOURCE_METADATA)?;
        let backup = self.required_metadata_value(MIGRATION_BACKUP_METADATA)?;
        Ok((PathBuf::from(source), PathBuf::from(backup)))
    }

    fn ensure_schema(&mut self) -> Result<(), String> {
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

    fn set_migration_identity(&mut self, source: &Path, backup: &Path) -> Result<(), String> {
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

    fn verify_migration_identity(&mut self, source: &Path, backup: &Path) -> Result<(), String> {
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

    fn validate_schema_version(&mut self) -> Result<(), String> {
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

    fn required_metadata_value(&mut self, name: &str) -> Result<String, String> {
        self.metadata_value(name)?.ok_or_else(|| {
            state_database_error(
                &self.canonical_path,
                "recover",
                format!("migration metadata {name} is missing"),
            )
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

fn preference_insert_sql(record: &StoredPreferenceRecord) -> Result<String, String> {
    let updated_at = sql_i64(record.updated_at_unix_seconds, "preference timestamp")?;
    Ok(format!(
        "INSERT INTO agent_preferences (record_key, scope_kind, scope_key, mod_root, preference_key, value_json, updated_at_unix_seconds) VALUES ({}, {}, {}, {}, {}, {}, {updated_at});",
        sql_text(&record.record_key),
        sql_text(&record.scope_kind),
        sql_text(&record.scope_key),
        sql_optional_text(record.mod_root.as_deref()),
        sql_text(&record.preference_key),
        sql_text(&record.value_json),
    ))
}

fn log_insert_sql(record: &StoredToolLogRecord) -> Result<String, String> {
    let sequence = sql_i64(record.sequence, "tool log sequence")?;
    let timestamp = sql_i64(record.timestamp_unix_seconds, "tool log timestamp")?;
    let success = if record.success { "TRUE" } else { "FALSE" };
    let search_text = log_search_text(record);
    Ok(format!(
        "INSERT INTO tool_logs (sequence, timestamp_unix_seconds, scope_kind, scope_key, mod_root, tool_name, arguments_json, success, result_json, error_text, search_text) VALUES ({sequence}, {timestamp}, {}, {}, {}, {}, {}, {success}, {}, {}, {});",
        sql_text(&record.scope_kind),
        sql_text(&record.scope_key),
        sql_optional_text(record.mod_root.as_deref()),
        sql_text(&record.tool_name),
        sql_text(&record.arguments_json),
        sql_optional_text(record.result_json.as_deref()),
        sql_optional_text(record.error_text.as_deref()),
        sql_text(&search_text),
    ))
}

fn log_search_text(record: &StoredToolLogRecord) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        record.tool_name,
        record.scope_kind,
        record.mod_root.as_deref().unwrap_or(&record.scope_key),
        record.arguments_json,
        record.result_json.as_deref().unwrap_or(""),
        record.error_text.as_deref().unwrap_or("")
    )
}

fn log_filter_where_clause(filter: &StoredToolLogFilter) -> Result<String, String> {
    let predicates = [
        log_scope_predicate(filter.scope.as_ref()),
        log_text_predicate("tool_name", filter.tool_name.as_deref()),
        log_success_predicate(filter.success),
        log_time_predicate(">=", filter.since_unix_seconds)?,
        log_time_predicate("<=", filter.until_unix_seconds)?,
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    Ok(where_clause(predicates))
}

fn log_scope_predicate(scope: Option<&StateScope>) -> Option<String> {
    scope.map(|scope| {
        format!(
            "(scope_kind = {} AND scope_key = {})",
            sql_text(scope.kind()),
            sql_text(scope.key())
        )
    })
}

fn log_text_predicate(column: &str, value: Option<&str>) -> Option<String> {
    value.map(|value| format!("{column} = {}", sql_text(value)))
}

fn log_success_predicate(success: Option<bool>) -> Option<String> {
    success.map(|success| format!("success = {}", if success { "TRUE" } else { "FALSE" }))
}

fn log_time_predicate(operator: &str, value: Option<u64>) -> Result<Option<String>, String> {
    value
        .map(|value| {
            sql_i64(value, "tool log filter timestamp")
                .map(|value| format!("timestamp_unix_seconds {operator} {value}"))
        })
        .transpose()
}

fn where_clause(predicates: Vec<String>) -> String {
    if predicates.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", predicates.join(" AND "))
    }
}

fn validate_import(
    preferences: &[StoredPreferenceRecord],
    logs: &[StoredToolLogRecord],
) -> Result<(), String> {
    for preference in preferences {
        validate_preference(preference)?;
    }
    for log in logs {
        validate_log(log)?;
    }
    Ok(())
}

fn validate_preference(record: &StoredPreferenceRecord) -> Result<(), String> {
    validate_preference_identity(record)?;
    validate_json(&record.value_json, "preference value")?;
    sql_i64(record.updated_at_unix_seconds, "preference timestamp").map(|_| ())
}

fn validate_preference_identity(record: &StoredPreferenceRecord) -> Result<(), String> {
    validate_stored_scope(
        &record.scope_kind,
        &record.scope_key,
        record.mod_root.as_deref(),
    )?;
    let expected = stored_preference_record_key(
        &record.scope_kind,
        &record.scope_key,
        &record.preference_key,
    );
    if record.record_key != expected {
        return Err("preference record_key does not match its scope and key".to_string());
    }
    Ok(())
}

fn validate_log(record: &StoredToolLogRecord) -> Result<(), String> {
    validate_stored_scope(
        &record.scope_kind,
        &record.scope_key,
        record.mod_root.as_deref(),
    )?;
    if record.tool_name.trim().is_empty() {
        return Err("tool log name must not be empty".to_string());
    }
    validate_json(&record.arguments_json, "tool log arguments")?;
    if let Some(result) = &record.result_json {
        validate_json(result, "tool log result")?;
    }
    sql_i64(record.sequence, "tool log sequence")?;
    sql_i64(record.timestamp_unix_seconds, "tool log timestamp").map(|_| ())
}

fn validate_json(value: &str, label: &str) -> Result<(), String> {
    serde_json::from_str::<Value>(value)
        .map(|_| ())
        .map_err(|error| format!("{label} is not valid JSON: {error}"))
}

fn decode_preference(values: &[SqlValue]) -> Result<StoredPreferenceRecord, String> {
    let record = StoredPreferenceRecord {
        record_key: required_text(values, 0, "agent_preferences.record_key")?,
        scope_kind: required_text(values, 1, "agent_preferences.scope_kind")?,
        scope_key: required_text(values, 2, "agent_preferences.scope_key")?,
        mod_root: optional_text(values, 3, "agent_preferences.mod_root")?,
        preference_key: required_text(values, 4, "agent_preferences.preference_key")?,
        value_json: required_json(values, 5, "agent_preferences.value_json")?,
        updated_at_unix_seconds: required_u64(
            values,
            6,
            "agent_preferences.updated_at_unix_seconds",
        )?,
    };
    validate_preference(&record)?;
    Ok(record)
}

fn decode_log(values: &[SqlValue]) -> Result<StoredToolLogRecord, String> {
    let record = StoredToolLogRecord {
        sequence: required_u64(values, 0, "tool_logs.sequence")?,
        timestamp_unix_seconds: required_u64(values, 1, "tool_logs.timestamp_unix_seconds")?,
        scope_kind: required_text(values, 2, "tool_logs.scope_kind")?,
        scope_key: required_text(values, 3, "tool_logs.scope_key")?,
        mod_root: optional_text(values, 4, "tool_logs.mod_root")?,
        tool_name: required_text(values, 5, "tool_logs.tool_name")?,
        arguments_json: required_json(values, 6, "tool_logs.arguments_json")?,
        success: required_bool(values, 7, "tool_logs.success")?,
        result_json: optional_json(values, 8, "tool_logs.result_json")?,
        error_text: optional_text(values, 9, "tool_logs.error_text")?,
    };
    validate_log(&record)?;
    Ok(record)
}

fn decode_log_search_row(values: &[SqlValue]) -> Result<StoredToolLogSearchRow, String> {
    let record = decode_log(values)?;
    let search_text = optional_text(values, 10, "tool_logs.search_text")?
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| log_search_text(&record));
    Ok(StoredToolLogSearchRow {
        record,
        search_text,
    })
}

fn required_text(values: &[SqlValue], index: usize, label: &str) -> Result<String, String> {
    match values.get(index) {
        Some(SqlValue::Text(value)) => Ok(value.clone()),
        _ => Err(format!("{label} is missing or not TEXT")),
    }
}

fn optional_text(values: &[SqlValue], index: usize, label: &str) -> Result<Option<String>, String> {
    match values.get(index) {
        Some(SqlValue::Null) => Ok(None),
        Some(SqlValue::Text(value)) => Ok(Some(value.clone())),
        _ => Err(format!("{label} is missing or not nullable TEXT")),
    }
}

fn required_json(values: &[SqlValue], index: usize, label: &str) -> Result<String, String> {
    let value = required_text(values, index, label)?;
    validate_json(&value, label)?;
    Ok(value)
}

fn optional_json(values: &[SqlValue], index: usize, label: &str) -> Result<Option<String>, String> {
    let value = optional_text(values, index, label)?;
    if let Some(json) = &value {
        validate_json(json, label)?;
    }
    Ok(value)
}

fn required_u64(values: &[SqlValue], index: usize, label: &str) -> Result<u64, String> {
    match values.get(index) {
        Some(SqlValue::Int64(value)) => {
            u64::try_from(*value).map_err(|_| format!("{label} must not be negative"))
        }
        _ => Err(format!("{label} is missing or not INT64")),
    }
}

fn optional_u64(values: &[SqlValue], index: usize, label: &str) -> Result<Option<u64>, String> {
    match values.get(index) {
        Some(SqlValue::Null) => Ok(None),
        Some(SqlValue::Int64(value)) => u64::try_from(*value)
            .map(Some)
            .map_err(|_| format!("{label} must not be negative")),
        _ => Err(format!("{label} is missing or not nullable INT64")),
    }
}

fn required_bool(values: &[SqlValue], index: usize, label: &str) -> Result<bool, String> {
    match values.get(index) {
        Some(SqlValue::Bool(value)) => Ok(*value),
        _ => Err(format!("{label} is missing or not BOOL")),
    }
}

fn rows_affected(output: CommandOutput) -> Option<u64> {
    match output {
        CommandOutput::RowsAffected(count) => Some(count),
        _ => None,
    }
}

fn row_count(output: CommandOutput) -> Option<usize> {
    match output {
        CommandOutput::Rows(batch) => Some(batch.rows().len()),
        _ => None,
    }
}

fn decode_count(output: CommandOutput, label: &str) -> Result<usize, String> {
    let CommandOutput::Rows(batch) = output else {
        return Err(format!("{label} returned an unexpected command result"));
    };
    let value = batch
        .rows()
        .first()
        .ok_or_else(|| format!("{label} returned no row"))
        .and_then(|row| required_u64(row.values(), 0, label))?;
    usize::try_from(value).map_err(|_| format!("{label} exceeds usize range"))
}

fn sql_i64(value: u64, label: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{label} exceeds RNMDB INT64 range"))
}

fn sql_usize(value: usize, label: &str) -> Result<i64, String> {
    let value = u64::try_from(value).map_err(|_| format!("{label} exceeds u64 range"))?;
    sql_i64(value, label)
}

fn sql_optional_text(value: Option<&str>) -> String {
    value.map(sql_text).unwrap_or_else(|| "NULL".to_string())
}

fn sql_text(value: &str) -> String {
    let escaped = value.replace('\'', "''");
    format!("'{escaped}'")
}
