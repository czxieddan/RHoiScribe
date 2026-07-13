//------------------------------------------------------------------------------------
// state/legacy.rs -- Part of RHoiScribe
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
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rnmdb_common::ids::PageId;
use rnmdb_storage::{
    PageCryptoKey, SingleFileBackend, SingleFileFormatCompatibilityStatus, SingleFileOptions,
    StorageBackend, check_single_file_format_compatibility, upgrade_single_file_with_key,
    verify_single_file_with_key,
};
use serde::Deserialize;
use serde_json::Value;

use super::{
    GLOBAL_SCOPE_KEY, GLOBAL_SCOPE_KIND, StateMigrationReport, StoredPreferenceRecord,
    StoredToolLogRecord, global_record_key, is_state_database_error,
    path::{
        existing_page_crypto_key, legacy_state_database_path, page_crypto_key,
        sync_parent_directory,
    },
    state_database_error,
    store::RnmdbStateStore,
};

const PREFERENCES_PAGE_ID: u64 = 1;
const TOOL_LOG_INDEX_PAGE_ID: u64 = 2;
const TOOL_LOG_DATA_START_PAGE_ID: u64 = 3;
const SQL_FRAME_MAGIC: &[u8; 8] = b"RNOVSI01";
const LEGACY_SCHEMA_VERSION: u32 = 1;
const SQL_MIGRATION_LABEL: &str = "migrating-sql-v2";
const FORMAT_UPGRADE_LABEL: &str = "legacy-format-upgrade";

#[derive(Debug, Deserialize)]
struct LegacyPreferences {
    schema_version: u32,
    preferences: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct LegacyToolLogIndex {
    schema_version: u32,
    byte_len: usize,
    page_count: u64,
}

impl LegacyToolLogIndex {
    fn is_empty(&self) -> bool {
        self.byte_len == 0 && self.page_count == 0
    }
}

#[derive(Debug, Deserialize)]
struct LegacyToolLogEntry {
    sequence: u64,
    timestamp_unix_seconds: u64,
    tool_name: String,
    arguments: Value,
    success: bool,
    result: Option<Value>,
    error: Option<String>,
}

struct LegacySnapshot {
    preferences: Vec<StoredPreferenceRecord>,
    logs: Vec<StoredToolLogRecord>,
}

struct MigrationTemporary {
    path: PathBuf,
}

impl MigrationTemporary {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

struct ReadableSource {
    original_path: PathBuf,
    readable_path: PathBuf,
    temporary_upgrade: Option<MigrationTemporary>,
}

enum ExistingLayout {
    Sql,
    Legacy(LegacySnapshot),
}

struct InterruptedMigration {
    temporary_path: PathBuf,
    backup_path: PathBuf,
    retained_artifact_paths: Vec<PathBuf>,
}

pub(super) fn prepare_state_database(
    canonical_path: &Path,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<Option<StateMigrationReport>, String> {
    if let Some(report) = recover_interrupted_migration(canonical_path, mutation_lock)? {
        return Ok(Some(report));
    }
    let Some(source_path) = existing_source_path(canonical_path)? else {
        return Ok(None);
    };
    let key =
        page_crypto_key().map_err(|error| state_database_error(canonical_path, "open", error))?;
    let readable = prepare_readable_source(&source_path, canonical_path, key, mutation_lock)?;
    let layout = match inspect_existing_layout(&readable.readable_path, canonical_path, key) {
        Ok(layout) => layout,
        Err(error) => return Err(clean_readable_source(&readable, canonical_path, error)),
    };
    finish_existing_layout(readable, canonical_path, key, layout, mutation_lock)
}

fn recover_interrupted_migration(
    canonical_path: &Path,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<Option<StateMigrationReport>, String> {
    if existing_source_path(canonical_path)?.is_some() {
        return Ok(None);
    }
    let paths = interrupted_temporary_paths(canonical_path)?;
    let existing = existing_paths(&paths, canonical_path)?;
    if existing.is_empty() {
        reject_backup_only_state(canonical_path)?;
        return Ok(None);
    }
    let key = existing_page_crypto_key()
        .map_err(|error| state_database_error(canonical_path, "recover", error))?;
    let mut candidates = Vec::new();
    let mut retained_artifacts = Vec::new();
    for path in existing {
        match interrupted_candidate(&path, canonical_path, key, mutation_lock)? {
            Some(candidate) => candidates.push(candidate),
            None => retained_artifacts.push(path),
        }
    }
    let mut candidate = single_recovery_candidate(candidates, canonical_path)?;
    candidate.retained_artifact_paths = retained_artifacts;
    install_interrupted_migration(candidate, canonical_path, mutation_lock).map(Some)
}

fn reject_backup_only_state(canonical_path: &Path) -> Result<(), String> {
    let backups = existing_backup_paths(canonical_path)?;
    if backups.is_empty() {
        return Ok(());
    }
    let paths = backups
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join(", ");
    Err(state_database_error(
        canonical_path,
        "recover",
        format!(
            "backup-only state requires explicit recovery before a new database can be created: {paths}"
        ),
    ))
}

fn existing_backup_paths(canonical_path: &Path) -> Result<Vec<PathBuf>, String> {
    let parent = canonical_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let entries = fs::read_dir(parent)
        .map_err(|error| state_database_error(canonical_path, "recover", error.to_string()))?;
    let mut backups = Vec::new();
    for entry in entries {
        let entry = entry
            .map_err(|error| state_database_error(canonical_path, "recover", error.to_string()))?;
        let path = entry.path();
        if is_backup_path(canonical_path, &path) {
            backups.push(path);
        }
    }
    backups.sort();
    Ok(backups)
}

fn interrupted_temporary_paths(canonical_path: &Path) -> Result<[PathBuf; 2], String> {
    Ok([
        temporary_path(canonical_path, SQL_MIGRATION_LABEL)?,
        temporary_path(canonical_path, FORMAT_UPGRADE_LABEL)?,
    ])
}

fn existing_paths(paths: &[PathBuf], canonical_path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut existing = Vec::new();
    for path in paths {
        if path_entry_exists(path)
            .map_err(|error| state_database_error(canonical_path, "recover", error))?
        {
            existing.push(path.clone());
        }
    }
    Ok(existing)
}

fn interrupted_candidate(
    temporary_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<Option<InterruptedMigration>, String> {
    validate_temporary_path(temporary_path, canonical_path, "recover")?;
    mutation_lock
        .bind_existing_database(temporary_path)
        .map_err(|error| state_database_error(canonical_path, "recover", error))?;
    verify_authenticated(temporary_path, canonical_path, key)?;
    match inspect_existing_layout(temporary_path, canonical_path, key)? {
        ExistingLayout::Legacy(_) => Ok(None),
        ExistingLayout::Sql => {
            let mut store = RnmdbStateStore::open_existing_migration(
                temporary_path,
                canonical_path,
                key,
                mutation_lock,
            )?;
            let (source_path, backup_path) = store.migration_identity()?;
            drop(store);
            validate_interrupted_paths(
                canonical_path,
                &source_path,
                &backup_path,
                key,
                mutation_lock,
            )?;
            Ok(Some(InterruptedMigration {
                temporary_path: temporary_path.to_path_buf(),
                backup_path,
                retained_artifact_paths: Vec::new(),
            }))
        }
    }
}

fn single_recovery_candidate(
    mut candidates: Vec<InterruptedMigration>,
    canonical_path: &Path,
) -> Result<InterruptedMigration, String> {
    if candidates.len() == 1 {
        return Ok(candidates.remove(0));
    }
    Err(state_database_error(
        canonical_path,
        "recover",
        format!(
            "expected exactly one authenticated interrupted migration, found {}",
            candidates.len()
        ),
    ))
}

fn validate_interrupted_paths(
    canonical_path: &Path,
    source_path: &Path,
    backup_path: &Path,
    key: PageCryptoKey,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<(), String> {
    let legacy_path = legacy_state_database_path(canonical_path);
    if !paths_match(source_path, canonical_path) && !paths_match(source_path, &legacy_path) {
        return Err(state_database_error(
            canonical_path,
            "recover",
            "migration source metadata does not name the canonical or legacy state path",
        ));
    }
    reject_existing_target(canonical_path, source_path, "recover")?;
    validate_backup_path(canonical_path, backup_path)?;
    mutation_lock
        .bind_existing_database(backup_path)
        .map_err(|error| state_database_error(canonical_path, "recover", error))?;
    verify_recovery_backup(backup_path, canonical_path, key)
}

fn verify_recovery_backup(
    backup_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
) -> Result<(), String> {
    let report = verify_single_file_with_key(backup_path, key)
        .map_err(|error| state_database_error(canonical_path, "recover", error.to_string()))?;
    if report.encryption_authenticated()
        && report.authenticated_page_records() == report.present_page_records()
    {
        return Ok(());
    }
    Err(state_database_error(
        canonical_path,
        "recover",
        "migration backup did not authenticate every stored page",
    ))
}

fn validate_backup_path(canonical_path: &Path, backup_path: &Path) -> Result<(), String> {
    if !is_backup_path(canonical_path, backup_path) {
        return Err(state_database_error(
            canonical_path,
            "recover",
            "migration backup metadata is outside the expected sibling namespace",
        ));
    }
    let metadata = path_entry_metadata(backup_path)
        .map_err(|error| state_database_error(canonical_path, "recover", error))?
        .ok_or_else(|| {
            state_database_error(canonical_path, "recover", "migration backup is missing")
        })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(state_database_error(
            canonical_path,
            "recover",
            "migration backup must be a regular file",
        ));
    }
    Ok(())
}

fn is_backup_path(canonical_path: &Path, backup_path: &Path) -> bool {
    let expected_parent = canonical_path.parent().unwrap_or_else(|| Path::new(""));
    let backup_parent = backup_path.parent().unwrap_or_else(|| Path::new(""));
    let Some(stem) = canonical_path.file_stem() else {
        return false;
    };
    let prefix = format!("{}.pre-sql-v2", stem.to_string_lossy());
    let name = backup_path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let valid_name = name == format!("{prefix}.rnmdb")
        || (name.starts_with(&format!("{prefix}.")) && name.ends_with(".rnmdb"));
    paths_match(expected_parent, backup_parent) && valid_name
}

fn install_interrupted_migration(
    candidate: InterruptedMigration,
    canonical_path: &Path,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<StateMigrationReport, String> {
    validate_temporary_path(&candidate.temporary_path, canonical_path, "recover")?;
    mutation_lock
        .bind_existing_database(&candidate.temporary_path)
        .map_err(|error| state_database_error(canonical_path, "recover", error))?;
    reject_existing_target(canonical_path, canonical_path, "recover")?;
    rename_no_replace(&candidate.temporary_path, canonical_path)
        .map_err(|error| state_database_error(canonical_path, "recover", error.to_string()))?;
    sync_parent_directory(canonical_path)
        .map_err(|error| state_database_error(canonical_path, "recover", error))?;
    Ok(StateMigrationReport {
        retained_backup_path: candidate.backup_path,
        retained_artifact_paths: candidate.retained_artifact_paths,
    })
}

fn validate_temporary_path(path: &Path, canonical_path: &Path, stage: &str) -> Result<(), String> {
    let metadata = path_entry_metadata(path)
        .map_err(|error| state_database_error(canonical_path, stage, error))?
        .ok_or_else(|| {
            state_database_error(canonical_path, stage, "migration temporary is missing")
        })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(state_database_error(
            canonical_path,
            stage,
            format!(
                "migration temporary {} must be a regular file",
                path.to_string_lossy()
            ),
        ));
    }
    Ok(())
}

fn existing_source_path(canonical_path: &Path) -> Result<Option<PathBuf>, String> {
    if source_path_candidate(canonical_path, canonical_path)? {
        return Ok(Some(canonical_path.to_path_buf()));
    }
    let legacy_path = legacy_state_database_path(canonical_path);
    if source_path_candidate(&legacy_path, canonical_path)? {
        return Ok(Some(legacy_path));
    }
    Ok(None)
}

fn source_path_candidate(path: &Path, canonical_path: &Path) -> Result<bool, String> {
    let Some(metadata) = path_entry_metadata(path)
        .map_err(|error| state_database_error(canonical_path, "open", error))?
    else {
        return Ok(false);
    };
    if metadata.file_type().is_symlink() {
        return Err(state_database_error(
            canonical_path,
            "open",
            format!(
                "state database source {} must not be a symbolic link",
                path.to_string_lossy()
            ),
        ));
    }
    Ok(metadata.file_type().is_file())
}

fn prepare_readable_source(
    source_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<ReadableSource, String> {
    let compatibility = check_single_file_format_compatibility(source_path)
        .map_err(|error| state_database_error(canonical_path, "open", error.to_string()))?;
    match compatibility.status() {
        SingleFileFormatCompatibilityStatus::Supported => Ok(ReadableSource {
            original_path: source_path.to_path_buf(),
            readable_path: source_path.to_path_buf(),
            temporary_upgrade: None,
        }),
        SingleFileFormatCompatibilityStatus::UnsupportedOlder => {
            upgrade_legacy_format(source_path, canonical_path, key, mutation_lock)
        }
        SingleFileFormatCompatibilityStatus::UnsupportedNewer => Err(state_database_error(
            canonical_path,
            "open",
            "state database requires a newer RNMDB engine",
        )),
    }
}

fn upgrade_legacy_format(
    source_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<ReadableSource, String> {
    let target = temporary_path(canonical_path, FORMAT_UPGRADE_LABEL)?;
    reject_existing_target(canonical_path, &target, "migrate")?;
    if let Err(error) = upgrade_single_file_with_key(source_path, &target, key) {
        let error = state_database_error(canonical_path, "migrate", error.to_string());
        return Err(retain_unowned_migration(&target, canonical_path, error));
    }
    validate_temporary_path(&target, canonical_path, "migrate")?;
    mutation_lock
        .bind_existing_database(&target)
        .map_err(|error| state_database_error(canonical_path, "migrate", error))?;
    let temporary = MigrationTemporary::new(target.clone());
    if let Err(error) = verify_authenticated(&target, canonical_path, key) {
        return Err(retain_created_migration(&temporary, canonical_path, error));
    }
    if let Err(error) = sync_verified_temporary(&target, canonical_path) {
        return Err(retain_created_migration(&temporary, canonical_path, error));
    }
    Ok(ReadableSource {
        original_path: source_path.to_path_buf(),
        readable_path: target,
        temporary_upgrade: Some(temporary),
    })
}

fn inspect_existing_layout(
    readable_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
) -> Result<ExistingLayout, String> {
    let backend = SingleFileBackend::open_with_key(readable_path, key)
        .map_err(|error| state_database_error(canonical_path, "open", error.to_string()))?;
    if backend.catalog_root().is_some() {
        return Ok(ExistingLayout::Sql);
    }
    if legacy_root_is_sql_frame(&backend, canonical_path)? {
        return Ok(ExistingLayout::Sql);
    }
    read_legacy_snapshot(&backend, canonical_path)
        .map(ExistingLayout::Legacy)
        .map_err(|error| state_database_error(canonical_path, "migrate", error))
}

fn legacy_root_is_sql_frame(
    backend: &SingleFileBackend,
    canonical_path: &Path,
) -> Result<bool, String> {
    let page = read_page(backend, canonical_path, PREFERENCES_PAGE_ID)?;
    Ok(page.is_some_and(|payload| payload.starts_with(SQL_FRAME_MAGIC)))
}

fn finish_existing_layout(
    readable: ReadableSource,
    canonical_path: &Path,
    key: PageCryptoKey,
    layout: ExistingLayout,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<Option<StateMigrationReport>, String> {
    match layout {
        ExistingLayout::Sql => finish_existing_sql(readable, canonical_path, key, mutation_lock),
        ExistingLayout::Legacy(snapshot) => {
            let retained = readable
                .temporary_upgrade
                .as_ref()
                .map(|temporary| temporary.path().to_path_buf());
            let mut report = migrate_snapshot(
                &readable.original_path,
                canonical_path,
                key,
                snapshot,
                mutation_lock,
            )?;
            if let (Some(report), Some(path)) = (&mut report, retained) {
                report.retained_artifact_paths.push(path);
            }
            Ok(report)
        }
    }
}

fn finish_existing_sql(
    readable: ReadableSource,
    canonical_path: &Path,
    key: PageCryptoKey,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<Option<StateMigrationReport>, String> {
    if let Err(error) = verify_authenticated(&readable.readable_path, canonical_path, key) {
        return Err(clean_readable_source(&readable, canonical_path, error));
    }
    if let Some(temporary) = &readable.temporary_upgrade {
        let backup = match unique_backup_path(canonical_path) {
            Ok(path) => path,
            Err(error) => return Err(clean_readable_source(&readable, canonical_path, error)),
        };
        if let Err(error) = prepare_swap_identity(
            temporary.path(),
            &readable.original_path,
            &backup,
            canonical_path,
            key,
            mutation_lock,
        ) {
            return Err(retain_created_migration(temporary, canonical_path, error));
        }
        return swap_database(&readable.original_path, temporary, &backup, canonical_path)
            .map(Some);
    }
    if readable.original_path != canonical_path {
        promote_legacy_name(&readable.original_path, canonical_path)?;
    }
    Ok(None)
}

fn prepare_swap_identity(
    temporary_path: &Path,
    source_path: &Path,
    backup_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<(), String> {
    validate_temporary_path(temporary_path, canonical_path, "migrate")?;
    let mut store = RnmdbStateStore::open_existing_migration(
        temporary_path,
        canonical_path,
        key,
        mutation_lock,
    )?;
    store.persist_migration_identity(source_path, backup_path)?;
    drop(store);
    verify_authenticated(temporary_path, canonical_path, key)?;
    sync_verified_temporary(temporary_path, canonical_path)
}

fn promote_legacy_name(source: &Path, canonical_path: &Path) -> Result<(), String> {
    reject_existing_target(canonical_path, canonical_path, "swap")?;
    rename_no_replace(source, canonical_path)
        .map_err(|error| state_database_error(canonical_path, "swap", error.to_string()))?;
    if let Err(error) = sync_parent_directory(canonical_path) {
        return restore_legacy_name(source, canonical_path, error);
    }
    Ok(())
}

fn restore_legacy_name(
    source: &Path,
    canonical_path: &Path,
    failure: String,
) -> Result<(), String> {
    let restore = match rename_no_replace(canonical_path, source) {
        Ok(()) => directory_sync_status(source),
        Err(error) => format!("restore failed: {error}"),
    };
    Err(state_database_error(
        canonical_path,
        "swap",
        format!("legacy-name directory sync failed: {failure}; {restore}"),
    ))
}

fn read_legacy_snapshot(
    backend: &SingleFileBackend,
    canonical_path: &Path,
) -> Result<LegacySnapshot, String> {
    let preferences = read_legacy_preferences(backend, canonical_path)?;
    let logs = read_legacy_logs(backend, canonical_path)?;
    Ok(LegacySnapshot { preferences, logs })
}

fn read_legacy_preferences(
    backend: &SingleFileBackend,
    canonical_path: &Path,
) -> Result<Vec<StoredPreferenceRecord>, String> {
    let Some(payload) = read_page(backend, canonical_path, PREFERENCES_PAGE_ID)? else {
        return Ok(Vec::new());
    };
    let Some(preferences) = decode_length_prefixed::<LegacyPreferences>(&payload, "preferences")?
    else {
        return Ok(Vec::new());
    };
    validate_legacy_schema(preferences.schema_version, "preferences")?;
    let updated_at = unix_timestamp_now();
    preferences
        .preferences
        .into_iter()
        .map(|(key, value)| legacy_preference_record(key, value, updated_at))
        .collect()
}

fn legacy_preference_record(
    preference_key: String,
    value: Value,
    updated_at_unix_seconds: u64,
) -> Result<StoredPreferenceRecord, String> {
    let value_json = serde_json::to_string(&value).map_err(|error| error.to_string())?;
    Ok(StoredPreferenceRecord {
        record_key: global_record_key(&preference_key),
        scope_kind: GLOBAL_SCOPE_KIND.to_string(),
        scope_key: GLOBAL_SCOPE_KEY.to_string(),
        mod_root: None,
        preference_key,
        value_json,
        updated_at_unix_seconds,
    })
}

fn read_legacy_logs(
    backend: &SingleFileBackend,
    canonical_path: &Path,
) -> Result<Vec<StoredToolLogRecord>, String> {
    let Some(payload) = read_page(backend, canonical_path, TOOL_LOG_INDEX_PAGE_ID)? else {
        return Ok(Vec::new());
    };
    let Some(index) = decode_length_prefixed::<LegacyToolLogIndex>(&payload, "tool log index")?
    else {
        return Ok(Vec::new());
    };
    validate_legacy_schema(index.schema_version, "tool log index")?;
    if index.is_empty() {
        return Ok(Vec::new());
    }
    validate_log_index(&index, backend)?;
    let bytes = read_legacy_log_bytes(backend, canonical_path, &index)?;
    let entries = serde_json::from_slice::<Vec<LegacyToolLogEntry>>(&bytes)
        .map_err(|error| format!("failed to decode legacy tool logs: {error}"))?;
    entries.into_iter().map(legacy_log_record).collect()
}

fn validate_log_index(
    index: &LegacyToolLogIndex,
    backend: &SingleFileBackend,
) -> Result<(), String> {
    let page_size = backend.page_size().bytes();
    let expected = index.byte_len.div_ceil(page_size);
    let actual = usize::try_from(index.page_count)
        .map_err(|_| "legacy tool log page count does not fit this platform".to_string())?;
    if actual != expected {
        return Err(format!(
            "legacy tool log index page count {actual} does not match byte length {}",
            index.byte_len
        ));
    }
    let file_len = fs::metadata(backend.path())
        .map_err(|error| format!("failed to inspect legacy state database size: {error}"))?
        .len();
    let byte_len = u64::try_from(index.byte_len)
        .map_err(|_| "legacy tool log byte length does not fit RNMDB limits".to_string())?;
    if byte_len > file_len {
        return Err("legacy tool log byte length exceeds the database file size".to_string());
    }
    Ok(())
}

fn read_legacy_log_bytes(
    backend: &SingleFileBackend,
    canonical_path: &Path,
    index: &LegacyToolLogIndex,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(index.byte_len)
        .map_err(|error| format!("legacy tool log allocation failed: {error}"))?;
    for offset in 0..index.page_count {
        let page_id = TOOL_LOG_DATA_START_PAGE_ID.saturating_add(offset);
        let payload = read_page(backend, canonical_path, page_id)?
            .ok_or_else(|| format!("legacy tool log page {page_id} is missing"))?;
        bytes.extend_from_slice(&payload);
    }
    bytes.truncate(index.byte_len);
    Ok(bytes)
}

fn legacy_log_record(entry: LegacyToolLogEntry) -> Result<StoredToolLogRecord, String> {
    let arguments_json =
        serde_json::to_string(&entry.arguments).map_err(|error| error.to_string())?;
    let result_json = entry
        .result
        .map(|value| serde_json::to_string(&value))
        .transpose()
        .map_err(|error| error.to_string())?;
    Ok(StoredToolLogRecord {
        sequence: entry.sequence,
        timestamp_unix_seconds: entry.timestamp_unix_seconds,
        scope_kind: GLOBAL_SCOPE_KIND.to_string(),
        scope_key: GLOBAL_SCOPE_KEY.to_string(),
        mod_root: None,
        tool_name: entry.tool_name,
        arguments_json,
        success: entry.success,
        result_json,
        error_text: entry.error,
    })
}

fn read_page(
    backend: &SingleFileBackend,
    canonical_path: &Path,
    page_id: u64,
) -> Result<Option<Vec<u8>>, String> {
    backend
        .read_page(PageId::new(page_id))
        .map(|page| page.map(|page| page.payload().to_vec()))
        .map_err(|error| {
            state_database_error(
                canonical_path,
                "migrate",
                format!("failed to read legacy page {page_id}: {error}"),
            )
        })
}

fn decode_length_prefixed<T>(payload: &[u8], label: &str) -> Result<Option<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    if payload.len() < 4 {
        return Ok(None);
    }
    let length = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
    if length == 0 {
        return Ok(None);
    }
    if length > payload.len().saturating_sub(4) {
        return Err(format!("legacy {label} page has an invalid payload length"));
    }
    serde_json::from_slice(&payload[4..4 + length])
        .map(Some)
        .map_err(|error| format!("failed to decode legacy {label}: {error}"))
}

pub(crate) fn is_legacy_state_page(page_id: u64, payload: &[u8]) -> bool {
    let Ok(Some(Value::Object(object))) =
        decode_length_prefixed::<Value>(payload, "state classifier")
    else {
        return false;
    };
    let has_schema = object.get("schema_version").is_some_and(Value::is_u64);
    match page_id {
        PREFERENCES_PAGE_ID => {
            has_schema && object.get("preferences").is_some_and(Value::is_object)
        }
        TOOL_LOG_INDEX_PAGE_ID => {
            has_schema
                && object.get("byte_len").is_some_and(Value::is_u64)
                && object.get("page_count").is_some_and(Value::is_u64)
        }
        _ => false,
    }
}

fn validate_legacy_schema(version: u32, label: &str) -> Result<(), String> {
    if version <= LEGACY_SCHEMA_VERSION {
        return Ok(());
    }
    Err(format!(
        "legacy {label} schema version {version} is newer than supported version {LEGACY_SCHEMA_VERSION}"
    ))
}

fn migrate_snapshot(
    source_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
    snapshot: LegacySnapshot,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<Option<StateMigrationReport>, String> {
    let backup_path = unique_backup_path(canonical_path)?;
    let migration_path = temporary_path(canonical_path, SQL_MIGRATION_LABEL)?;
    reject_existing_target(canonical_path, &migration_path, "migrate")?;
    let migration =
        reserve_migration_database(&migration_path, canonical_path, key, mutation_lock)?;
    let build_result = build_migration_database(
        migration.path(),
        canonical_path,
        &backup_path,
        source_path,
        key,
        &snapshot,
        mutation_lock,
    );
    if let Err(error) = build_result {
        return Err(retain_created_migration(&migration, canonical_path, error));
    }
    swap_database(source_path, &migration, &backup_path, canonical_path).map(Some)
}

fn reserve_migration_database(
    migration_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<MigrationTemporary, String> {
    SingleFileBackend::create(
        migration_path,
        SingleFileOptions::default().with_page_key(key),
    )
    .map(drop)
    .map_err(|error| state_database_error(canonical_path, "migrate", error.to_string()))?;
    validate_temporary_path(migration_path, canonical_path, "migrate")?;
    mutation_lock
        .bind_existing_database(migration_path)
        .map_err(|error| state_database_error(canonical_path, "migrate", error))?;
    Ok(MigrationTemporary::new(migration_path.to_path_buf()))
}

fn build_migration_database(
    migration_path: &Path,
    canonical_path: &Path,
    backup_path: &Path,
    source_path: &Path,
    key: PageCryptoKey,
    snapshot: &LegacySnapshot,
    mutation_lock: &mut super::path::StateMutationLock,
) -> Result<(), String> {
    let mut store =
        RnmdbStateStore::create_migration(migration_path, canonical_path, mutation_lock)?;
    store.import_legacy(
        &snapshot.preferences,
        &snapshot.logs,
        source_path,
        backup_path,
    )?;
    store.verify_import(
        snapshot.preferences.len(),
        snapshot.logs.len(),
        source_path,
        backup_path,
    )?;
    drop(store);
    verify_authenticated(migration_path, canonical_path, key)?;
    sync_verified_temporary(migration_path, canonical_path)
}

fn retain_created_migration(
    temporary: &MigrationTemporary,
    canonical_path: &Path,
    error: String,
) -> String {
    retain_migration(temporary.path(), canonical_path, error)
}

fn retain_unowned_migration(path: &Path, canonical_path: &Path, error: String) -> String {
    retain_migration(path, canonical_path, error)
}

fn retain_migration(path: &Path, canonical_path: &Path, error: String) -> String {
    let retained = format!(
        "incomplete migration database retained for manual inspection at {}",
        path.to_string_lossy()
    );
    let detail = format!("{error}; {retained}");
    if is_state_database_error(&error) {
        return detail;
    }
    state_database_error(canonical_path, "migrate", detail)
}

fn verify_authenticated(
    path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
) -> Result<(), String> {
    let report = verify_single_file_with_key(path, key)
        .map_err(|error| state_database_error(canonical_path, "verify", error.to_string()))?;
    if report.encryption_authenticated() && report.is_valid() {
        return Ok(());
    }
    Err(state_database_error(
        canonical_path,
        "verify",
        "RNMDB authenticated verification did not validate every stored page",
    ))
}

fn sync_verified_temporary(path: &Path, canonical_path: &Path) -> Result<(), String> {
    sync_parent_directory(path)
        .map_err(|error| state_database_error(canonical_path, "verify", error))
}

fn swap_database(
    source: &Path,
    migration: &MigrationTemporary,
    backup: &Path,
    canonical_path: &Path,
) -> Result<StateMigrationReport, String> {
    if let Err(error) = reject_existing_target(canonical_path, backup, "swap") {
        return Err(retain_created_migration(migration, canonical_path, error));
    }
    if let Err(error) = rename_no_replace(source, backup) {
        let error = state_database_error(canonical_path, "swap", error.to_string());
        return Err(retain_created_migration(migration, canonical_path, error));
    }
    if let Err(error) = sync_parent_directory(backup) {
        return recover_uninstalled(source, migration, backup, canonical_path, error);
    }
    install_migration(source, migration, backup, canonical_path)
}

fn install_migration(
    source: &Path,
    migration: &MigrationTemporary,
    backup: &Path,
    canonical_path: &Path,
) -> Result<StateMigrationReport, String> {
    if let Err(error) = reject_existing_target(canonical_path, canonical_path, "swap") {
        return recover_uninstalled(source, migration, backup, canonical_path, error);
    }
    if let Err(error) = rename_no_replace(migration.path(), canonical_path) {
        return recover_uninstalled(source, migration, backup, canonical_path, error.to_string());
    }
    if let Err(error) = sync_parent_directory(canonical_path) {
        return recover_installed(source, backup, canonical_path, error);
    }
    Ok(StateMigrationReport {
        retained_backup_path: backup.to_path_buf(),
        retained_artifact_paths: Vec::new(),
    })
}

fn recover_uninstalled(
    source: &Path,
    migration: &MigrationTemporary,
    backup: &Path,
    canonical_path: &Path,
    failure: String,
) -> Result<StateMigrationReport, String> {
    let restore = restore_original(backup, source);
    let cleanup = format!(
        "uninstalled migration retained at {} for fail-closed recovery",
        migration.path().to_string_lossy()
    );
    Err(recovery_error(canonical_path, &failure, &restore, &cleanup))
}

fn recover_installed(
    _source: &Path,
    backup: &Path,
    canonical_path: &Path,
    failure: String,
) -> Result<StateMigrationReport, String> {
    let restore = format!(
        "original retained at {}; installed replacement retained at {}",
        backup.to_string_lossy(),
        canonical_path.to_string_lossy()
    );
    Err(recovery_error(
        canonical_path,
        &failure,
        &restore,
        "no path was deleted after the installed replacement failed to sync",
    ))
}

fn restore_original(backup: &Path, source: &Path) -> String {
    match path_entry_exists(source) {
        Ok(true) => {
            return format!(
                "original restore refused because source path already exists; original remains at {}",
                backup.to_string_lossy()
            );
        }
        Err(error) => {
            return format!(
                "original restore refused because source path could not be inspected ({error}); original remains at {}",
                backup.to_string_lossy()
            );
        }
        Ok(false) => {}
    }
    match rename_no_replace(backup, source) {
        Ok(()) => format!("original restored; {}", directory_sync_status(source)),
        Err(error) => format!(
            "original restore failed: {error}; original remains at {}",
            backup.to_string_lossy()
        ),
    }
}

fn recovery_error(canonical_path: &Path, failure: &str, restore: &str, cleanup: &str) -> String {
    state_database_error(
        canonical_path,
        "swap",
        format!("install failed: {failure}; restore status: {restore}; cleanup status: {cleanup}"),
    )
}

fn directory_sync_status(path: &Path) -> String {
    match sync_parent_directory(path) {
        Ok(()) => "parent directory synced".to_string(),
        Err(error) => format!("parent directory sync failed: {error}"),
    }
}

fn unique_backup_path(canonical_path: &Path) -> Result<PathBuf, String> {
    unique_sibling_path(canonical_path, "pre-sql-v2", "swap")
}

fn temporary_path(canonical_path: &Path, label: &str) -> Result<PathBuf, String> {
    sibling_path(canonical_path, label, "migrate")
}

fn unique_sibling_path(canonical_path: &Path, label: &str, stage: &str) -> Result<PathBuf, String> {
    for suffix in 0..10_000_u32 {
        let label = if suffix == 0 {
            label.to_string()
        } else {
            format!("{label}.{suffix}")
        };
        let candidate = sibling_path(canonical_path, &label, stage)?;
        let occupied = path_entry_exists(&candidate)
            .map_err(|error| state_database_error(canonical_path, stage, error))?;
        if !occupied {
            return Ok(candidate);
        }
    }
    Err(state_database_error(
        canonical_path,
        stage,
        format!("could not allocate a unique {label} sibling path"),
    ))
}

fn sibling_path(canonical_path: &Path, label: &str, stage: &str) -> Result<PathBuf, String> {
    let stem = canonical_path
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| state_database_error(canonical_path, stage, "invalid database name"))?;
    let file_name = format!("{}.{}.rnmdb", stem.to_string_lossy(), label);
    Ok(canonical_path.with_file_name(file_name))
}

fn reject_existing_target(canonical_path: &Path, target: &Path, stage: &str) -> Result<(), String> {
    let occupied = path_entry_exists(target)
        .map_err(|error| state_database_error(canonical_path, stage, error))?;
    if !occupied {
        return Ok(());
    }
    Err(state_database_error(
        canonical_path,
        stage,
        format!(
            "refusing to overwrite migration target {}",
            target.to_string_lossy()
        ),
    ))
}

fn path_entry_exists(path: &Path) -> Result<bool, String> {
    path_entry_metadata(path).map(|metadata| metadata.is_some())
}

fn path_entry_metadata(path: &Path) -> Result<Option<fs::Metadata>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "failed to inspect path entry {}: {error}",
            path.to_string_lossy()
        )),
    }
}

#[cfg(windows)]
fn paths_match(left: &Path, right: &Path) -> bool {
    super::path::clean_display_path(left)
        .eq_ignore_ascii_case(&super::path::clean_display_path(right))
}

#[cfg(not(windows))]
fn paths_match(left: &Path, right: &Path) -> bool {
    left == right
}

fn clean_readable_source(
    readable: &ReadableSource,
    canonical_path: &Path,
    error: String,
) -> String {
    let Some(path) = &readable.temporary_upgrade else {
        return error;
    };
    retain_created_migration(path, canonical_path, error)
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "redox"
))]
fn rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, source, CWD, destination, RenameFlags::NOREPLACE).map_err(io::Error::from)
}

#[cfg(windows)]
fn rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // MoveFileExW without MOVEFILE_REPLACE_EXISTING is an atomic no-overwrite move.
    let moved = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "redox",
    windows
)))]
fn rename_no_replace(_source: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace state migration is unsupported on this platform",
    ))
}

fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
