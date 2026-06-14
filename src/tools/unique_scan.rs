use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    sync::Arc,
    thread,
};

use serde::{Deserialize, Serialize};

use super::hoi4_keys::{flag_entity_type, normalize_entity_type};
use super::paradox_lexer::{Token, TokenKind, tokenize};
use super::project_files::{ProjectFile, collect_project_files};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScanRoot {
    pub path: String,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentifierCandidate {
    pub entity_type: String,
    pub value: String,
    pub intent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UniqueIdentifierScanRequest {
    pub roots: Vec<ScanRoot>,
    pub candidates: Vec<IdentifierCandidate>,
    #[serde(default)]
    pub planned_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentifierMatch {
    pub entity_type: String,
    pub value: String,
    pub kind: String,
    pub root: String,
    pub root_role: Option<String>,
    pub path: String,
    pub line: usize,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateScanResult {
    pub entity_type: String,
    pub value: String,
    pub intent: String,
    pub availability: String,
    pub available: bool,
    pub matches: Vec<IdentifierMatch>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathRisk {
    pub kind: String,
    pub root: String,
    pub root_role: Option<String>,
    pub path: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UniqueIdentifierScanResult {
    pub candidates: Vec<CandidateScanResult>,
    pub path_risks: Vec<PathRisk>,
    pub scanned_roots: usize,
    pub scanned_files: usize,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct WorkerOutput {
    matches: Vec<IdentifierMatch>,
    replace_paths: Vec<ReplacePathHit>,
    scanned_files: usize,
}

#[derive(Debug, Clone)]
struct ReplacePathHit {
    root: String,
    root_role: Option<String>,
    path: String,
    source_path: String,
    line: usize,
}

pub fn scan_unique_identifiers(
    request: UniqueIdentifierScanRequest,
) -> Result<UniqueIdentifierScanResult, String> {
    if request.roots.is_empty() {
        return Err("at least one scan root is required".to_string());
    }

    if request.candidates.is_empty() && request.planned_paths.is_empty() {
        return Err("provide candidates, planned_paths, or both".to_string());
    }

    let candidate_lookup = Arc::new(candidate_lookup(&request.candidates));
    let scan_files = collect_scan_files(&request.roots)?;
    let worker_count = worker_count(scan_files.len());
    let outputs = scan_files_parallel(scan_files.clone(), worker_count, candidate_lookup)?;
    let mut matches = Vec::new();
    let mut replace_paths = Vec::new();
    let mut scanned_files = 0usize;

    for output in outputs {
        matches.extend(output.matches);
        replace_paths.extend(output.replace_paths);
        scanned_files += output.scanned_files;
    }

    matches.sort_by(|left, right| {
        (
            &left.entity_type,
            &left.value,
            &left.path,
            left.line,
            &left.kind,
        )
            .cmp(&(
                &right.entity_type,
                &right.value,
                &right.path,
                right.line,
                &right.kind,
            ))
    });

    let candidates = request
        .candidates
        .iter()
        .map(|candidate| candidate_result(candidate, &matches))
        .collect();
    let path_risks = path_risks(&request.roots, &request.planned_paths, &replace_paths);

    Ok(UniqueIdentifierScanResult {
        candidates,
        path_risks,
        scanned_roots: request.roots.len(),
        scanned_files,
        messages: vec![format!(
            "scanned {} files with {} worker(s)",
            scanned_files, worker_count
        )],
    })
}

fn candidate_lookup(candidates: &[IdentifierCandidate]) -> HashMap<String, HashSet<String>> {
    let mut lookup: HashMap<String, HashSet<String>> = HashMap::new();

    for candidate in candidates {
        lookup
            .entry(normalize_entity_type(&candidate.entity_type))
            .or_default()
            .insert(candidate.value.clone());
    }

    lookup
}

fn collect_scan_files(roots: &[ScanRoot]) -> Result<Vec<ProjectFile>, String> {
    collect_project_files(roots, should_scan_file)
}

fn should_scan_file(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/");
    let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);

    if file_name == "descriptor.mod" || file_name.ends_with(".mod") {
        return true;
    }

    let Some(extension) = Path::new(&normalized)
        .extension()
        .and_then(|extension| extension.to_str())
    else {
        return false;
    };

    let extension = extension.to_ascii_lowercase();
    if !matches!(
        extension.as_str(),
        "txt" | "gui" | "gfx" | "yml" | "yaml" | "lua" | "csv"
    ) {
        return false;
    }

    matches!(
        normalized.split('/').next(),
        Some("common")
            | Some("events")
            | Some("history")
            | Some("interface")
            | Some("localisation")
            | Some("gfx")
    )
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

fn scan_files_parallel(
    files: Vec<ProjectFile>,
    worker_count: usize,
    candidate_lookup: Arc<HashMap<String, HashSet<String>>>,
) -> Result<Vec<WorkerOutput>, String> {
    if files.is_empty() {
        return Ok(Vec::new());
    }

    let chunk_size = files.len().div_ceil(worker_count);
    let mut handles = Vec::new();

    for chunk in files.chunks(chunk_size) {
        let chunk = chunk.to_vec();
        let lookup = Arc::clone(&candidate_lookup);
        handles.push(thread::spawn(move || scan_file_chunk(&chunk, &lookup)));
    }

    let mut outputs = Vec::new();
    for handle in handles {
        outputs.push(
            handle
                .join()
                .map_err(|_| "scan worker panicked".to_string())??,
        );
    }

    Ok(outputs)
}

fn scan_file_chunk(
    files: &[ProjectFile],
    candidate_lookup: &HashMap<String, HashSet<String>>,
) -> Result<WorkerOutput, String> {
    let mut output = WorkerOutput::default();

    for file in files {
        let Ok(content) = fs::read_to_string(&file.absolute_path) else {
            continue;
        };

        let file_output = scan_file(file, &content, candidate_lookup);
        output.matches.extend(file_output.matches);
        output.replace_paths.extend(file_output.replace_paths);
        output.scanned_files += 1;
    }

    Ok(output)
}

fn scan_file(
    file: &ProjectFile,
    content: &str,
    candidate_lookup: &HashMap<String, HashSet<String>>,
) -> WorkerOutput {
    let tokens = tokenize(content);
    let mut output = WorkerOutput::default();
    let mut stack = Vec::<String>::new();
    let mut index = 0usize;

    scan_localisation_keys(file, content, candidate_lookup, &mut output);

    while index < tokens.len() {
        let token = &tokens[index];

        if token.kind == TokenKind::Close {
            stack.pop();
            index += 1;
            continue;
        }

        if is_block_start(&tokens, index) {
            let key = tokens[index].text.clone();
            scan_block_definition(
                file,
                &key,
                token.line,
                &stack,
                candidate_lookup,
                &mut output,
            );
            stack.push(key);
            index += 3;
            continue;
        }

        if is_assignment(&tokens, index) {
            let key = &tokens[index].text;
            let value = &tokens[index + 2].text;
            scan_assignment(
                file,
                key,
                value,
                token.line,
                &stack,
                candidate_lookup,
                &mut output,
            );
            index += 3;
            continue;
        }

        if token.kind == TokenKind::Open {
            stack.push(String::new());
        }

        index += 1;
    }

    output
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

fn scan_block_definition(
    file: &ProjectFile,
    key: &str,
    line: usize,
    stack: &[String],
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    let normalized_path = file.relative_path.as_str();
    let parent = stack.last().map(String::as_str);

    scan_idea_definition(file, key, line, normalized_path, candidate_lookup, output);
    scan_dynamic_modifier_definition(
        file,
        key,
        line,
        normalized_path,
        parent,
        candidate_lookup,
        output,
    );
    scan_character_definition(file, key, line, parent, candidate_lookup, output);
    scan_scripted_definition(
        file,
        key,
        line,
        normalized_path,
        stack.is_empty(),
        candidate_lookup,
        output,
    );
    scan_decision_definition(
        file,
        key,
        line,
        normalized_path,
        stack.len(),
        candidate_lookup,
        output,
    );
}

fn scan_assignment(
    file: &ProjectFile,
    key: &str,
    value: &str,
    line: usize,
    stack: &[String],
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    let current_block = stack.last().map(String::as_str);

    scan_replace_path(file, key, value, line, output);
    scan_focus_event_assignment(
        file,
        key,
        value,
        line,
        current_block,
        candidate_lookup,
        output,
    );
    scan_country_tag_assignment(file, key, line, candidate_lookup, output);
    scan_flag_assignment(
        file,
        key,
        value,
        line,
        current_block,
        candidate_lookup,
        output,
    );
    scan_variable_assignment(
        file,
        key,
        value,
        line,
        current_block,
        candidate_lookup,
        output,
    );
    scan_dynamic_modifier_reference(file, key, value, line, candidate_lookup, output);
}

fn scan_idea_definition(
    file: &ProjectFile,
    key: &str,
    line: usize,
    normalized_path: &str,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    if normalized_path.starts_with("common/ideas/")
        && !is_ignored_idea_block(key)
        && has_candidate(candidate_lookup, "idea_token", key)
    {
        push_match(
            file,
            output,
            "idea_token",
            key,
            "idea",
            line,
            "idea token block",
        );
    }
}

fn scan_dynamic_modifier_definition(
    file: &ProjectFile,
    key: &str,
    line: usize,
    normalized_path: &str,
    parent: Option<&str>,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    if (normalized_path.starts_with("common/dynamic_modifiers/")
        || parent == Some("dynamic_modifier"))
        && has_candidate(candidate_lookup, "dynamic_modifier", key)
    {
        push_match(
            file,
            output,
            "dynamic_modifier",
            key,
            "dynamic_modifier",
            line,
            "dynamic modifier block",
        );
    }
}

fn scan_character_definition(
    file: &ProjectFile,
    key: &str,
    line: usize,
    parent: Option<&str>,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    if parent == Some("characters") && has_candidate(candidate_lookup, "character", key) {
        push_match(
            file,
            output,
            "character",
            key,
            "character",
            line,
            "character block",
        );
    }
}

fn scan_scripted_definition(
    file: &ProjectFile,
    key: &str,
    line: usize,
    normalized_path: &str,
    is_top_level: bool,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    let Some((entity_type, kind, context)) = scripted_definition_kind(normalized_path) else {
        return;
    };
    if is_top_level && has_candidate(candidate_lookup, entity_type, key) {
        push_match(file, output, entity_type, key, kind, line, context);
    }
}

fn scripted_definition_kind(path: &str) -> Option<(&'static str, &'static str, &'static str)> {
    if path.starts_with("common/scripted_effects/") {
        Some((
            "scripted_effect",
            "scripted_effect",
            "top-level scripted effect",
        ))
    } else if path.starts_with("common/scripted_triggers/") {
        Some((
            "scripted_trigger",
            "scripted_trigger",
            "top-level scripted trigger",
        ))
    } else {
        None
    }
}

fn scan_decision_definition(
    file: &ProjectFile,
    key: &str,
    line: usize,
    normalized_path: &str,
    stack_depth: usize,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    if !normalized_path.starts_with("common/decisions/") {
        return;
    }

    match stack_depth {
        0 if has_candidate(candidate_lookup, "decision_category", key) => push_match(
            file,
            output,
            "decision_category",
            key,
            "decision_category",
            line,
            "top-level decision category",
        ),
        1 if !is_ignored_decision_block(key)
            && has_candidate(candidate_lookup, "decision", key) =>
        {
            push_match(
                file,
                output,
                "decision",
                key,
                "decision",
                line,
                "decision block inside category",
            );
        }
        _ => {}
    }
}

fn scan_replace_path(
    file: &ProjectFile,
    key: &str,
    value: &str,
    line: usize,
    output: &mut WorkerOutput,
) {
    if key == "replace_path" && is_mod_descriptor(&file.relative_path) {
        output.replace_paths.push(ReplacePathHit {
            root: file.root.clone(),
            root_role: file.root_role.clone(),
            path: normalize_relative_path(value),
            source_path: file.relative_path.clone(),
            line,
        });
    }
}

fn scan_focus_event_assignment(
    file: &ProjectFile,
    key: &str,
    value: &str,
    line: usize,
    current_block: Option<&str>,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    scan_id_assignment(
        file,
        key,
        value,
        line,
        current_block,
        candidate_lookup,
        output,
    );
    scan_reusable_focus_reference(file, key, value, line, candidate_lookup, output);
    scan_event_namespace(file, key, value, line, candidate_lookup, output);
}

fn scan_id_assignment(
    file: &ProjectFile,
    key: &str,
    value: &str,
    line: usize,
    current_block: Option<&str>,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    if key != "id" {
        return;
    }

    let Some((entity_type, kind, context)) = id_assignment_kind(current_block) else {
        return;
    };
    if has_candidate(candidate_lookup, entity_type, value) {
        push_match(file, output, entity_type, value, kind, line, context);
    }
}

fn id_assignment_kind(block: Option<&str>) -> Option<(&'static str, &'static str, &'static str)> {
    match block {
        Some("focus") => Some(("focus_id", "focus", "id inside focus-like block")),
        Some("shared_focus") => Some(("focus_id", "shared_focus", "id inside focus-like block")),
        Some("joint_focus") => Some(("focus_id", "joint_focus", "id inside focus-like block")),
        Some("focus_tree") => Some(("focus_tree_id", "focus_tree", "id inside focus_tree block")),
        Some("country_event") => Some(("event_id", "country_event", "event id")),
        Some("news_event") => Some(("event_id", "news_event", "event id")),
        Some("state_event") => Some(("event_id", "state_event", "event id")),
        Some("unit_event") => Some(("event_id", "unit_event", "event id")),
        _ => None,
    }
}

fn scan_reusable_focus_reference(
    file: &ProjectFile,
    key: &str,
    value: &str,
    line: usize,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    if matches!(key, "shared_focus" | "joint_focus")
        && has_candidate(candidate_lookup, "focus_id", value)
    {
        push_match(
            file,
            output,
            "focus_id",
            value,
            &format!("{}_reference", key),
            line,
            "focus tree reference to reusable focus id",
        );
    }
}

fn scan_event_namespace(
    file: &ProjectFile,
    key: &str,
    value: &str,
    line: usize,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    if key == "namespace" && has_candidate(candidate_lookup, "event_namespace", value) {
        push_match(
            file,
            output,
            "event_namespace",
            value,
            "event_namespace",
            line,
            "event namespace assignment",
        );
    }
}

fn scan_country_tag_assignment(
    file: &ProjectFile,
    key: &str,
    line: usize,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    if file.relative_path.starts_with("common/country_tags/")
        && has_candidate(candidate_lookup, "country_tag", key)
    {
        push_match(
            file,
            output,
            "country_tag",
            key,
            "country_tag",
            line,
            "country tag assignment",
        );
    }
}

fn scan_flag_assignment(
    file: &ProjectFile,
    key: &str,
    value: &str,
    line: usize,
    current_block: Option<&str>,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    if let Some(flag_entity_type) = flag_entity_type(key) {
        push_flag_match(
            file,
            output,
            candidate_lookup,
            FlagMatch {
                entity_type: flag_entity_type,
                value,
                kind: key,
                line,
                context: "flag usage",
            },
        );
    }

    if let ("flag", Some(flag_entity_type), Some(block)) =
        (key, current_block.and_then(flag_entity_type), current_block)
    {
        push_flag_match(
            file,
            output,
            candidate_lookup,
            FlagMatch {
                entity_type: flag_entity_type,
                value,
                kind: block,
                line,
                context: "flag field",
            },
        );
    }
}

fn scan_variable_assignment(
    file: &ProjectFile,
    key: &str,
    value: &str,
    line: usize,
    current_block: Option<&str>,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    if is_variable_key(key) && has_candidate(candidate_lookup, "variable", value) {
        push_match(file, output, "variable", value, key, line, "variable usage");
    }

    if let Some(block) = current_block.filter(|block| is_variable_key(block)) {
        let variable_name = if key == "var" { value } else { key };
        if (key == "var" || has_candidate(candidate_lookup, "variable", key))
            && has_candidate(candidate_lookup, "variable", variable_name)
        {
            push_match(
                file,
                output,
                "variable",
                variable_name,
                block,
                line,
                "variable field",
            );
        }
    }
}

fn scan_dynamic_modifier_reference(
    file: &ProjectFile,
    key: &str,
    value: &str,
    line: usize,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    if key == "modifier" && has_candidate(candidate_lookup, "dynamic_modifier", value) {
        push_match(
            file,
            output,
            "dynamic_modifier",
            value,
            "dynamic_modifier_reference",
            line,
            "modifier field reference",
        );
    }
}

fn scan_localisation_keys(
    file: &ProjectFile,
    content: &str,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    output: &mut WorkerOutput,
) {
    if !file.relative_path.starts_with("localisation/") {
        return;
    }

    for (line_index, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        let Some((key, rest)) = trimmed.split_once(':') else {
            continue;
        };

        if rest.starts_with('0') && has_candidate(candidate_lookup, "localisation_key", key) {
            push_match(
                file,
                output,
                "localisation_key",
                key,
                "localisation_key",
                line_index + 1,
                "localisation key",
            );
        }
    }
}

fn has_candidate(
    candidate_lookup: &HashMap<String, HashSet<String>>,
    entity_type: &str,
    value: &str,
) -> bool {
    candidate_lookup
        .get(entity_type)
        .is_some_and(|values| values.contains(value))
}

fn push_match(
    file: &ProjectFile,
    output: &mut WorkerOutput,
    entity_type: &str,
    value: &str,
    kind: &str,
    line: usize,
    context: &str,
) {
    output.matches.push(IdentifierMatch {
        entity_type: entity_type.to_string(),
        value: value.to_string(),
        kind: kind.to_string(),
        root: file.root.clone(),
        root_role: file.root_role.clone(),
        path: file.relative_path.clone(),
        line,
        context: context.to_string(),
    });
}

struct FlagMatch<'a> {
    entity_type: &'a str,
    value: &'a str,
    kind: &'a str,
    line: usize,
    context: &'a str,
}

fn push_flag_match(
    file: &ProjectFile,
    output: &mut WorkerOutput,
    candidate_lookup: &HashMap<String, HashSet<String>>,
    flag_match: FlagMatch<'_>,
) {
    if has_candidate(candidate_lookup, flag_match.entity_type, flag_match.value) {
        push_match(
            file,
            output,
            flag_match.entity_type,
            flag_match.value,
            flag_match.kind,
            flag_match.line,
            flag_match.context,
        );
    }

    if has_candidate(candidate_lookup, "flag", flag_match.value) {
        push_match(
            file,
            output,
            "flag",
            flag_match.value,
            flag_match.kind,
            flag_match.line,
            flag_match.context,
        );
    }
}

fn candidate_result(
    candidate: &IdentifierCandidate,
    matches: &[IdentifierMatch],
) -> CandidateScanResult {
    let entity_type = normalize_entity_type(&candidate.entity_type);
    let intent = candidate
        .intent
        .as_deref()
        .unwrap_or("create")
        .to_ascii_lowercase();
    let candidate_matches = matches
        .iter()
        .filter(|match_item| {
            match_item.entity_type == entity_type && match_item.value == candidate.value
        })
        .cloned()
        .collect::<Vec<_>>();

    let (availability, available, messages) = if intent == "create" {
        if candidate_matches.is_empty() {
            (
                "available".to_string(),
                true,
                vec!["no structured duplicate match found".to_string()],
            )
        } else {
            (
                "duplicate".to_string(),
                false,
                vec![format!(
                    "{} structured match(es) found; choose a new identifier or intentionally reference existing content",
                    candidate_matches.len()
                )],
            )
        }
    } else {
        (
            "not_checked".to_string(),
            true,
            vec!["intent is not create; uniqueness is informational only".to_string()],
        )
    };

    CandidateScanResult {
        entity_type,
        value: candidate.value.clone(),
        intent,
        availability,
        available,
        matches: candidate_matches,
        messages,
    }
}

fn path_risks(
    roots: &[ScanRoot],
    planned_paths: &[String],
    replace_paths: &[ReplacePathHit],
) -> Vec<PathRisk> {
    let mut risks = Vec::new();
    let planned_paths = planned_paths
        .iter()
        .map(|path| normalize_relative_path(path))
        .collect::<Vec<_>>();

    for root in roots {
        for planned_path in &planned_paths {
            if unsafe_relative_path(planned_path) {
                risks.push(PathRisk {
                    kind: "invalid_path".to_string(),
                    root: root.path.clone(),
                    root_role: root.role.clone(),
                    path: planned_path.clone(),
                    detail: "planned path must stay inside the mod root".to_string(),
                });
                continue;
            }

            if Path::new(&root.path).join(planned_path).exists() {
                risks.push(PathRisk {
                    kind: "file_exists".to_string(),
                    root: root.path.clone(),
                    root_role: root.role.clone(),
                    path: planned_path.clone(),
                    detail: "planned output path already exists under this root".to_string(),
                });
            }
        }
    }

    for replace_path in replace_paths {
        let affected = planned_paths
            .iter()
            .filter(|planned_path| path_starts_with(planned_path, &replace_path.path))
            .cloned()
            .collect::<Vec<_>>();
        let detail = if affected.is_empty() {
            format!(
                "replace_path declared in {}:{}; generated files under this folder hide vanilla files",
                replace_path.source_path, replace_path.line
            )
        } else {
            format!(
                "replace_path declared in {}:{} affects planned path(s): {}",
                replace_path.source_path,
                replace_path.line,
                affected.join(", ")
            )
        };

        risks.push(PathRisk {
            kind: "replace_path".to_string(),
            root: replace_path.root.clone(),
            root_role: replace_path.root_role.clone(),
            path: replace_path.path.clone(),
            detail,
        });
    }

    risks.sort_by(|left, right| {
        (&left.kind, &left.root, &left.path).cmp(&(&right.kind, &right.root, &right.path))
    });
    risks
}

fn path_starts_with(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn unsafe_relative_path(path: &str) -> bool {
    path.is_empty()
        || path.starts_with('/')
        || path.starts_with("../")
        || path.contains("/../")
        || path.contains(':')
}

fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/").trim().trim_matches('/').to_string()
}

fn is_mod_descriptor(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    file_name == "descriptor.mod" || file_name.ends_with(".mod")
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

fn is_ignored_decision_block(key: &str) -> bool {
    matches!(
        key,
        "visible"
            | "available"
            | "allowed"
            | "activation"
            | "cancel_trigger"
            | "complete_effect"
            | "timeout_effect"
            | "cancel_effect"
            | "remove_effect"
            | "ai_will_do"
            | "modifier"
            | "target_trigger"
            | "target_root_trigger"
            | "state_target"
            | "custom_cost_trigger"
            | "custom_cost_text"
    )
}
