use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
};

use serde::{Deserialize, Serialize};

use self::project_validation_localisation::missing_localisation_checks;
use super::paradox_lexer::{TokenKind, tokenize};
use super::project_files::ProjectFile;
use super::{IndexedFile, ProjectIndexItem, ProjectIndexRequest, ScanRoot, project_index};

#[path = "project_validation_localisation.rs"]
mod project_validation_localisation;
#[cfg(test)]
#[path = "project_validation_tests.rs"]
mod project_validation_tests;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectValidationRequest {
    pub roots: Vec<ScanRoot>,
    pub include_game_roots: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectValidationCheck {
    pub id: String,
    pub status: String,
    pub severity: String,
    pub path: String,
    pub line: usize,
    pub message: String,
    pub quick_fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectValidationResult {
    pub status: String,
    pub checks: Vec<ProjectValidationCheck>,
    pub index_summary: String,
    pub messages: Vec<String>,
}

pub fn validate_hoi4_project(
    request: ProjectValidationRequest,
) -> Result<ProjectValidationResult, String> {
    if request.roots.is_empty() {
        return Err("at least one project root is required".to_string());
    }

    let index = project_index::index_hoi4_project(ProjectIndexRequest {
        roots: request.roots.clone(),
        include_game_roots: request.include_game_roots,
    })?;
    let validation_files = validation_files_from_index(&index.files);
    let mut checks = vec![index_completed_check(&index)];
    check_duplicate_definitions(&index.definitions, &mut checks);
    check_brace_balance(&validation_files, &mut checks)?;
    check_replace_path_risks(&validation_files, &mut checks)?;
    check_missing_gfx_textures(&request.roots, &index.references, &mut checks);
    check_missing_gfx_sprites(&index.definitions, &index.references, &mut checks);
    check_missing_localisation(&validation_files, &index.definitions, &mut checks)?;
    sort_checks(&mut checks);

    let status = overall_status(&checks).to_string();

    Ok(ProjectValidationResult {
        status,
        index_summary: index_summary(&index),
        messages: vec![
            "red blocks game-readability or likely load success; yellow needs review before release; green passed".to_string(),
        ],
        checks,
    })
}

fn index_completed_check(index: &project_index::ProjectIndexResult) -> ProjectValidationCheck {
    check(
        "index_completed",
        "green",
        "info",
        "",
        0,
        &format!(
            "Indexed {} file(s), {} definition(s), and {} reference(s).",
            index.scanned_files,
            index.definitions.len(),
            index.references.len()
        ),
        None,
    )
}

fn index_summary(index: &project_index::ProjectIndexResult) -> String {
    format!(
        "{} file(s), {} definition(s), {} reference(s)",
        index.scanned_files,
        index.definitions.len(),
        index.references.len()
    )
}

fn sort_checks(checks: &mut [ProjectValidationCheck]) {
    checks.sort_by(|left, right| {
        (
            status_rank(&left.status),
            &left.id,
            &left.path,
            left.line,
            &left.message,
        )
            .cmp(&(
                status_rank(&right.status),
                &right.id,
                &right.path,
                right.line,
                &right.message,
            ))
    });
}

fn validation_files_from_index(files: &[IndexedFile]) -> Vec<ProjectFile> {
    files
        .iter()
        .map(|file| ProjectFile {
            root: file.root.clone(),
            root_role: file.root_role.clone(),
            absolute_path: PathBuf::from(&file.root).join(&file.path),
            relative_path: file.path.clone(),
            bytes: file.bytes,
        })
        .collect()
}

fn check_duplicate_definitions(
    definitions: &[ProjectIndexItem],
    checks: &mut Vec<ProjectValidationCheck>,
) {
    let mut grouped: HashMap<(&str, &str), Vec<&ProjectIndexItem>> = HashMap::new();

    for definition in definitions {
        grouped
            .entry((&definition.kind, &definition.name))
            .or_default()
            .push(definition);
    }

    for ((kind, name), hits) in grouped {
        if hits.len() < 2 {
            continue;
        }

        let locations = hits
            .iter()
            .map(|hit| format!("{}:{}", hit.path, hit.line))
            .collect::<Vec<_>>()
            .join(", ");

        for hit in hits {
            checks.push(check(
                "duplicate_definition",
                "red",
                "error",
                &hit.path,
                hit.line,
                &format!(
                    "Duplicate {} `{}` appears in {}. Create a unique identifier unless this is an intentional replace-path override.",
                    kind, name, locations
                ),
                Some(format!("Rename `{}` or remove the duplicate definition.", name)),
            ));
        }
    }
}

fn check_brace_balance(
    project_files: &[ProjectFile],
    checks: &mut Vec<ProjectValidationCheck>,
) -> Result<(), String> {
    let files = project_files
        .iter()
        .filter(|file| is_paradox_text_file(&file.relative_path))
        .cloned()
        .collect::<Vec<_>>();
    checks.extend(parallel_file_checks(files, brace_balance_checks)?);

    Ok(())
}

fn check_replace_path_risks(
    project_files: &[ProjectFile],
    checks: &mut Vec<ProjectValidationCheck>,
) -> Result<(), String> {
    for file in project_files {
        if !is_mod_descriptor(&file.relative_path) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&file.absolute_path) else {
            continue;
        };
        for token_window in tokenize(&content).windows(3) {
            if token_window[0].text == "replace_path"
                && token_window[1].kind == TokenKind::Equals
                && matches!(token_window[2].kind, TokenKind::Word | TokenKind::String)
            {
                checks.push(check(
                    "replace_path",
                    "yellow",
                    "warning",
                    &file.relative_path,
                    token_window[0].line,
                    &format!(
                        "Descriptor replace_path `{}` hides vanilla files under that folder.",
                        token_window[2].text
                    ),
                    Some("Confirm every generated file in this folder intentionally replaces vanilla content.".to_string()),
                ));
            }
        }
    }

    Ok(())
}

fn check_missing_gfx_textures(
    roots: &[ScanRoot],
    references: &[ProjectIndexItem],
    checks: &mut Vec<ProjectValidationCheck>,
) {
    for reference in references
        .iter()
        .filter(|reference| reference.kind == "asset_texture")
    {
        let texture_path = normalize_relative_path(&reference.name);
        if roots
            .iter()
            .any(|root| Path::new(&root.path).join(&texture_path).is_file())
        {
            continue;
        }

        checks.push(check(
            "missing_gfx_texture",
            "red",
            "error",
            &reference.path,
            reference.line,
            &format!(
                "GFX sprite references texture `{}` but no scanned root contains that file.",
                reference.name
            ),
            Some(format!(
                "Create `{}` or update the sprite texturefile.",
                texture_path
            )),
        ));
    }
}

fn check_missing_gfx_sprites(
    definitions: &[ProjectIndexItem],
    references: &[ProjectIndexItem],
    checks: &mut Vec<ProjectValidationCheck>,
) {
    let defined_sprites = definitions
        .iter()
        .filter(|definition| definition.kind == "gfx_sprite")
        .map(|definition| definition.name.as_str())
        .collect::<HashSet<_>>();

    for reference in references
        .iter()
        .filter(|reference| reference.kind == "gfx_sprite")
    {
        if defined_sprites.contains(reference.name.as_str()) {
            continue;
        }

        checks.push(check(
            "missing_gfx_sprite",
            "yellow",
            "warning",
            &reference.path,
            reference.line,
            &format!(
                "GUI references sprite `{}` but no spriteType definition was indexed.",
                reference.name
            ),
            Some(
                "Add a matching spriteType in interface/*.gfx or reuse an existing sprite name."
                    .to_string(),
            ),
        ));
    }
}

fn check_missing_localisation(
    project_files: &[ProjectFile],
    definitions: &[ProjectIndexItem],
    checks: &mut Vec<ProjectValidationCheck>,
) -> Result<(), String> {
    let defined_keys = definitions
        .iter()
        .filter(|definition| definition.kind == "localisation_key")
        .map(|definition| definition.name.clone())
        .collect::<HashSet<_>>();
    let defined_keys = Arc::new(defined_keys);

    let files = project_files
        .iter()
        .filter(|file| is_script_with_localisation_refs(&file.relative_path))
        .cloned()
        .collect::<Vec<_>>();
    checks.extend(parallel_file_checks(files, move |file| {
        missing_localisation_checks(file, &defined_keys)
    })?);

    Ok(())
}

fn brace_balance_checks(file: ProjectFile) -> Vec<ProjectValidationCheck> {
    let Ok(content) = fs::read_to_string(&file.absolute_path) else {
        return Vec::new();
    };
    let outcome = brace_balance_outcome(&content);

    if let Some(line) = outcome.first_underflow {
        return vec![brace_underflow_check(&file.relative_path, line)];
    }
    if outcome.depth != 0 {
        return vec![brace_depth_check(
            &file.relative_path,
            outcome.last_line,
            outcome.depth,
        )];
    }

    Vec::new()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BraceBalanceOutcome {
    depth: isize,
    first_underflow: Option<usize>,
    last_line: usize,
}

fn brace_balance_outcome(content: &str) -> BraceBalanceOutcome {
    let mut outcome = BraceBalanceOutcome {
        depth: 0,
        first_underflow: None,
        last_line: 1,
    };
    for token in tokenize(content) {
        update_brace_outcome(&mut outcome, token.kind, token.line);
    }
    outcome
}

fn update_brace_outcome(outcome: &mut BraceBalanceOutcome, kind: TokenKind, line: usize) {
    outcome.last_line = line;
    match kind {
        TokenKind::Open => outcome.depth += 1,
        TokenKind::Close => {
            outcome.depth -= 1;
            if outcome.depth < 0 && outcome.first_underflow.is_none() {
                outcome.first_underflow = Some(line);
            }
        }
        _ => {}
    }
}

fn brace_underflow_check(path: &str, line: usize) -> ProjectValidationCheck {
    check(
        "brace_balance",
        "red",
        "error",
        path,
        line,
        "Closing brace appears before a matching opening brace.",
        Some("Remove the extra closing brace or add the missing opening block.".to_string()),
    )
}

fn brace_depth_check(path: &str, line: usize, depth: isize) -> ProjectValidationCheck {
    check(
        "brace_balance",
        "red",
        "error",
        path,
        line,
        &format!(
            "Brace balance ends at {}; HOI4 will not parse this file reliably.",
            depth
        ),
        Some("Add or remove braces until the file ends at depth 0.".to_string()),
    )
}

fn check(
    id: &str,
    status: &str,
    severity: &str,
    path: &str,
    line: usize,
    message: &str,
    quick_fix: Option<String>,
) -> ProjectValidationCheck {
    ProjectValidationCheck {
        id: id.to_string(),
        status: status.to_string(),
        severity: severity.to_string(),
        path: path.to_string(),
        line,
        message: message.to_string(),
        quick_fix,
    }
}

fn parallel_file_checks<F>(
    files: Vec<ProjectFile>,
    checker: F,
) -> Result<Vec<ProjectValidationCheck>, String>
where
    F: Fn(ProjectFile) -> Vec<ProjectValidationCheck> + Send + Sync + 'static,
{
    if files.is_empty() {
        return Ok(Vec::new());
    }

    let worker_count = worker_count(files.len());
    let chunk_size = files.len().div_ceil(worker_count);
    let files = Arc::new(files);
    let checker = Arc::new(checker);
    let mut handles = Vec::new();

    for chunk_start in (0..files.len()).step_by(chunk_size) {
        let files = Arc::clone(&files);
        let checker = Arc::clone(&checker);
        handles.push(thread::spawn(move || {
            let chunk_end = (chunk_start + chunk_size).min(files.len());
            let mut checks = Vec::new();
            for file in &files[chunk_start..chunk_end] {
                checks.extend(checker(file.clone()));
            }
            checks
        }));
    }

    let mut checks = Vec::new();
    for handle in handles {
        checks.extend(
            handle
                .join()
                .map_err(|_| "project validation worker panicked".to_string())?,
        );
    }
    Ok(checks)
}

fn worker_count(file_count: usize) -> usize {
    if file_count == 0 {
        return 1;
    }

    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(file_count)
        .max(1)
}

fn is_paradox_text_file(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/");
    if is_mod_descriptor(&normalized) {
        return true;
    }
    Path::new(&normalized)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "txt" | "gui" | "gfx" | "lua"
            )
        })
}

fn is_script_with_localisation_refs(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/");
    if normalized.starts_with("localisation/") {
        return false;
    }
    Path::new(&normalized)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "txt" | "gui" | "gfx"
            )
        })
}

fn is_mod_descriptor(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    file_name == "descriptor.mod" || file_name.ends_with(".mod")
}

fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/").trim().trim_matches('/').to_string()
}

fn overall_status(checks: &[ProjectValidationCheck]) -> &str {
    if checks.iter().any(|check| check.status == "red") {
        "red"
    } else if checks.iter().any(|check| check.status == "yellow") {
        "yellow"
    } else {
        "green"
    }
}

fn status_rank(status: &str) -> u8 {
    match status {
        "red" => 0,
        "yellow" => 1,
        "green" => 2,
        _ => 3,
    }
}
