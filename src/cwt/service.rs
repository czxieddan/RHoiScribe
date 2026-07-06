//------------------------------------------------------------------------------------
// service.rs -- Part of RHoiScribe
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
    collections::HashMap,
    error::Error,
    fmt,
    sync::{Arc, RwLock},
};

use super::workspace::{
    CwtWorkspaceConfig, CwtWorkspaceError, CwtWorkspaceHandle, CwtWorkspaceStatus,
    workspace_handle_id,
};

#[derive(Default)]
pub struct CwtLanguageService {
    workspaces: RwLock<HashMap<String, Arc<CwtWorkspaceHandle>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CwtMemoryActionReport {
    pub action: CwtMemoryAction,
    pub handle_id: Option<String>,
    pub affected_workspace_count: usize,
    pub status: Option<CwtWorkspaceStatus>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CwtMemoryAction {
    ReloadRules,
    RefreshWorkspace,
    ClearWorkspace,
    ClearAll,
    RebuildVanilla,
}

#[derive(Debug)]
pub enum CwtLanguageServiceError {
    RegistryLockPoisoned,
    UnknownWorkspace(String),
    Workspace(CwtWorkspaceError),
}

impl CwtLanguageService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_workspace(
        &self,
        config: CwtWorkspaceConfig,
    ) -> Result<Arc<CwtWorkspaceHandle>, CwtLanguageServiceError> {
        let id = workspace_handle_id(&config);
        let handle = {
            let mut workspaces = self
                .workspaces
                .write()
                .map_err(|_| CwtLanguageServiceError::RegistryLockPoisoned)?;
            workspaces
                .entry(id.clone())
                .or_insert_with(|| Arc::new(CwtWorkspaceHandle::new(id, config)))
                .clone()
        };
        handle
            .refresh()
            .map_err(CwtLanguageServiceError::Workspace)?;
        Ok(handle)
    }

    pub fn open_workspace_blocking(
        &self,
        config: CwtWorkspaceConfig,
    ) -> Result<Arc<CwtWorkspaceHandle>, CwtLanguageServiceError> {
        let id = workspace_handle_id(&config);
        let handle = {
            let mut workspaces = self
                .workspaces
                .write()
                .map_err(|_| CwtLanguageServiceError::RegistryLockPoisoned)?;
            workspaces
                .entry(id.clone())
                .or_insert_with(|| Arc::new(CwtWorkspaceHandle::new(id, config)))
                .clone()
        };
        handle
            .refresh_blocking()
            .map_err(CwtLanguageServiceError::Workspace)?;
        Ok(handle)
    }

    pub fn get_workspace(
        &self,
        handle_id: &str,
    ) -> Result<Option<Arc<CwtWorkspaceHandle>>, CwtLanguageServiceError> {
        let workspaces = self
            .workspaces
            .read()
            .map_err(|_| CwtLanguageServiceError::RegistryLockPoisoned)?;
        Ok(workspaces.get(handle_id).cloned())
    }

    pub fn list_workspace_statuses(
        &self,
    ) -> Result<Vec<CwtWorkspaceStatus>, CwtLanguageServiceError> {
        let workspaces = self
            .workspaces
            .read()
            .map_err(|_| CwtLanguageServiceError::RegistryLockPoisoned)?;
        workspaces
            .values()
            .map(|handle| handle.status().map_err(CwtLanguageServiceError::Workspace))
            .collect()
    }

    pub fn clear(&self) -> Result<usize, CwtLanguageServiceError> {
        let mut workspaces = self
            .workspaces
            .write()
            .map_err(|_| CwtLanguageServiceError::RegistryLockPoisoned)?;
        let cleared = workspaces.len();
        workspaces.clear();
        Ok(cleared)
    }

    pub fn reload_workspace_rules(
        &self,
        handle_id: &str,
    ) -> Result<CwtMemoryActionReport, CwtLanguageServiceError> {
        self.refresh_handle(handle_id, CwtMemoryAction::ReloadRules)
    }

    pub fn refresh_workspace(
        &self,
        handle_id: &str,
    ) -> Result<CwtMemoryActionReport, CwtLanguageServiceError> {
        self.refresh_handle(handle_id, CwtMemoryAction::RefreshWorkspace)
    }

    pub fn rebuild_vanilla_memory(
        &self,
        handle_id: &str,
    ) -> Result<CwtMemoryActionReport, CwtLanguageServiceError> {
        self.refresh_handle(handle_id, CwtMemoryAction::RebuildVanilla)
    }

    pub fn clear_workspace(
        &self,
        handle_id: &str,
    ) -> Result<CwtMemoryActionReport, CwtLanguageServiceError> {
        let removed = {
            let mut workspaces = self
                .workspaces
                .write()
                .map_err(|_| CwtLanguageServiceError::RegistryLockPoisoned)?;
            workspaces.remove(handle_id).is_some()
        };

        if !removed {
            return Err(CwtLanguageServiceError::UnknownWorkspace(
                handle_id.to_string(),
            ));
        }

        Ok(CwtMemoryActionReport {
            action: CwtMemoryAction::ClearWorkspace,
            handle_id: Some(handle_id.to_string()),
            affected_workspace_count: 1,
            status: None,
            message: "cleared CWT workspace memory".to_string(),
        })
    }

    pub fn clear_all_workspace_memory(
        &self,
    ) -> Result<CwtMemoryActionReport, CwtLanguageServiceError> {
        let affected_workspace_count = self.clear()?;
        Ok(CwtMemoryActionReport {
            action: CwtMemoryAction::ClearAll,
            handle_id: None,
            affected_workspace_count,
            status: None,
            message: "cleared all CWT workspace memory".to_string(),
        })
    }

    fn refresh_handle(
        &self,
        handle_id: &str,
        action: CwtMemoryAction,
    ) -> Result<CwtMemoryActionReport, CwtLanguageServiceError> {
        let handle = self.workspace_or_error(handle_id)?;
        handle
            .refresh()
            .map_err(CwtLanguageServiceError::Workspace)?;
        let status = handle
            .status()
            .map_err(CwtLanguageServiceError::Workspace)?;
        Ok(CwtMemoryActionReport {
            action,
            handle_id: Some(handle_id.to_string()),
            affected_workspace_count: 1,
            status: Some(status),
            message: "scheduled CWT workspace warm refresh".to_string(),
        })
    }

    fn workspace_or_error(
        &self,
        handle_id: &str,
    ) -> Result<Arc<CwtWorkspaceHandle>, CwtLanguageServiceError> {
        self.get_workspace(handle_id)?
            .ok_or_else(|| CwtLanguageServiceError::UnknownWorkspace(handle_id.to_string()))
    }
}

impl fmt::Debug for CwtLanguageService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CwtLanguageService")
            .field("workspaces", &"RwLock<HashMap<String, CwtWorkspaceHandle>>")
            .finish()
    }
}

impl fmt::Display for CwtLanguageServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CwtLanguageServiceError::RegistryLockPoisoned => {
                write!(
                    formatter,
                    "CWT language workspace registry lock is poisoned"
                )
            }
            CwtLanguageServiceError::UnknownWorkspace(handle_id) => {
                write!(formatter, "unknown CWT workspace `{}`", handle_id)
            }
            CwtLanguageServiceError::Workspace(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for CwtLanguageServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CwtLanguageServiceError::Workspace(error) => Some(error),
            _ => None,
        }
    }
}
