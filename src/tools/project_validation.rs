use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    sync::Arc,
    thread,
};

use serde::{Deserialize, Serialize};

use super::paradox_lexer::{TokenKind, tokenize};
use super::project_files::{ProjectFile, collect_project_files};
use super::{ProjectIndexItem, ProjectIndexRequest, ScanRoot, project_index};

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

    let roots = if request.include_game_roots.unwrap_or(true) {
        request.roots.clone()
    } else {
        request
            .roots
            .iter()
            .filter(|root| root.role.as_deref() != Some("game"))
            .cloned()
            .collect::<Vec<_>>()
    };

    if roots.is_empty() {
        return Err("no roots remain after filtering game roots".to_string());
    }

    let index = project_index::index_hoi4_project(ProjectIndexRequest {
        roots: request.roots.clone(),
        include_game_roots: request.include_game_roots,
    })?;

    let mut checks = Vec::new();
    checks.push(check(
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
    ));

    check_duplicate_definitions(&index.definitions, &mut checks);
    check_brace_balance(&roots, &mut checks)?;
    check_replace_path_risks(&roots, &mut checks)?;
    check_missing_gfx_textures(&request.roots, &index.references, &mut checks);
    check_missing_gfx_sprites(&index.definitions, &index.references, &mut checks);
    check_missing_localisation(&roots, &index.definitions, &mut checks)?;

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

    let status = overall_status(&checks).to_string();

    Ok(ProjectValidationResult {
        status,
        checks,
        index_summary: format!(
            "{} file(s), {} definition(s), {} reference(s)",
            index.scanned_files,
            index.definitions.len(),
            index.references.len()
        ),
        messages: vec![
            "red blocks game-readability or likely load success; yellow needs review before release; green passed".to_string(),
        ],
    })
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
    roots: &[ScanRoot],
    checks: &mut Vec<ProjectValidationCheck>,
) -> Result<(), String> {
    let files = collect_files(roots)?
        .into_iter()
        .filter(|file| is_paradox_text_file(&file.relative_path))
        .collect::<Vec<_>>();
    checks.extend(parallel_file_checks(files, brace_balance_checks)?);

    Ok(())
}

fn check_replace_path_risks(
    roots: &[ScanRoot],
    checks: &mut Vec<ProjectValidationCheck>,
) -> Result<(), String> {
    for file in collect_files(roots)? {
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
    roots: &[ScanRoot],
    definitions: &[ProjectIndexItem],
    checks: &mut Vec<ProjectValidationCheck>,
) -> Result<(), String> {
    let defined_keys = definitions
        .iter()
        .filter(|definition| definition.kind == "localisation_key")
        .map(|definition| definition.name.clone())
        .collect::<HashSet<_>>();
    let defined_keys = Arc::new(defined_keys);

    let files = collect_files(roots)?
        .into_iter()
        .filter(|file| is_script_with_localisation_refs(&file.relative_path))
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
    let mut depth = 0isize;
    let mut first_underflow = None;
    let mut last_line = 1usize;

    for token in tokenize(&content) {
        last_line = token.line;
        match token.kind {
            TokenKind::Open => depth += 1,
            TokenKind::Close => {
                depth -= 1;
                if depth < 0 && first_underflow.is_none() {
                    first_underflow = Some(token.line);
                }
            }
            _ => {}
        }
    }

    if let Some(line) = first_underflow {
        return vec![check(
            "brace_balance",
            "red",
            "error",
            &file.relative_path,
            line,
            "Closing brace appears before a matching opening brace.",
            Some("Remove the extra closing brace or add the missing opening block.".to_string()),
        )];
    }
    if depth != 0 {
        return vec![check(
            "brace_balance",
            "red",
            "error",
            &file.relative_path,
            last_line,
            &format!(
                "Brace balance ends at {}; HOI4 will not parse this file reliably.",
                depth
            ),
            Some("Add or remove braces until the file ends at depth 0.".to_string()),
        )];
    }

    Vec::new()
}

fn missing_localisation_checks(
    file: ProjectFile,
    defined_keys: &HashSet<String>,
) -> Vec<ProjectValidationCheck> {
    let Ok(content) = fs::read_to_string(&file.absolute_path) else {
        return Vec::new();
    };

    let is_txt = extension_is(&file.relative_path, "txt");
    localisation_references(&content, is_txt)
        .into_iter()
        .filter(|(_, value, _)| {
            !defined_keys.contains(value.as_str()) && !is_inline_or_builtin_loc_value(value)
        })
        .map(|(key, value, line)| {
            check(
                "missing_localisation",
                "yellow",
                "warning",
                &file.relative_path,
                line,
                &format!(
                    "`{} = {}` looks like a localisation key but was not found in localisation files.",
                    key, value
                ),
                Some(format!("Add localisation key `{}` or update the script reference.", value)),
            )
        })
        .collect()
}

fn localisation_references(content: &str, is_txt: bool) -> Vec<(String, String, usize)> {
    let tokens = tokenize(content);
    let mut references = Vec::new();

    for window in tokens.windows(3) {
        if window[1].kind != TokenKind::Equals {
            continue;
        }
        if !matches!(window[2].kind, TokenKind::Word | TokenKind::String) {
            continue;
        }
        if is_localisation_reference_key(&window[0].text, is_txt) {
            references.push((
                window[0].text.clone(),
                window[2].text.clone(),
                window[0].line,
            ));
        }
    }

    references
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

fn collect_files(roots: &[ScanRoot]) -> Result<Vec<ProjectFile>, String> {
    collect_project_files(roots, should_validate_file)
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

fn should_validate_file(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/");
    if is_mod_descriptor(&normalized) {
        return true;
    }

    let Some(root) = normalized.split('/').next() else {
        return false;
    };
    if !matches!(
        root,
        "common" | "events" | "history" | "interface" | "localisation" | "gfx" | "sound" | "music"
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
            | "mod"
    )
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

fn extension_is(path: &str, expected: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/").trim().trim_matches('/').to_string()
}

fn is_localisation_reference_key(key: &str, is_txt: bool) -> bool {
    match key {
        "title"
        | "desc"
        | "description"
        | "custom_effect_tooltip"
        | "custom_trigger_tooltip"
        | "tooltip"
        | "delayed_event_text"
        | "major"
        | "minor" => true,
        "name" => is_txt,
        _ => false,
    }
}

fn is_inline_or_builtin_loc_value(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    if value.starts_with("GFX_")
        || value.starts_with("generic_")
        || matches!(value, "yes" | "no" | "always" | "ROOT" | "FROM" | "THIS")
    {
        return true;
    }
    if value
        .chars()
        .all(|character| character.is_ascii_digit() || matches!(character, '.' | '-' | '+' | '%'))
    {
        return true;
    }
    value.contains(' ')
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{ProjectValidationRequest, validate_hoi4_project};
    use crate::tools::ScanRoot;

    #[test]
    fn validation_reports_red_yellow_and_green_checks() {
        let root = unique_temp_dir();
        write_file(
            &root,
            "common/national_focus/CHI_tree.txt",
            "focus_tree = {\n\tid = CHI_tree\n\tfocus = { id = CHI_rebuild title = CHI_rebuild desc = CHI_rebuild_desc }\n\tfocus = { id = CHI_rebuild }\n",
        );
        write_file(
            &root,
            "interface/CHI_interface.gfx",
            "spriteTypes = { spriteType = { name = \"GFX_CHI_panel\" texturefile = \"gfx/interface/CHI/missing_panel.png\" } }\n",
        );
        write_file(
            &root,
            "interface/CHI_interface.gui",
            "guiTypes = { containerWindowType = { name = \"CHI_panel\" background = { quadTextureSprite = \"GFX_CHI_missing\" } } }\n",
        );
        write_file(
            &root,
            "localisation/simp_chinese/CHI_l_simp_chinese.yml",
            "\u{feff}l_simp_chinese:\n CHI_rebuild:0 \"重建\"\n",
        );

        let result = validate_hoi4_project(ProjectValidationRequest {
            roots: vec![ScanRoot {
                path: root.to_string_lossy().to_string(),
                role: Some("mod".to_string()),
            }],
            include_game_roots: Some(true),
        })
        .expect("validation should complete");

        assert_eq!(result.status, "red");
        assert!(result.index_summary.contains("file"));
        assert!(
            result
                .checks
                .iter()
                .any(|check| check.id == "duplicate_definition"
                    && check.status == "red"
                    && check.message.contains("CHI_rebuild"))
        );
        assert!(
            result
                .checks
                .iter()
                .any(|check| check.id == "brace_balance" && check.status == "red")
        );
        assert!(
            result
                .checks
                .iter()
                .any(|check| check.id == "missing_gfx_texture"
                    && check.status == "red"
                    && check.path == "interface/CHI_interface.gfx")
        );
        assert!(
            result
                .checks
                .iter()
                .any(|check| check.id == "missing_gfx_sprite"
                    && check.status == "yellow"
                    && check.message.contains("GFX_CHI_missing"))
        );
        assert!(
            result
                .checks
                .iter()
                .any(|check| check.id == "missing_localisation"
                    && check.status == "yellow"
                    && check.message.contains("CHI_rebuild_desc"))
        );
        assert!(
            result
                .checks
                .iter()
                .any(|check| check.id == "index_completed" && check.status == "green")
        );

        fs::remove_dir_all(root).expect("temp output should clean up");
    }

    #[test]
    fn validation_avoids_gui_name_and_vanilla_texture_false_positives() {
        let mod_root = unique_temp_dir();
        let game_root = unique_temp_dir();
        write_file(
            &mod_root,
            "interface/CHI_interface.gfx",
            "spriteTypes = { spriteType = { name = \"GFX_CHI_panel\" texturefile = \"gfx/interface/vanilla/panel.dds\" } }\n",
        );
        write_file(
            &mod_root,
            "interface/CHI_interface.gui",
            "guiTypes = { containerWindowType = { name = \"CHI_panel\" background = { quadTextureSprite = \"GFX_CHI_panel\" } } }\n",
        );
        write_file(
            &game_root,
            "gfx/interface/vanilla/panel.dds",
            "fake texture",
        );

        let result = validate_hoi4_project(ProjectValidationRequest {
            roots: vec![
                ScanRoot {
                    path: mod_root.to_string_lossy().to_string(),
                    role: Some("mod".to_string()),
                },
                ScanRoot {
                    path: game_root.to_string_lossy().to_string(),
                    role: Some("game".to_string()),
                },
            ],
            include_game_roots: Some(false),
        })
        .expect("validation should complete");

        assert!(
            !result
                .checks
                .iter()
                .any(|check| check.id == "missing_gfx_texture")
        );
        assert!(!result.checks.iter().any(|check| {
            check.id == "missing_localisation" && check.message.contains("CHI_panel")
        }));

        fs::remove_dir_all(mod_root).expect("temp output should clean up");
        fs::remove_dir_all(game_root).expect("temp output should clean up");
    }

    fn write_file(root: &std::path::Path, relative_path: &str, content: &str) {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent should be created");
        }
        fs::write(path, content).expect("fixture file should be written");
    }

    fn unique_temp_dir() -> std::path::PathBuf {
        static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos();
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rhoiscribe-project-validation-test-{}-{}-{}",
                std::process::id(),
                suffix,
                counter
            ));
            if fs::create_dir(&path).is_ok() {
                return path;
            }
        }
        panic!("failed to create unique temp directory");
    }
}
