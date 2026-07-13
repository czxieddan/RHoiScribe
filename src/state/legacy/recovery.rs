//------------------------------------------------------------------------------------
// state/legacy/recovery.rs -- Part of RHoiScribe
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
    fs,
    path::{Path, PathBuf},
};

use rnmdb_storage::{PageCryptoKey, verify_single_file_with_key};

use super::{
    super::{
        StateMigrationReport,
        path::{
            StateMutationLock, existing_page_crypto_key, legacy_state_database_path,
            sync_parent_directory,
        },
        state_database_error,
        store::RnmdbStateStore,
    },
    artifacts::{
        FORMAT_UPGRADE_LABEL, SQL_MIGRATION_LABEL, path_entry_exists, path_entry_metadata,
        paths_match, reject_existing_target, rename_no_replace, temporary_path,
        validate_temporary_path, verify_authenticated,
    },
    reader::{ExistingLayout, inspect_existing_layout},
};

struct InterruptedMigration {
    temporary_path: PathBuf,
    backup_path: PathBuf,
    retained_artifact_paths: Vec<PathBuf>,
}

#[derive(Default)]
struct RecoveryCandidates {
    installable: Vec<InterruptedMigration>,
    retained_artifact_paths: Vec<PathBuf>,
}

impl RecoveryCandidates {
    fn record(&mut self, path: PathBuf, candidate: Option<InterruptedMigration>) {
        match candidate {
            Some(candidate) => self.installable.push(candidate),
            None => self.retained_artifact_paths.push(path),
        }
    }

    fn into_single(mut self, canonical_path: &Path) -> Result<InterruptedMigration, String> {
        if self.installable.len() != 1 {
            return Err(state_database_error(
                canonical_path,
                "recover",
                format!(
                    "expected exactly one authenticated interrupted migration, found {}",
                    self.installable.len()
                ),
            ));
        }
        let mut candidate = self.installable.remove(0);
        candidate.retained_artifact_paths = self.retained_artifact_paths;
        Ok(candidate)
    }
}

pub(super) fn recover_interrupted_migration(
    canonical_path: &Path,
    mutation_lock: &mut StateMutationLock,
) -> Result<Option<StateMigrationReport>, String> {
    let Some(paths) = recovery_paths(canonical_path)? else {
        return Ok(None);
    };
    let key = existing_page_crypto_key()
        .map_err(|error| state_database_error(canonical_path, "recover", error))?;
    let candidate = collect_recovery_candidates(paths, canonical_path, key, mutation_lock)?;
    install_interrupted_migration(candidate, canonical_path, mutation_lock).map(Some)
}

fn recovery_paths(canonical_path: &Path) -> Result<Option<Vec<PathBuf>>, String> {
    if existing_source_path(canonical_path)?.is_some() {
        return Ok(None);
    }
    let paths = interrupted_temporary_paths(canonical_path)?;
    let existing = existing_paths(&paths, canonical_path)?;
    if !existing.is_empty() {
        return Ok(Some(existing));
    }
    reject_backup_only_state(canonical_path)?;
    Ok(None)
}

fn collect_recovery_candidates(
    paths: Vec<PathBuf>,
    canonical_path: &Path,
    key: PageCryptoKey,
    mutation_lock: &mut StateMutationLock,
) -> Result<InterruptedMigration, String> {
    let mut candidates = RecoveryCandidates::default();
    for path in paths {
        let candidate = interrupted_candidate(&path, canonical_path, key, mutation_lock)?;
        candidates.record(path, candidate);
    }
    candidates.into_single(canonical_path)
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
    mutation_lock: &mut StateMutationLock,
) -> Result<Option<InterruptedMigration>, String> {
    validate_temporary_path(temporary_path, canonical_path, "recover")?;
    mutation_lock
        .bind_existing_database(temporary_path)
        .map_err(|error| state_database_error(canonical_path, "recover", error))?;
    verify_authenticated(temporary_path, canonical_path, key)?;
    let layout = inspect_existing_layout(temporary_path, canonical_path, key)?;
    candidate_for_layout(layout, temporary_path, canonical_path, key, mutation_lock)
}

fn candidate_for_layout(
    layout: ExistingLayout,
    temporary_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
    mutation_lock: &mut StateMutationLock,
) -> Result<Option<InterruptedMigration>, String> {
    match layout {
        ExistingLayout::Legacy(_) => Ok(None),
        ExistingLayout::Sql => {
            sql_interrupted_candidate(temporary_path, canonical_path, key, mutation_lock).map(Some)
        }
    }
}

fn sql_interrupted_candidate(
    temporary_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
    mutation_lock: &mut StateMutationLock,
) -> Result<InterruptedMigration, String> {
    let mut store = RnmdbStateStore::open_existing_migration(
        temporary_path,
        canonical_path,
        key,
        mutation_lock,
    )?;
    let identity = store.migration_identity();
    drop(store);
    let (source_path, backup_path) = identity?;
    validate_interrupted_paths(
        canonical_path,
        &source_path,
        &backup_path,
        key,
        mutation_lock,
    )?;
    Ok(InterruptedMigration {
        temporary_path: temporary_path.to_path_buf(),
        backup_path,
        retained_artifact_paths: Vec::new(),
    })
}

fn validate_interrupted_paths(
    canonical_path: &Path,
    source_path: &Path,
    backup_path: &Path,
    key: PageCryptoKey,
    mutation_lock: &mut StateMutationLock,
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
    mutation_lock: &mut StateMutationLock,
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

pub(super) fn existing_source_path(canonical_path: &Path) -> Result<Option<PathBuf>, String> {
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
