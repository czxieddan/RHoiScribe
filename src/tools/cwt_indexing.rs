//------------------------------------------------------------------------------------
// cwt_indexing.rs -- Part of RHoiScribe
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

use std::{path::Path, sync::Arc};

use crate::RhoiScribeRuntime;

use super::{
    ScanRoot, ToolError,
    cwt_common::workspace_root_from_handle,
    project_index::{self, ProjectIndexRequest, ProjectIndexResult},
};

pub(super) struct CwtIndexQuery<'a> {
    pub(super) handle_id: Option<&'a str>,
    pub(super) workspace_root: Option<&'a str>,
    pub(super) roots: &'a [ScanRoot],
    pub(super) include_game_roots: Option<bool>,
    pub(super) missing_roots_message: &'static str,
}

pub(super) fn index_project(
    runtime: &Arc<RhoiScribeRuntime>,
    query: CwtIndexQuery<'_>,
) -> Result<ProjectIndexResult, ToolError> {
    let include_game_roots = query.include_game_roots;
    let roots = resolve_scan_roots(runtime, &query)?;
    project_index::index_hoi4_project(ProjectIndexRequest {
        roots,
        include_game_roots,
    })
    .map_err(ToolError::InvalidRequest)
}

fn resolve_scan_roots(
    runtime: &Arc<RhoiScribeRuntime>,
    query: &CwtIndexQuery<'_>,
) -> Result<Vec<ScanRoot>, ToolError> {
    if !query.roots.is_empty() {
        return Ok(query.roots.to_vec());
    }

    query
        .workspace_root
        .map(|workspace_root| Ok(root_from_workspace_path(workspace_root)))
        .or_else(|| root_from_handle(runtime, query.handle_id))
        .transpose()?
        .map(|root| vec![root])
        .ok_or_else(|| ToolError::InvalidRequest(query.missing_roots_message.to_string()))
}

fn root_from_workspace_path(workspace_root: &str) -> ScanRoot {
    ScanRoot {
        path: workspace_root.to_string(),
        role: Some("mod".to_string()),
    }
}

fn root_from_handle(
    runtime: &Arc<RhoiScribeRuntime>,
    handle_id: Option<&str>,
) -> Option<Result<ScanRoot, ToolError>> {
    let handle_id = handle_id?;
    Some(
        workspace_root_from_handle(runtime, handle_id).map(|workspace_root| ScanRoot {
            path: path_to_string(&workspace_root),
            role: Some("mod".to_string()),
        }),
    )
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
