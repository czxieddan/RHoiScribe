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
        let root_path = checked_root_path(root)?;
        collect_root_files(root, &root_path, should_include, &mut files)?;
    }

    Ok(files)
}

fn checked_root_path(root: &ScanRoot) -> Result<PathBuf, String> {
    let root_path = PathBuf::from(&root.path);
    if !root_path.exists() {
        return Err(format!("project root does not exist: {}", root.path));
    }
    if !root_path.is_dir() {
        return Err(format!("project root is not a directory: {}", root.path));
    }
    Ok(root_path)
}

fn collect_root_files(
    root: &ScanRoot,
    root_path: &Path,
    should_include: fn(&str) -> bool,
    files: &mut Vec<ProjectFile>,
) -> Result<(), String> {
    let mut pending = vec![root_path.to_path_buf()];
    while let Some(path) = pending.pop() {
        collect_directory_entries(root, root_path, &path, should_include, &mut pending, files)?;
    }
    Ok(())
}

fn collect_directory_entries(
    root: &ScanRoot,
    root_path: &Path,
    path: &Path,
    should_include: fn(&str) -> bool,
    pending: &mut Vec<PathBuf>,
    files: &mut Vec<ProjectFile>,
) -> Result<(), String> {
    let entries = fs::read_dir(path)
        .map_err(|error| format!("failed to read {}: {}", path.display(), error))?;

    for entry in entries {
        collect_entry(root, root_path, entry, should_include, pending, files)?;
    }

    Ok(())
}

fn collect_entry(
    root: &ScanRoot,
    root_path: &Path,
    entry: Result<fs::DirEntry, std::io::Error>,
    should_include: fn(&str) -> bool,
    pending: &mut Vec<PathBuf>,
    files: &mut Vec<ProjectFile>,
) -> Result<(), String> {
    let entry = entry.map_err(|error| error.to_string())?;
    let entry_path = entry.path();
    let file_type = entry.file_type().map_err(|error| error.to_string())?;

    if file_type.is_dir() {
        if should_descend(&entry_path) {
            pending.push(entry_path);
        }
        return Ok(());
    }

    if file_type.is_file() {
        collect_file(root, root_path, entry, entry_path, should_include, files)?;
    }

    Ok(())
}

fn collect_file(
    root: &ScanRoot,
    root_path: &Path,
    entry: fs::DirEntry,
    entry_path: PathBuf,
    should_include: fn(&str) -> bool,
    files: &mut Vec<ProjectFile>,
) -> Result<(), String> {
    let relative_path = relative_path(root_path, &entry_path)?;
    if !should_include(&relative_path) {
        return Ok(());
    }

    let metadata = entry.metadata().map_err(|error| error.to_string())?;
    files.push(ProjectFile {
        root: root.path.clone(),
        root_role: root.role.clone(),
        absolute_path: entry_path,
        relative_path,
        bytes: metadata.len(),
    });
    Ok(())
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

fn relative_path(root: &Path, file: &Path) -> Result<String, String> {
    file.strip_prefix(root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .map_err(|error| {
            format!(
                "failed to normalize {} relative to {}: {}",
                file.display(),
                root.display(),
                error
            )
        })
}

#[cfg(test)]
mod tests {
    use super::relative_path;

    #[test]
    fn relative_path_rejects_files_outside_root() {
        let root = std::path::Path::new("project/root");
        let outside = std::path::Path::new("other/root/common/test.txt");

        let result = relative_path(root, outside);

        assert!(result.is_err());
    }
}
