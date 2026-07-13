//------------------------------------------------------------------------------------
// maintenance.rs -- Part of RHoiScribe
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
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use rnmdb_catalog::{Catalog, CatalogCodec, Column, Table};
use rnmdb_executor::{
    durable::{DurableExecutorImage, DurableTableRows, read_image_from_single_file_backend},
    row::RowCodec,
    vector::ColumnSchema,
};
use rnmdb_storage::{
    PageCryptoKey, SingleFileBackend, backup_single_file, inspect_single_file,
    inspect_single_file_with_key, verify_single_file_with_key,
};
use rnmdb_types::SqlValue;

use super::{
    StateMutationLock, clean_display_path,
    path::{existing_page_crypto_key, state_store_path},
    state_database_error,
};

const SCHEMA_VERSION_METADATA: &str = "schema_version";
const RNMDB_REVISION_METADATA: &str = "rnmdb_revision";
const MIGRATION_SOURCE_METADATA: &str = "last_migration_source_path";
const MIGRATION_BACKUP_METADATA: &str = "last_migration_backup_path";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateInspectionReport {
    pub(crate) database_path: PathBuf,
    pub(crate) schema_version: u32,
    pub(crate) rnmdb_revision: String,
    pub(crate) format_version: u16,
    pub(crate) page_size_bytes: usize,
    pub(crate) file_len_bytes: u64,
    pub(crate) present_page_records: u64,
    pub(crate) superblock_generation: u64,
    pub(crate) catalog_root: u64,
    pub(crate) deep_verification_performed: bool,
    pub(crate) verification_valid: bool,
    pub(crate) encryption_authenticated: bool,
    pub(crate) authenticated_page_records: u64,
    pub(crate) last_migration_source_path: Option<String>,
    pub(crate) last_migration_backup_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateBackupReport {
    pub(crate) applied: bool,
    pub(crate) source_path: PathBuf,
    pub(crate) destination_path: PathBuf,
    pub(crate) planned_bytes: u64,
    pub(crate) bytes_copied: Option<u64>,
    pub(crate) page_size_bytes: usize,
    pub(crate) present_page_records: u64,
    pub(crate) superblock_generation: u64,
    pub(crate) verification_valid: bool,
    pub(crate) encryption_authenticated: bool,
}

struct StateSnapshot {
    inspection: StateInspectionReport,
    key: PageCryptoKey,
}

struct StateMetadata {
    schema_version: u32,
    rnmdb_revision: String,
    last_migration_source_path: Option<String>,
    last_migration_backup_path: Option<String>,
}

pub(crate) fn inspect_state(
    store_path: Option<&str>,
    deep_verify: bool,
) -> Result<StateInspectionReport, String> {
    let path = state_store_path(store_path);
    inspect_state_path(&path, deep_verify).map(|snapshot| snapshot.inspection)
}

pub(crate) fn backup_state(
    store_path: Option<&str>,
    destination: &str,
    apply: bool,
) -> Result<StateBackupReport, String> {
    let source = state_store_path(store_path);
    let destination = backup_destination(destination)?;
    validate_destination(&source, &destination)?;
    if !apply {
        let snapshot = inspect_state_path(&source, true)?;
        return Ok(planned_backup(snapshot.inspection, destination));
    }
    apply_backup(&source, destination)
}

fn inspect_state_path(path: &Path, deep_verify: bool) -> Result<StateSnapshot, String> {
    validate_state_source(path)?;
    let inspection = inspect_single_file(path)
        .map_err(|error| state_database_error(path, "inspect", error.to_string()))?;
    let key =
        existing_page_crypto_key().map_err(|error| state_database_error(path, "inspect", error))?;
    let (backend, catalog_root) = open_sql_backend(path, key)?;
    let metadata = read_state_metadata(path, &backend)?;
    let verification = verification_status(path, key, deep_verify, &inspection)?;
    Ok(StateSnapshot {
        inspection: StateInspectionReport {
            database_path: path.to_path_buf(),
            schema_version: metadata.schema_version,
            rnmdb_revision: metadata.rnmdb_revision,
            format_version: inspection.format_version(),
            page_size_bytes: inspection.page_size().bytes(),
            file_len_bytes: inspection.file_len_bytes(),
            present_page_records: inspection.present_page_records(),
            superblock_generation: inspection.superblock_generation(),
            catalog_root,
            deep_verification_performed: deep_verify,
            verification_valid: verification.valid,
            encryption_authenticated: verification.authenticated,
            authenticated_page_records: verification.authenticated_pages,
            last_migration_source_path: metadata.last_migration_source_path,
            last_migration_backup_path: metadata.last_migration_backup_path,
        },
        key,
    })
}

fn validate_state_source(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| state_database_error(path, "inspect", error.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(state_database_error(
            path,
            "inspect",
            "state database path must be an existing regular file, not a directory or symlink",
        ));
    }
    Ok(())
}

fn open_sql_backend(path: &Path, key: PageCryptoKey) -> Result<(SingleFileBackend, u64), String> {
    let backend = SingleFileBackend::open_with_key(path, key)
        .map_err(|error| state_database_error(path, "inspect", error.to_string()))?;
    let catalog_root = backend.catalog_root().map(|page_id| page_id.get()).ok_or_else(|| {
        state_database_error(
            path,
            "inspect",
            "state database has no SQL catalog root; maintenance inspection never migrates legacy state",
        )
    })?;
    Ok((backend, catalog_root))
}

fn read_state_metadata(path: &Path, backend: &SingleFileBackend) -> Result<StateMetadata, String> {
    let image = read_durable_image(path, backend)?;
    let catalog = decode_catalog(path, &image)?;
    let table = state_metadata_table(path, &catalog)?;
    let durable_rows = metadata_durable_rows(path, &image)?;
    decode_state_metadata(path, table, durable_rows)
}

fn read_durable_image(
    path: &Path,
    backend: &SingleFileBackend,
) -> Result<DurableExecutorImage, String> {
    let bytes = read_image_from_single_file_backend(backend)
        .map_err(|error| state_database_error(path, "query", error.to_string()))?;
    let bytes = bytes
        .ok_or_else(|| state_database_error(path, "query", "durable RNMDB SQL image is missing"))?;
    DurableExecutorImage::decode(&bytes)
        .map_err(|error| state_database_error(path, "query", error.to_string()))
}

fn decode_catalog(path: &Path, image: &DurableExecutorImage) -> Result<Catalog, String> {
    CatalogCodec::decode(image.catalog())
        .map_err(|error| state_database_error(path, "query", error.to_string()))
}

fn state_metadata_table<'a>(path: &Path, catalog: &'a Catalog) -> Result<&'a Table, String> {
    catalog
        .get_table("public", "state_metadata")
        .ok_or_else(|| {
            state_database_error(
                path,
                "query",
                "state_metadata table is missing from the catalog",
            )
        })
}

fn metadata_durable_rows<'a>(
    path: &Path,
    image: &'a DurableExecutorImage,
) -> Result<&'a DurableTableRows, String> {
    let mut matches = image
        .tables()
        .iter()
        .filter(|table| matches!(table.name(), "state_metadata" | "public.state_metadata"));
    let rows = matches.next().ok_or_else(|| {
        state_database_error(path, "query", "durable state_metadata rows are missing")
    })?;
    if matches.next().is_some() {
        return Err(state_database_error(
            path,
            "query",
            "durable state_metadata rows are ambiguous",
        ));
    }
    Ok(rows)
}

fn decode_state_metadata(
    path: &Path,
    table: &Table,
    rows: &DurableTableRows,
) -> Result<StateMetadata, String> {
    let columns = metadata_columns(path, table)?;
    let mut values = BTreeMap::new();
    for encoded in rows.rows() {
        let row = RowCodec::decode(&columns, encoded)
            .map_err(|error| state_database_error(path, "query", error.to_string()))?;
        let name = metadata_text(row.values(), 0, "state_metadata.name")
            .map_err(|error| state_database_error(path, "query", error))?;
        let value = metadata_text(row.values(), 1, "state_metadata.value")
            .map_err(|error| state_database_error(path, "query", error))?;
        if values.insert(name.clone(), value).is_some() {
            return Err(state_database_error(
                path,
                "query",
                format!("duplicate state metadata key {name}"),
            ));
        }
    }
    build_state_metadata(path, values)
}

fn metadata_columns(path: &Path, table: &Table) -> Result<Vec<ColumnSchema>, String> {
    table
        .columns()
        .iter()
        .map(|column| metadata_column(path, column))
        .collect()
}

fn metadata_column(path: &Path, column: &Column) -> Result<ColumnSchema, String> {
    if column.generated_expr().is_some() {
        return Err(state_database_error(
            path,
            "query",
            "state_metadata unexpectedly contains a generated column",
        ));
    }
    let mut schema = ColumnSchema::new(column.name(), column.data_type().clone());
    if !column.nullable() {
        schema = schema.not_null();
    }
    Ok(schema.with_encrypted(column.is_encrypted()))
}

fn metadata_text(values: &[SqlValue], index: usize, label: &str) -> Result<String, String> {
    match values.get(index) {
        Some(SqlValue::Text(value)) => Ok(value.clone()),
        _ => Err(format!("{label} is missing or not TEXT")),
    }
}

fn build_state_metadata(
    path: &Path,
    mut values: BTreeMap<String, String>,
) -> Result<StateMetadata, String> {
    let schema_version = take_required_metadata(path, &mut values, SCHEMA_VERSION_METADATA)?
        .parse::<u32>()
        .map_err(|error| state_database_error(path, "query", error.to_string()))?;
    let rnmdb_revision = take_required_metadata(path, &mut values, RNMDB_REVISION_METADATA)?;
    Ok(StateMetadata {
        schema_version,
        rnmdb_revision,
        last_migration_source_path: values.remove(MIGRATION_SOURCE_METADATA),
        last_migration_backup_path: values.remove(MIGRATION_BACKUP_METADATA),
    })
}

fn take_required_metadata(
    path: &Path,
    values: &mut BTreeMap<String, String>,
    name: &str,
) -> Result<String, String> {
    values.remove(name).ok_or_else(|| {
        state_database_error(path, "query", format!("state metadata {name} is missing"))
    })
}

struct VerificationStatus {
    valid: bool,
    authenticated: bool,
    authenticated_pages: u64,
}

fn verification_status(
    path: &Path,
    key: PageCryptoKey,
    deep_verify: bool,
    inspection: &rnmdb_storage::SingleFileInspection,
) -> Result<VerificationStatus, String> {
    if !deep_verify {
        return Ok(VerificationStatus {
            valid: inspection.superblock_checksum_verified(),
            authenticated: false,
            authenticated_pages: 0,
        });
    }
    let authenticated = inspect_single_file_with_key(path, key)
        .map_err(|error| state_database_error(path, "verify", error.to_string()))?;
    let verification = verify_single_file_with_key(path, key)
        .map_err(|error| state_database_error(path, "verify", error.to_string()))?;
    require_authenticated_verification(path, &verification)?;
    Ok(VerificationStatus {
        valid: true,
        authenticated: true,
        authenticated_pages: authenticated.authenticated_page_records(),
    })
}

fn require_authenticated_verification(
    path: &Path,
    verification: &rnmdb_storage::SingleFileVerificationReport,
) -> Result<(), String> {
    if verification.is_valid() && verification.encryption_authenticated() {
        return Ok(());
    }
    Err(state_database_error(
        path,
        "verify",
        "authenticated RNMDB verification did not report a valid encrypted database",
    ))
}

fn backup_destination(destination: &str) -> Result<PathBuf, String> {
    let cleaned = destination.trim().trim_matches('"');
    if cleaned.is_empty() {
        return Err("backup destination must be a non-empty explicit path".to_string());
    }
    let path = PathBuf::from(cleaned);
    if path.file_name().is_none() {
        return Err("backup destination must include a file name".to_string());
    }
    Ok(path)
}

fn validate_destination(source: &Path, destination: &Path) -> Result<(), String> {
    reject_existing_destination(destination)?;
    let parent = destination_parent(destination);
    let metadata = fs::metadata(parent).map_err(|error| {
        format!(
            "backup destination parent {} must already exist: {error}",
            clean_display_path(parent)
        )
    })?;
    if !metadata.is_dir() {
        return Err(format!(
            "backup destination parent {} is not a directory",
            clean_display_path(parent)
        ));
    }
    reject_source_alias(source, destination, parent)
}

fn reject_existing_destination(destination: &Path) -> Result<(), String> {
    match fs::symlink_metadata(destination) {
        Ok(_) => Err(format!(
            "backup destination {} already exists; overwrite is not allowed",
            clean_display_path(destination)
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to inspect backup destination {}: {error}",
            clean_display_path(destination)
        )),
    }
}

fn destination_parent(destination: &Path) -> &Path {
    destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn reject_source_alias(source: &Path, destination: &Path, parent: &Path) -> Result<(), String> {
    let source = fs::canonicalize(source)
        .map_err(|error| state_database_error(source, "inspect", error.to_string()))?;
    let parent = fs::canonicalize(parent).map_err(|error| {
        format!(
            "failed to resolve backup destination parent {}: {error}",
            clean_display_path(parent)
        )
    })?;
    let destination = parent.join(
        destination
            .file_name()
            .expect("destination file name checked"),
    );
    if paths_match(&source, &destination) {
        return Err("backup source and destination must be different paths".to_string());
    }
    Ok(())
}

#[cfg(windows)]
fn paths_match(left: &Path, right: &Path) -> bool {
    clean_display_path(left).eq_ignore_ascii_case(&clean_display_path(right))
}

#[cfg(not(windows))]
fn paths_match(left: &Path, right: &Path) -> bool {
    left == right
}

fn planned_backup(inspection: StateInspectionReport, destination: PathBuf) -> StateBackupReport {
    StateBackupReport {
        applied: false,
        source_path: inspection.database_path,
        destination_path: destination,
        planned_bytes: inspection.file_len_bytes,
        bytes_copied: None,
        page_size_bytes: inspection.page_size_bytes,
        present_page_records: inspection.present_page_records,
        superblock_generation: inspection.superblock_generation,
        verification_valid: inspection.verification_valid,
        encryption_authenticated: inspection.encryption_authenticated,
    }
}

fn apply_backup(source: &Path, destination: PathBuf) -> Result<StateBackupReport, String> {
    let _lock = StateMutationLock::acquire(source)
        .map_err(|error| state_database_error(source, "backup", error))?;
    validate_destination(source, &destination)?;
    let snapshot = inspect_state_path(source, true)?;
    let report = backup_single_file(source, &destination).map_err(|error| {
        state_database_error(
            source,
            "backup",
            format!(
                "{error}; the destination may contain a partial backup and was retained for manual inspection"
            ),
        )
    })?;
    verify_created_backup(&destination, snapshot.key)?;
    Ok(StateBackupReport {
        applied: true,
        source_path: source.to_path_buf(),
        destination_path: destination,
        planned_bytes: snapshot.inspection.file_len_bytes,
        bytes_copied: Some(report.bytes_copied()),
        page_size_bytes: report.page_size().bytes(),
        present_page_records: report.present_page_records(),
        superblock_generation: report.superblock_generation(),
        verification_valid: true,
        encryption_authenticated: true,
    })
}

fn verify_created_backup(destination: &Path, key: PageCryptoKey) -> Result<(), String> {
    let verification = verify_single_file_with_key(destination, key);
    let detail = match verification {
        Ok(report) if report.is_valid() && report.encryption_authenticated() => return Ok(()),
        Ok(_) => "authenticated verification reported an invalid backup".to_string(),
        Err(error) => error.to_string(),
    };
    Err(state_database_error(
        destination,
        "verify backup",
        format!(
            "{detail}; the unverified destination was retained because deleting a path after verification failure could remove a concurrently replaced file"
        ),
    ))
}
