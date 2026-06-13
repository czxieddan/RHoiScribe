use std::{
    fs,
    path::{Path, PathBuf},
};

use super::ScanRoot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectFile {
    pub root: String,
    pub root_role: Option<String>,
    pub absolute_path: PathBuf,
    pub relative_path: String,
    pub bytes: u64,
}

pub(crate) fn collect_project_files(
    roots: &[ScanRoot],
    should_include: fn(&str) -> bool,
) -> Result<Vec<ProjectFile>, String> {
    let mut files = Vec::new();

    for root in roots {
        let root_path = PathBuf::from(&root.path);
        if !root_path.exists() {
            return Err(format!("project root does not exist: {}", root.path));
        }
        if !root_path.is_dir() {
            return Err(format!("project root is not a directory: {}", root.path));
        }

        let mut pending = vec![root_path.clone()];
        while let Some(path) = pending.pop() {
            let entries = fs::read_dir(&path)
                .map_err(|error| format!("failed to read {}: {}", path.display(), error))?;

            for entry in entries {
                let entry = entry.map_err(|error| error.to_string())?;
                let entry_path = entry.path();
                let file_type = entry.file_type().map_err(|error| error.to_string())?;

                if file_type.is_dir() {
                    if should_descend(&entry_path) {
                        pending.push(entry_path);
                    }
                    continue;
                }

                if !file_type.is_file() {
                    continue;
                }

                let relative_path = relative_path(&root_path, &entry_path);
                if !should_include(&relative_path) {
                    continue;
                }

                let metadata = entry.metadata().map_err(|error| error.to_string())?;
                files.push(ProjectFile {
                    root: root.path.clone(),
                    root_role: root.role.clone(),
                    absolute_path: entry_path,
                    relative_path,
                    bytes: metadata.len(),
                });
            }
        }
    }

    Ok(files)
}

fn should_descend(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    !matches!(
        name.to_ascii_lowercase().as_str(),
        ".git" | "target" | "plans" | "tests" | "scripts" | ".idea" | ".vscode" | ".superpowers"
    )
}

fn relative_path(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}
