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

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crate::{
    RhoiScribeRuntime,
    cwt::workspace::{CwtWorkspaceHandle, CwtWorkspaceSnapshot, CwtWorkspaceWarmState},
};

use super::ToolError;

const SNAPSHOT_WAIT_TIMEOUT: Duration = Duration::from_secs(2);
const SNAPSHOT_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) fn workspace_snapshot_from_handle(
    runtime: &Arc<RhoiScribeRuntime>,
    handle_id: &str,
) -> Result<Arc<CwtWorkspaceSnapshot>, ToolError> {
    let handle = runtime
        .cwt_language()
        .get_workspace(handle_id)
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
        .ok_or_else(|| ToolError::InvalidRequest(format!("unknown CWT workspace `{handle_id}`")))?;

    if let Some(snapshot) = handle
        .snapshot()
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
    {
        return Ok(snapshot);
    }

    schedule_warm_refresh(&handle)?;
    wait_for_snapshot(&handle)
}

pub(super) fn workspace_root_from_handle(
    runtime: &Arc<RhoiScribeRuntime>,
    handle_id: &str,
) -> Result<PathBuf, ToolError> {
    workspace_snapshot_from_handle(runtime, handle_id)
        .map(|snapshot| snapshot.workspace_root.clone())
}

pub(super) fn bounded_limit(limit: Option<usize>, default: usize) -> usize {
    limit.unwrap_or(default).clamp(1, default)
}

pub(super) fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub(super) fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn schedule_warm_refresh(handle: &Arc<CwtWorkspaceHandle>) -> Result<(), ToolError> {
    let status = handle
        .status()
        .map_err(|error| ToolError::InvalidRequest(error.to_string()))?;
    match status.state {
        CwtWorkspaceWarmState::Warming => Ok(()),
        CwtWorkspaceWarmState::Cold | CwtWorkspaceWarmState::Failed => handle
            .refresh()
            .map(|_| ())
            .map_err(|error| ToolError::InvalidRequest(error.to_string())),
        CwtWorkspaceWarmState::Warm => Ok(()),
    }
}

fn wait_for_snapshot(
    handle: &Arc<CwtWorkspaceHandle>,
) -> Result<Arc<CwtWorkspaceSnapshot>, ToolError> {
    let deadline = Instant::now() + SNAPSHOT_WAIT_TIMEOUT;

    loop {
        if let Some(snapshot) = handle
            .snapshot()
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?
        {
            return Ok(snapshot);
        }
        let status = handle
            .status()
            .map_err(|error| ToolError::InvalidRequest(error.to_string()))?;
        if status.state == CwtWorkspaceWarmState::Failed {
            return Err(ToolError::InvalidRequest(
                status
                    .last_error
                    .unwrap_or_else(|| "CWT workspace warm-up failed".to_string()),
            ));
        }
        if Instant::now() >= deadline {
            return Err(ToolError::InvalidRequest(
                "CWT workspace is still warming; poll get_hoi4_language_status and retry"
                    .to_string(),
            ));
        }
        thread::sleep(SNAPSHOT_POLL_INTERVAL);
    }
}
