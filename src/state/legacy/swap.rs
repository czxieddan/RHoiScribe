//------------------------------------------------------------------------------------
// state/legacy/swap.rs -- Part of RHoiScribe
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

use std::path::Path;

use super::{
    super::{StateMigrationReport, path::sync_parent_directory, state_database_error},
    artifacts::{
        MigrationTemporary, path_entry_exists, reject_existing_target, rename_no_replace,
        retain_created_migration,
    },
};

pub(super) fn promote_legacy_name(source: &Path, canonical_path: &Path) -> Result<(), String> {
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

pub(super) fn swap_database(
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
