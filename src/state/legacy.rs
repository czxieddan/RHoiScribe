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

use std::path::{Path, PathBuf};

use rnmdb_storage::{
    PageCryptoKey, SingleFileBackend, SingleFileFormatCompatibilityStatus, SingleFileOptions,
    check_single_file_format_compatibility, upgrade_single_file_with_key,
};

use self::{
    artifacts::{
        FORMAT_UPGRADE_LABEL, MigrationTemporary, SQL_MIGRATION_LABEL, reject_existing_target,
        retain_created_migration, retain_unowned_migration, sync_verified_temporary,
        temporary_path, unique_backup_path, validate_temporary_path, verify_authenticated,
    },
    reader::{ExistingLayout, LegacySnapshot, inspect_existing_layout},
    recovery::{existing_source_path, recover_interrupted_migration},
    swap::{promote_legacy_name, swap_database},
};
use super::{
    StateMigrationReport,
    path::{StateMutationLock, page_crypto_key},
    state_database_error,
    store::RnmdbStateStore,
};

mod artifacts;
mod reader;
mod recovery;
mod swap;

pub(crate) use reader::is_legacy_state_page;

struct ReadableSource {
    original_path: PathBuf,
    readable_path: PathBuf,
    temporary_upgrade: Option<MigrationTemporary>,
}

pub(super) fn prepare_state_database(
    canonical_path: &Path,
    mutation_lock: &mut StateMutationLock,
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

fn prepare_readable_source(
    source_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
    mutation_lock: &mut StateMutationLock,
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
    mutation_lock: &mut StateMutationLock,
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

fn finish_existing_layout(
    readable: ReadableSource,
    canonical_path: &Path,
    key: PageCryptoKey,
    layout: ExistingLayout,
    mutation_lock: &mut StateMutationLock,
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
    mutation_lock: &mut StateMutationLock,
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
    mutation_lock: &mut StateMutationLock,
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

fn migrate_snapshot(
    source_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
    snapshot: LegacySnapshot,
    mutation_lock: &mut StateMutationLock,
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
    mutation_lock: &mut StateMutationLock,
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
    mutation_lock: &mut StateMutationLock,
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
