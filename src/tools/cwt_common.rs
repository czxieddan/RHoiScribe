//------------------------------------------------------------------------------------
// cwt_common.rs -- Part of RHoiScribe
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

use std::{path::PathBuf, sync::Arc};

use crate::{
    RhoiScribeRuntime,
    cwt::workspace::{CwtWorkspaceHandle, CwtWorkspaceSnapshot},
};

use super::ToolError;

pub(super) fn workspace_snapshot_from_handle(
    runtime: &Arc<RhoiScribeRuntime>,
    handle_id: &str,
) -> Result<Arc<CwtWorkspaceSnapshot>, ToolError> {
    let handle = runtime
        .cwt_language()
        .get_workspace(handle_id)
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
        .ok_or_else(|| ToolError::InvalidRequest(format!("unknown CWT workspace `{handle_id}`")))?;

    ensure_snapshot_is_warm(&handle)?;
    handle
        .snapshot()
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
        .ok_or_else(|| ToolError::InvalidRequest("CWT workspace has no warm snapshot".to_string()))
}

pub(super) fn workspace_root_from_handle(
    runtime: &Arc<RhoiScribeRuntime>,
    handle_id: &str,
) -> Result<PathBuf, ToolError> {
    workspace_snapshot_from_handle(runtime, handle_id)
        .map(|snapshot| snapshot.workspace_root.clone())
}

fn ensure_snapshot_is_warm(handle: &Arc<CwtWorkspaceHandle>) -> Result<(), ToolError> {
    if handle
        .snapshot()
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
        .is_none()
    {
        handle
            .refresh_blocking()
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?;
    }

    Ok(())
}
