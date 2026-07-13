//------------------------------------------------------------------------------------
// state/legacy/artifacts.rs -- Part of RHoiScribe
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
    fs, io,
    path::{Path, PathBuf},
};

use rnmdb_storage::{PageCryptoKey, verify_single_file_with_key};

use super::super::{is_state_database_error, path::sync_parent_directory, state_database_error};

pub(super) const SQL_MIGRATION_LABEL: &str = "migrating-sql-v2";
pub(super) const FORMAT_UPGRADE_LABEL: &str = "legacy-format-upgrade";

pub(super) struct MigrationTemporary {
    path: PathBuf,
}

impl MigrationTemporary {
    pub(super) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

pub(super) fn validate_temporary_path(
    path: &Path,
    canonical_path: &Path,
    stage: &str,
) -> Result<(), String> {
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

pub(super) fn retain_created_migration(
    temporary: &MigrationTemporary,
    canonical_path: &Path,
    error: String,
) -> String {
    retain_migration(temporary.path(), canonical_path, error)
}

pub(super) fn retain_unowned_migration(
    path: &Path,
    canonical_path: &Path,
    error: String,
) -> String {
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

pub(super) fn verify_authenticated(
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

pub(super) fn sync_verified_temporary(path: &Path, canonical_path: &Path) -> Result<(), String> {
    sync_parent_directory(path)
        .map_err(|error| state_database_error(canonical_path, "verify", error))
}

pub(super) fn unique_backup_path(canonical_path: &Path) -> Result<PathBuf, String> {
    unique_sibling_path(canonical_path, "pre-sql-v2", "swap")
}

pub(super) fn temporary_path(canonical_path: &Path, label: &str) -> Result<PathBuf, String> {
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

pub(super) fn reject_existing_target(
    canonical_path: &Path,
    target: &Path,
    stage: &str,
) -> Result<(), String> {
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

pub(super) fn path_entry_exists(path: &Path) -> Result<bool, String> {
    path_entry_metadata(path).map(|metadata| metadata.is_some())
}

pub(super) fn path_entry_metadata(path: &Path) -> Result<Option<fs::Metadata>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "failed to inspect path entry {}: {error}",
            path.to_string_lossy()
        )),
    }
}

#[cfg(windows)]
pub(super) fn paths_match(left: &Path, right: &Path) -> bool {
    super::super::path::clean_display_path(left)
        .eq_ignore_ascii_case(&super::super::path::clean_display_path(right))
}

#[cfg(not(windows))]
pub(super) fn paths_match(left: &Path, right: &Path) -> bool {
    left == right
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "redox"
))]
pub(super) fn rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, source, CWD, destination, RenameFlags::NOREPLACE).map_err(io::Error::from)
}

#[cfg(windows)]
pub(super) fn rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    atomicwrites::move_atomic(source, destination)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "redox",
    windows
)))]
pub(super) fn rename_no_replace(_source: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace state migration is unsupported on this platform",
    ))
}
