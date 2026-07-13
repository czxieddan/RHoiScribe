//------------------------------------------------------------------------------------
// state/path.rs -- Part of RHoiScribe
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
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use rnmdb_cli::page_key_from_hex;
use rnmdb_storage::PageCryptoKey;
use sha2::{Digest, Sha256};

pub(crate) const STATE_DATABASE_FILE_NAME: &str = "state.rnmdb";
pub(crate) const LEGACY_STATE_DATABASE_FILE_NAME: &str = "rhoiscribe-state.rnmdb";
pub(crate) const PAGE_KEY_FILE_NAME: &str = "rnmdb-page.key";

const LOCK_RETRY_COUNT: usize = 250;
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(20);

pub(crate) fn default_rhoiscribe_dir() -> PathBuf {
    user_home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".rhoiscribe")
}

pub(crate) fn state_store_path(store_path: Option<&str>) -> PathBuf {
    let path = store_path
        .map(clean_input_path)
        .unwrap_or_else(|| default_rhoiscribe_dir().join(STATE_DATABASE_FILE_NAME));
    canonical_state_database_path(path)
}

fn clean_input_path(path: &str) -> PathBuf {
    PathBuf::from(path.trim().trim_matches('"'))
}

pub(crate) fn canonical_state_database_path(path: PathBuf) -> PathBuf {
    if file_name_matches(&path, LEGACY_STATE_DATABASE_FILE_NAME) {
        return path.with_file_name(STATE_DATABASE_FILE_NAME);
    }
    path
}

pub(crate) fn legacy_state_database_path(path: &Path) -> PathBuf {
    if file_name_matches(path, STATE_DATABASE_FILE_NAME) {
        return path.with_file_name(LEGACY_STATE_DATABASE_FILE_NAME);
    }
    path.to_path_buf()
}

fn file_name_matches(path: &Path, expected: &str) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(expected))
}

pub(crate) fn clean_display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

pub(crate) fn ensure_parent(path: &Path) -> Result<(), String> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(fs::create_dir_all)
        .transpose()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub(crate) fn sync_parent_directory(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    sync_directory(parent).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(windows)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?
        .sync_all()
}

#[cfg(not(any(unix, windows)))]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

pub(crate) fn page_crypto_key() -> Result<PageCryptoKey, String> {
    let path = default_rhoiscribe_dir().join(PAGE_KEY_FILE_NAME);
    match read_page_key(&path)? {
        Some(key) => Ok(key),
        None => create_page_key(&path),
    }
}

pub(crate) fn existing_page_crypto_key() -> Result<PageCryptoKey, String> {
    let path = default_rhoiscribe_dir().join(PAGE_KEY_FILE_NAME);
    read_page_key(&path)?.ok_or_else(|| {
        format!(
            "existing RNMDB page key is missing at {}",
            clean_display_path(&path)
        )
    })
}

fn read_page_key(path: &Path) -> Result<Option<PageCryptoKey>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read RNMDB page key at {}: {error}",
            clean_display_path(path)
        )
    })?;
    page_key_from_hex(content.trim())
        .map(Some)
        .map_err(|error| {
            format!(
                "RNMDB page key at {} is invalid: {error}",
                clean_display_path(path)
            )
        })
}

fn create_page_key(path: &Path) -> Result<PageCryptoKey, String> {
    ensure_parent(path)?;
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|error| error.to_string())?;
    match write_new_page_key(path, &bytes) {
        Ok(()) => Ok(PageCryptoKey::from_bytes(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            concurrent_page_key(path)
        }
        Err(error) => Err(error.to_string()),
    }
}

fn concurrent_page_key(path: &Path) -> Result<PageCryptoKey, String> {
    read_page_key(path)?
        .ok_or_else(|| "RNMDB page key was created concurrently but could not be read".to_string())
}

fn write_new_page_key(path: &Path, bytes: &[u8; 32]) -> std::io::Result<()> {
    let mut file = new_key_file(path)?;
    file.write_all(hex::encode(bytes).as_bytes())?;
    file.write_all(b"\n")?;
    secure_key_permissions(&file)?;
    file.sync_all()
}

fn new_key_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(unix)]
fn secure_key_permissions(file: &File) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn secure_key_permissions(_file: &File) -> std::io::Result<()> {
    Ok(())
}

pub(crate) struct StateMutationLock {
    files: Vec<File>,
    database_identities: Vec<LockedDatabaseIdentity>,
}

struct LockedDatabaseIdentity {
    key: String,
    _handle: same_file::Handle,
}

impl StateMutationLock {
    pub(crate) fn acquire(store_path: &Path) -> Result<Self, String> {
        reject_reserved_state_path(store_path)?;
        let path = path_lock_path(store_path)?;
        let file = acquire_named_lock(&path)?;
        let mut state_lock = Self {
            files: vec![file],
            database_identities: Vec::new(),
        };
        state_lock.bind_existing_database(store_path)?;
        let legacy_path = legacy_state_database_path(store_path);
        if legacy_path != store_path {
            state_lock.bind_existing_database(&legacy_path)?;
        }
        Ok(state_lock)
    }

    pub(crate) fn bind_existing_database(&mut self, store_path: &Path) -> Result<(), String> {
        let Some(metadata) = database_path_metadata(store_path)? else {
            return Ok(());
        };
        validate_database_file_type(store_path, &metadata)?;
        if !metadata.file_type().is_file() {
            return Ok(());
        }
        let identity = same_file::Handle::from_path(store_path).map_err(|error| {
            format!(
                "failed to identify state database at {}: {error}",
                clean_display_path(store_path)
            )
        })?;
        self.bind_database_identity(store_path, identity)
    }

    fn bind_database_identity(
        &mut self,
        store_path: &Path,
        identity: same_file::Handle,
    ) -> Result<(), String> {
        let (kind, key) = stable_identity_key(store_path, &identity)?;
        let lock_key = format!("{kind}:{key}");
        if self
            .database_identities
            .iter()
            .any(|existing| existing.key == lock_key)
        {
            return Ok(());
        }
        let path = named_lock_path(kind, &key);
        self.files.push(acquire_named_lock(&path)?);
        self.database_identities.push(LockedDatabaseIdentity {
            key: lock_key,
            _handle: identity,
        });
        Ok(())
    }
}

impl Drop for StateMutationLock {
    fn drop(&mut self) {
        for file in self.files.iter_mut().rev() {
            let _ = file.unlock();
        }
    }
}

fn path_lock_path(store_path: &Path) -> Result<PathBuf, String> {
    ensure_parent(store_path)?;
    let parent = store_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent).map_err(|error| {
        format!(
            "failed to canonicalize state database parent {}: {error}",
            clean_display_path(parent)
        )
    })?;
    let file_name = store_path
        .file_name()
        .ok_or_else(|| "state database path must include a file name".to_string())?;
    let normalized = parent.join(lock_file_name(file_name));
    Ok(named_lock_path(
        "path-v1",
        &hex::encode(Sha256::digest(path_lock_bytes(&normalized))),
    ))
}

#[cfg(windows)]
fn lock_file_name(file_name: &std::ffi::OsStr) -> std::ffi::OsString {
    file_name.to_string_lossy().to_lowercase().into()
}

#[cfg(not(windows))]
fn lock_file_name(file_name: &std::ffi::OsStr) -> std::ffi::OsString {
    file_name.to_os_string()
}

fn named_lock_path(kind: &str, key: &str) -> PathBuf {
    default_rhoiscribe_dir()
        .join("state-locks")
        .join(format!("{kind}-{key}.lock"))
}

#[cfg(unix)]
fn path_lock_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_lock_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn path_lock_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

#[cfg(unix)]
fn stable_identity_key(
    _store_path: &Path,
    identity: &same_file::Handle,
) -> Result<(&'static str, String), String> {
    Ok((
        "file-v1-unix",
        format!("{:016x}-{:016x}", identity.dev(), identity.ino()),
    ))
}

#[cfg(windows)]
fn stable_identity_key(
    store_path: &Path,
    _identity: &same_file::Handle,
) -> Result<(&'static str, String), String> {
    let identity = file_id::get_high_res_file_id(store_path).map_err(|error| {
        format!(
            "failed to read stable state database identity at {}: {error}",
            clean_display_path(store_path)
        )
    })?;
    let file_id::FileId::HighRes {
        volume_serial_number,
        file_id,
    } = identity
    else {
        return Err("stable state database identity was not high resolution".to_string());
    };
    Ok((
        "file-v1-windows",
        format!(
            "{volume_serial_number:016x}-{}",
            hex::encode(file_id.to_le_bytes())
        ),
    ))
}

#[cfg(not(any(unix, windows)))]
fn stable_identity_key(
    _store_path: &Path,
    _identity: &same_file::Handle,
) -> Result<(&'static str, String), String> {
    Err("stable state database identity locks are unsupported on this platform".to_string())
}

fn reject_reserved_state_path(path: &Path) -> Result<(), String> {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if [".migrating-sql-v2", ".legacy-format-upgrade", ".pre-sql-v2"]
        .iter()
        .any(|label| file_name.contains(label))
    {
        return Err(format!(
            "state database path {} is reserved for migration recovery",
            clean_display_path(path)
        ));
    }
    Ok(())
}

fn database_path_metadata(path: &Path) -> Result<Option<fs::Metadata>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "failed to inspect state database path {}: {error}",
            clean_display_path(path)
        )),
    }
}

fn validate_database_file_type(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "state database path {} must not be a symbolic link",
            clean_display_path(path)
        ));
    }
    Ok(())
}

fn acquire_named_lock(path: &Path) -> Result<File, String> {
    ensure_parent(path)?;
    acquire_lock_file(path).map_err(|error| {
        format!(
            "failed to acquire state store lock at {}: {error}",
            clean_display_path(path)
        )
    })
}

fn acquire_lock_file(path: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|error| error.to_string())?;
    for _ in 0..LOCK_RETRY_COUNT {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) => {
                thread::sleep(LOCK_RETRY_DELAY);
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(error.to_string()),
        }
    }
    Err(format!(
        "timed out waiting for state store lock at {}",
        clean_display_path(path)
    ))
}
