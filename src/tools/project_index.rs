use std::{fs, path::Path, sync::Arc, thread};

use serde::{Deserialize, Serialize};

use self::project_index_scan::scan_text_file;
use super::ScanRoot;
use super::project_files::{ProjectFile, collect_project_files};

#[path = "project_index_scan.rs"]
mod project_index_scan;
#[cfg(test)]
#[path = "project_index_tests.rs"]
mod project_index_tests;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectIndexRequest {
    pub roots: Vec<ScanRoot>,
    pub include_game_roots: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectIndexItem {
    pub kind: String,
    pub name: String,
    pub root: String,
    pub root_role: Option<String>,
    pub path: String,
    pub line: usize,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexedFile {
    pub root: String,
    pub root_role: Option<String>,
    pub path: String,
    pub file_type: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectIndexResult {
    pub files: Vec<IndexedFile>,
    pub definitions: Vec<ProjectIndexItem>,
    pub references: Vec<ProjectIndexItem>,
    pub scanned_roots: usize,
    pub scanned_files: usize,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct ScanFile {
    pub(super) root: String,
    pub(super) root_role: Option<String>,
    pub(super) absolute_path: std::path::PathBuf,
    pub(super) relative_path: String,
    pub(super) file_type: String,
    pub(super) bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub(super) struct WorkerOutput {
    pub(super) files: Vec<IndexedFile>,
    pub(super) definitions: Vec<ProjectIndexItem>,
    pub(super) references: Vec<ProjectIndexItem>,
}

pub fn index_hoi4_project(request: ProjectIndexRequest) -> Result<ProjectIndexResult, String> {
    if request.roots.is_empty() {
        return Err("at least one project root is required".to_string());
    }

    let roots = if request.include_game_roots.unwrap_or(true) {
        request.roots
    } else {
        request
            .roots
            .into_iter()
            .filter(|root| root.role.as_deref() != Some("game"))
            .collect::<Vec<_>>()
    };

    if roots.is_empty() {
        return Err("no roots remain after filtering game roots".to_string());
    }

    let scan_files = collect_scan_files(&roots)?;
    let worker_count = worker_count(scan_files.len());
    let outputs = scan_files_parallel(scan_files, worker_count)?;
    let mut files = Vec::new();
    let mut definitions = Vec::new();
    let mut references = Vec::new();

    for output in outputs {
        files.extend(output.files);
        definitions.extend(output.definitions);
        references.extend(output.references);
    }

    sort_items(&mut definitions);
    sort_items(&mut references);
    files.sort_by(|left, right| (&left.root, &left.path).cmp(&(&right.root, &right.path)));

    let scanned_files = files.len();

    Ok(ProjectIndexResult {
        scanned_roots: roots.len(),
        scanned_files,
        files,
        definitions,
        references,
        messages: vec![format!(
            "indexed {} file(s) with {} worker(s)",
            scanned_files, worker_count
        )],
    })
}

fn collect_scan_files(roots: &[ScanRoot]) -> Result<Vec<ScanFile>, String> {
    collect_project_files(roots, should_index_file)
        .map(|files| files.into_iter().map(scan_file_from_project_file).collect())
}

fn scan_files_parallel(
    files: Vec<ScanFile>,
    worker_count: usize,
) -> Result<Vec<WorkerOutput>, String> {
    if files.is_empty() {
        return Ok(Vec::new());
    }

    let files = Arc::new(files);
    let chunk_size = files.len().div_ceil(worker_count);
    let mut handles = Vec::new();

    for chunk_start in (0..files.len()).step_by(chunk_size) {
        let files = Arc::clone(&files);
        handles.push(thread::spawn(move || {
            let chunk_end = (chunk_start + chunk_size).min(files.len());
            scan_file_chunk(&files[chunk_start..chunk_end])
        }));
    }

    let mut outputs = Vec::new();
    for handle in handles {
        outputs.push(
            handle
                .join()
                .map_err(|_| "project index worker panicked".to_string())?,
        );
    }

    Ok(outputs)
}

fn scan_file_chunk(files: &[ScanFile]) -> WorkerOutput {
    let mut output = WorkerOutput::default();

    for file in files {
        output.files.push(IndexedFile {
            root: file.root.clone(),
            root_role: file.root_role.clone(),
            path: file.relative_path.clone(),
            file_type: file.file_type.clone(),
            bytes: file.bytes,
        });

        if is_text_index_file(&file.relative_path) {
            let Ok(content) = fs::read_to_string(&file.absolute_path) else {
                continue;
            };
            scan_text_file(file, &content, &mut output);
        }
    }

    output
}

fn sort_items(items: &mut [ProjectIndexItem]) {
    items.sort_by(|left, right| {
        (
            &left.kind,
            &left.name,
            &left.root,
            &left.path,
            left.line,
            &left.context,
        )
            .cmp(&(
                &right.kind,
                &right.name,
                &right.root,
                &right.path,
                right.line,
                &right.context,
            ))
    });
}

fn should_index_file(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/");
    if normalized == "descriptor.mod" || normalized.ends_with(".mod") {
        return true;
    }

    let root = normalized.split('/').next();
    if !matches!(
        root,
        Some("common")
            | Some("events")
            | Some("history")
            | Some("interface")
            | Some("localisation")
            | Some("gfx")
            | Some("sound")
            | Some("music")
    ) {
        return false;
    }

    let Some(extension) = Path::new(&normalized)
        .extension()
        .and_then(|extension| extension.to_str())
    else {
        return false;
    };

    matches!(
        extension.to_ascii_lowercase().as_str(),
        "txt"
            | "gui"
            | "gfx"
            | "yml"
            | "yaml"
            | "lua"
            | "csv"
            | "png"
            | "dds"
            | "tga"
            | "wav"
            | "ogg"
    )
}

fn is_text_index_file(relative_path: &str) -> bool {
    let Some(extension) = Path::new(relative_path)
        .extension()
        .and_then(|extension| extension.to_str())
    else {
        return true;
    };

    matches!(
        extension.to_ascii_lowercase().as_str(),
        "txt" | "gui" | "gfx" | "yml" | "yaml" | "lua" | "csv" | "mod"
    )
}

fn file_type_name(relative_path: &str) -> String {
    Path::new(relative_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("mod")
        .to_ascii_lowercase()
}

fn scan_file_from_project_file(file: ProjectFile) -> ScanFile {
    let file_type = file_type_name(&file.relative_path);
    ScanFile {
        root: file.root,
        root_role: file.root_role,
        absolute_path: file.absolute_path,
        relative_path: file.relative_path,
        file_type,
        bytes: file.bytes,
    }
}

fn worker_count(file_count: usize) -> usize {
    if file_count == 0 {
        return 1;
    }

    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .clamp(1, file_count)
}
