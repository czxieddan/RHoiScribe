//------------------------------------------------------------------------------------
// rnmdb_store.rs -- Part of RHoiScribe
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

use rchadow::rnmdb::{RnmdbPageStore, database_error};
use rnmdb_common::ids::PageId;
use rnmdb_storage::{Page, PageSize, SingleFileBackend, SingleFileOptions, StorageBackend};

pub(crate) use crate::state::path::{clean_display_path, default_rhoiscribe_dir};
use crate::state::{
    StateMutationLock,
    legacy::is_legacy_state_page,
    path::{
        LEGACY_STATE_DATABASE_FILE_NAME, STATE_DATABASE_FILE_NAME, ensure_parent, page_crypto_key,
    },
};

pub(crate) const DEFAULT_RNMDB_PAGE_SIZE_BYTES: usize = 16 * 1024;

pub(crate) struct RnmdbSingleFilePageStore {
    backend: SingleFileBackend,
    page_size_bytes: usize,
    _mutation_lock: StateMutationLock,
}

impl RnmdbSingleFilePageStore {
    pub(crate) fn open_or_create(path: &Path, page_size_bytes: usize) -> Result<Self, String> {
        Self::open_or_create_current(path, page_size_bytes)
    }

    fn open_or_create_current(path: &Path, page_size_bytes: usize) -> Result<Self, String> {
        reject_state_store_name(path)?;
        let mut mutation_lock = StateMutationLock::acquire(path)?;
        ensure_parent(path)?;
        let page_key = page_crypto_key()?;
        let backend = if path.exists() {
            let backend = SingleFileBackend::open_with_key(path, page_key)
                .map_err(|error| error.to_string())?;
            reject_rhoiscribe_state_backend(path, &backend)?;
            backend
        } else {
            SingleFileBackend::create(
                path,
                SingleFileOptions::new(PageSize::new(page_size_bytes)).with_page_key(page_key),
            )
            .map_err(|error| error.to_string())?
        };
        mutation_lock.bind_existing_database(path)?;
        let page_size_bytes = backend.page_size().bytes();

        Ok(Self {
            backend,
            page_size_bytes,
            _mutation_lock: mutation_lock,
        })
    }

    pub(crate) fn read_payload_page(&self, page_id: u64) -> Result<Option<Vec<u8>>, String> {
        self.backend
            .read_page(PageId::new(page_id))
            .map(|page| page.map(|page| page.payload().to_vec()))
            .map_err(|error| error.to_string())
    }

    pub(crate) fn write_payload_page(&self, page_id: u64, payload: Vec<u8>) -> Result<(), String> {
        if payload.len() != self.page_size_bytes {
            return Err(format!(
                "RNMDB page payload must be {} bytes, got {}",
                self.page_size_bytes,
                payload.len()
            ));
        }

        let page = Page::new(PageId::new(page_id), payload).map_err(|error| error.to_string())?;
        self.backend
            .write_page(page)
            .and_then(|_| self.backend.sync().map(|_| ()))
            .map_err(|error| error.to_string())
    }
}

fn reject_state_store_name(path: &Path) -> Result<(), String> {
    let reserved = path.file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        name.eq_ignore_ascii_case(STATE_DATABASE_FILE_NAME)
            || name.eq_ignore_ascii_case(LEGACY_STATE_DATABASE_FILE_NAME)
    });
    if reserved {
        return Err(format!(
            "Rchadow page storage must not use the RHoiScribe state database path {}",
            clean_display_path(path)
        ));
    }
    Ok(())
}

fn reject_rhoiscribe_state_backend(path: &Path, backend: &SingleFileBackend) -> Result<(), String> {
    let is_state = backend.catalog_root().is_some() || has_state_pages(backend)?;
    if is_state {
        return Err(format!(
            "Rchadow page storage refuses RHoiScribe state database {}",
            clean_display_path(path)
        ));
    }
    Ok(())
}

fn has_state_pages(backend: &SingleFileBackend) -> Result<bool, String> {
    for page_id in [1_u64, 2_u64] {
        let page = backend
            .read_page(PageId::new(page_id))
            .map_err(|error| error.to_string())?;
        if page.as_ref().is_some_and(|page| {
            (page_id == 1 && page.payload().starts_with(b"RNOVSI01"))
                || is_legacy_state_page(page_id, page.payload())
        }) {
            return Ok(true);
        }
    }
    Ok(false)
}

impl RnmdbPageStore for RnmdbSingleFilePageStore {
    fn page_size_bytes(&self) -> usize {
        self.page_size_bytes
    }

    fn read_page(&self, page_id: u64) -> rchadow::Result<Option<Vec<u8>>> {
        self.read_payload_page(page_id).map_err(database_error)
    }

    fn write_page(&mut self, page_id: u64, payload: Vec<u8>) -> rchadow::Result<()> {
        self.write_payload_page(page_id, payload)
            .map_err(database_error)
    }

    fn sync(&mut self) -> rchadow::Result<()> {
        self.backend
            .sync()
            .map(|_| ())
            .map_err(|error| database_error(error.to_string()))
    }
}
