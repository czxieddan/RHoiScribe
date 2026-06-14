use std::{fs, path::Path, sync::Arc, thread};

use serde::{Deserialize, Serialize};

use super::ScanRoot;
use super::hoi4_keys::flag_entity_type;
use super::paradox_lexer::{Token, TokenKind, tokenize};
use super::project_files::{ProjectFile, collect_project_files};

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
struct ScanFile {
    root: String,
    root_role: Option<String>,
    absolute_path: std::path::PathBuf,
    relative_path: String,
    file_type: String,
    bytes: u64,
}

#[derive(Debug, Clone, Default)]
struct WorkerOutput {
    files: Vec<IndexedFile>,
    definitions: Vec<ProjectIndexItem>,
    references: Vec<ProjectIndexItem>,
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

fn scan_text_file(file: &ScanFile, content: &str, output: &mut WorkerOutput) {
    scan_localisation(file, content, output);

    let tokens = tokenize(content);
    let mut stack = Vec::<String>::new();
    let mut index = 0usize;

    while index < tokens.len() {
        let token = &tokens[index];

        if token.kind == TokenKind::Close {
            stack.pop();
            index += 1;
            continue;
        }

        if is_block_start(&tokens, index) {
            let key = tokens[index].text.clone();
            scan_block_definition(file, &key, token.line, &stack, output);
            stack.push(key);
            index += 3;
            continue;
        }

        if is_assignment(&tokens, index) {
            let key = &tokens[index].text;
            let value = &tokens[index + 2].text;
            scan_assignment(file, key, value, token.line, &stack, output);
            index += 3;
            continue;
        }

        index += 1;
    }
}

fn scan_block_definition(
    file: &ScanFile,
    key: &str,
    line: usize,
    stack: &[String],
    output: &mut WorkerOutput,
) {
    let path = file.relative_path.as_str();
    let parent = stack.last().map(String::as_str);

    if path.starts_with("common/scripted_triggers/") && stack.is_empty() {
        push_definition(
            file,
            output,
            "scripted_trigger",
            key,
            line,
            "top-level scripted trigger",
        );
    }

    if path.starts_with("common/scripted_effects/") && stack.is_empty() {
        push_definition(
            file,
            output,
            "scripted_effect",
            key,
            line,
            "top-level scripted effect",
        );
    }

    if path.starts_with("common/ideas/") && !is_ignored_idea_block(key) {
        push_definition(file, output, "idea_token", key, line, "idea token block");
    }

    if path.starts_with("common/dynamic_modifiers/") || parent == Some("dynamic_modifier") {
        push_definition(
            file,
            output,
            "dynamic_modifier",
            key,
            line,
            "dynamic modifier block",
        );
    }
}

fn scan_assignment(
    file: &ScanFile,
    key: &str,
    value: &str,
    line: usize,
    stack: &[String],
    output: &mut WorkerOutput,
) {
    let current_block = stack.last().map(String::as_str);

    scan_asset_assignment(file, key, value, line, current_block, output);
    scan_flag_assignment(file, key, value, line, current_block, output);
    scan_variable_assignment(file, key, value, line, current_block, output);
    scan_focus_event_assignment(file, key, value, line, current_block, output);
    scan_country_tag_assignment(file, key, line, output);
}

fn scan_asset_assignment(
    file: &ScanFile,
    key: &str,
    value: &str,
    line: usize,
    current_block: Option<&str>,
    output: &mut WorkerOutput,
) {
    if key == "name" && current_block == Some("spriteType") {
        push_definition(file, output, "gfx_sprite", value, line, "spriteType name");
    }

    if key == "name"
        && file.relative_path.starts_with("interface/")
        && let Some(block) = current_block
        && matches!(
            block,
            "containerWindowType"
                | "buttonType"
                | "iconType"
                | "instantTextBoxType"
                | "listboxType"
        )
    {
        push_definition(file, output, "gui_element", value, line, block);
    }

    if key == "texturefile" && current_block == Some("spriteType") {
        push_reference(
            file,
            output,
            "asset_texture",
            value,
            line,
            "sprite texturefile",
        );
    }

    if key == "quadTextureSprite" || key == "spriteType" {
        push_reference(file, output, "gfx_sprite", value, line, key);
    }
}

fn scan_flag_assignment(
    file: &ScanFile,
    key: &str,
    value: &str,
    line: usize,
    current_block: Option<&str>,
    output: &mut WorkerOutput,
) {
    if let Some(flag_kind) = flag_entity_type(key) {
        push_reference(file, output, flag_kind, value, line, key);
    }

    if key == "flag"
        && let Some(block) = current_block.and_then(flag_entity_type)
    {
        push_reference(file, output, block, value, line, "flag field");
    }
}

fn scan_variable_assignment(
    file: &ScanFile,
    key: &str,
    value: &str,
    line: usize,
    current_block: Option<&str>,
    output: &mut WorkerOutput,
) {
    if is_variable_key(key) {
        push_reference(file, output, "variable", value, line, key);
    }

    if current_block.is_some_and(is_variable_key)
        && let Some(variable_name) = variable_name_from_field(key, value)
    {
        push_reference(
            file,
            output,
            "variable",
            variable_name,
            line,
            "variable field",
        );
    }
}

fn scan_focus_event_assignment(
    file: &ScanFile,
    key: &str,
    value: &str,
    line: usize,
    current_block: Option<&str>,
    output: &mut WorkerOutput,
) {
    if key == "id" {
        match current_block {
            Some("focus") | Some("shared_focus") | Some("joint_focus") => {
                push_definition(file, output, "focus_id", value, line, "focus id");
            }
            Some("focus_tree") => {
                push_definition(file, output, "focus_tree_id", value, line, "focus tree id")
            }
            Some("country_event" | "news_event" | "state_event" | "unit_event") => {
                push_definition(file, output, "event_id", value, line, "event id");
            }
            _ => {}
        }
    }

    if matches!(key, "shared_focus" | "joint_focus") {
        push_reference(file, output, "focus_id", value, line, key);
    }

    if key == "namespace" {
        push_definition(
            file,
            output,
            "event_namespace",
            value,
            line,
            "event namespace",
        );
    }
}

fn scan_country_tag_assignment(file: &ScanFile, key: &str, line: usize, output: &mut WorkerOutput) {
    let path = file.relative_path.as_str();
    if path.starts_with("common/country_tags/") {
        push_definition(file, output, "country_tag", key, line, "country tag");
    }
}

fn scan_localisation(file: &ScanFile, content: &str, output: &mut WorkerOutput) {
    if !file.relative_path.starts_with("localisation/") {
        return;
    }

    for (line_index, line) in content.lines().enumerate() {
        let trimmed = line.trim_start().trim_start_matches('\u{feff}');
        let Some((key, rest)) = trimmed.split_once(':') else {
            continue;
        };

        let key = key.trim();
        let rest = rest.trim_start();
        if key.is_empty() || is_localisation_language_header(key, rest) {
            continue;
        }

        push_definition(
            file,
            output,
            "localisation_key",
            key,
            line_index + 1,
            "localisation key",
        );
    }
}

fn push_definition(
    file: &ScanFile,
    output: &mut WorkerOutput,
    kind: &str,
    name: &str,
    line: usize,
    context: &str,
) {
    output
        .definitions
        .push(project_item(file, kind, name, line, context));
}

fn push_reference(
    file: &ScanFile,
    output: &mut WorkerOutput,
    kind: &str,
    name: &str,
    line: usize,
    context: &str,
) {
    output
        .references
        .push(project_item(file, kind, name, line, context));
}

fn project_item(
    file: &ScanFile,
    kind: &str,
    name: &str,
    line: usize,
    context: &str,
) -> ProjectIndexItem {
    ProjectIndexItem {
        kind: kind.to_string(),
        name: name.to_string(),
        root: file.root.clone(),
        root_role: file.root_role.clone(),
        path: file.relative_path.clone(),
        line,
        context: context.to_string(),
    }
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

fn is_block_start(tokens: &[Token], index: usize) -> bool {
    index + 2 < tokens.len()
        && tokens[index].kind == TokenKind::Word
        && tokens[index + 1].kind == TokenKind::Equals
        && tokens[index + 2].kind == TokenKind::Open
}

fn is_assignment(tokens: &[Token], index: usize) -> bool {
    index + 2 < tokens.len()
        && tokens[index].kind == TokenKind::Word
        && tokens[index + 1].kind == TokenKind::Equals
        && matches!(tokens[index + 2].kind, TokenKind::Word | TokenKind::String)
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

fn is_variable_key(key: &str) -> bool {
    matches!(
        key,
        "set_variable"
            | "set_temp_variable"
            | "add_to_variable"
            | "subtract_from_variable"
            | "multiply_variable"
            | "divide_variable"
            | "modulo_variable"
            | "clamp_variable"
            | "round_variable"
            | "check_variable"
            | "has_variable"
            | "clear_variable"
    )
}

fn is_variable_name_field(key: &str) -> bool {
    matches!(key, "var" | "variable" | "which")
}

fn variable_name_from_field<'a>(key: &'a str, value: &'a str) -> Option<&'a str> {
    if is_variable_name_field(key) {
        return Some(value);
    }

    (!is_variable_option_key(key) && is_script_identifier(key)).then_some(key)
}

fn is_variable_option_key(key: &str) -> bool {
    matches!(
        key,
        "value"
            | "min"
            | "max"
            | "add"
            | "subtract"
            | "multiply"
            | "divide"
            | "modulo"
            | "tooltip"
            | "days"
            | "check_range_bounds"
    )
}

fn is_script_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '.'))
}

fn is_localisation_language_header(key: &str, rest: &str) -> bool {
    key.starts_with("l_") && (rest.is_empty() || rest.starts_with('#'))
}

fn is_ignored_idea_block(key: &str) -> bool {
    matches!(
        key,
        "ideas"
            | "country"
            | "political_advisor"
            | "theorist"
            | "army_chief"
            | "navy_chief"
            | "air_chief"
            | "high_command"
            | "designer"
            | "industrial_concern"
            | "materiel_manufacturer"
            | "modifier"
            | "allowed"
            | "visible"
            | "available"
            | "allowed_civil_war"
            | "cancel"
            | "on_add"
            | "on_remove"
            | "traits"
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{ProjectIndexRequest, index_hoi4_project};
    use crate::tools::{ScanRoot, test_support::unique_test_dir};

    #[test]
    fn indexes_hoi4_definitions_and_references() {
        let root = unique_test_dir("project-index");
        write_file(
            &root,
            "common/scripted_triggers/CHI_triggers.txt",
            "CHI_has_system_ready = { has_country_flag = CHI_system_ready check_variable = { CHI_score > 0 } }\n",
        );
        write_file(
            &root,
            "common/scripted_effects/CHI_effects.txt",
            "CHI_apply_system = { set_country_flag = CHI_system_ready set_variable = { CHI_score = 1 } }\n",
        );
        write_file(
            &root,
            "interface/CHI_interface.gfx",
            "spriteTypes = { spriteType = { name = \"GFX_CHI_panel\" texturefile = \"gfx/interface/CHI/panel.png\" } }\n",
        );
        write_file(
            &root,
            "interface/CHI_interface.gui",
            "guiTypes = { containerWindowType = { name = \"CHI_panel_window\" background = { quadTextureSprite = \"GFX_CHI_panel\" } } }\n",
        );
        write_file(
            &root,
            "localisation/simp_chinese/CHI_l_simp_chinese.yml",
            "\u{feff}l_simp_chinese:\n CHI_system_ready:0 \"系统\"\n",
        );

        let index = index_hoi4_project(ProjectIndexRequest {
            roots: vec![ScanRoot {
                path: root.to_string_lossy().to_string(),
                role: Some("mod".to_string()),
            }],
            include_game_roots: Some(true),
        })
        .expect("index should build");

        assert_eq!(index.scanned_roots, 1);
        assert!(index.scanned_files >= 5);
        assert!(
            index
                .definitions
                .iter()
                .any(|item| item.kind == "scripted_trigger" && item.name == "CHI_has_system_ready")
        );
        assert!(
            index
                .definitions
                .iter()
                .any(|item| item.kind == "scripted_effect" && item.name == "CHI_apply_system")
        );
        assert!(
            index
                .definitions
                .iter()
                .any(|item| item.kind == "gfx_sprite" && item.name == "GFX_CHI_panel")
        );
        assert!(
            index
                .definitions
                .iter()
                .any(|item| item.kind == "gui_element" && item.name == "CHI_panel_window")
        );
        assert!(
            index
                .references
                .iter()
                .any(|item| item.kind == "country_flag" && item.name == "CHI_system_ready")
        );
        assert!(
            index
                .references
                .iter()
                .any(|item| item.kind == "variable" && item.name == "CHI_score")
        );
        assert!(
            index
                .references
                .iter()
                .any(|item| item.kind == "gfx_sprite" && item.name == "GFX_CHI_panel")
        );

        fs::remove_dir_all(root).expect("temp output should clean up");
    }

    fn write_file(root: &std::path::Path, relative_path: &str, content: &str) {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent should be created");
        }
        fs::write(path, content).expect("fixture file should be written");
    }
}
