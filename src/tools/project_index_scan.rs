use super::{ProjectIndexItem, ScanFile, WorkerOutput};
use crate::tools::hoi4_keys::flag_entity_type;
use crate::tools::paradox_lexer::{Token, TokenKind, tokenize};

pub(super) fn scan_text_file(file: &ScanFile, content: &str, output: &mut WorkerOutput) {
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
            scan_assignment(
                file,
                &tokens[index].text,
                &tokens[index + 2].text,
                token.line,
                &stack,
                output,
            );
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

    if let Some((kind, context)) = top_level_scripted_kind(path, stack.is_empty()) {
        push_definition(file, output, kind, key, line, context);
    }
    if idea_token_definition(path, key) {
        push_definition(file, output, "idea_token", key, line, "idea token block");
    }
    if dynamic_modifier_definition(path, parent) {
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

fn top_level_scripted_kind(path: &str, is_top_level: bool) -> Option<(&'static str, &'static str)> {
    if !is_top_level {
        return None;
    }
    if path.starts_with("common/scripted_triggers/") {
        Some(("scripted_trigger", "top-level scripted trigger"))
    } else if path.starts_with("common/scripted_effects/") {
        Some(("scripted_effect", "top-level scripted effect"))
    } else {
        None
    }
}

fn idea_token_definition(path: &str, key: &str) -> bool {
    path.starts_with("common/ideas/") && !is_ignored_idea_block(key)
}

fn dynamic_modifier_definition(path: &str, parent: Option<&str>) -> bool {
    path.starts_with("common/dynamic_modifiers/") || parent == Some("dynamic_modifier")
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
    if let Some((kind, context)) = asset_definition_kind(file, key, current_block) {
        push_definition(file, output, kind, value, line, context);
    }
    if let Some((kind, context)) = asset_reference_kind(key, current_block) {
        push_reference(file, output, kind, value, line, context);
    }
}

fn asset_definition_kind<'a>(
    file: &ScanFile,
    key: &str,
    current_block: Option<&'a str>,
) -> Option<(&'static str, &'a str)> {
    if key != "name" {
        return None;
    }
    match current_block {
        Some("spriteType") => Some(("gfx_sprite", "spriteType name")),
        Some(block)
            if is_gui_element_block(block) && file.relative_path.starts_with("interface/") =>
        {
            Some(("gui_element", block))
        }
        _ => None,
    }
}

fn asset_reference_kind<'a>(
    key: &'a str,
    current_block: Option<&str>,
) -> Option<(&'static str, &'a str)> {
    match key {
        "texturefile" if current_block == Some("spriteType") => {
            Some(("asset_texture", "sprite texturefile"))
        }
        "quadTextureSprite" | "spriteType" => Some(("gfx_sprite", key)),
        _ => None,
    }
}

fn is_gui_element_block(block: &str) -> bool {
    matches!(
        block,
        "containerWindowType" | "buttonType" | "iconType" | "instantTextBoxType" | "listboxType"
    )
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
    if let Some((kind, context)) = id_definition_kind(key, current_block) {
        push_definition(file, output, kind, value, line, context);
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

fn id_definition_kind(
    key: &str,
    current_block: Option<&str>,
) -> Option<(&'static str, &'static str)> {
    if key != "id" {
        return None;
    }
    match current_block {
        Some("focus" | "shared_focus" | "joint_focus") => Some(("focus_id", "focus id")),
        Some("focus_tree") => Some(("focus_tree_id", "focus tree id")),
        Some("country_event" | "news_event" | "state_event" | "unit_event") => {
            Some(("event_id", "event id"))
        }
        _ => None,
    }
}

fn scan_country_tag_assignment(file: &ScanFile, key: &str, line: usize, output: &mut WorkerOutput) {
    if file.relative_path.starts_with("common/country_tags/") {
        push_definition(file, output, "country_tag", key, line, "country tag");
    }
}

fn scan_localisation(file: &ScanFile, content: &str, output: &mut WorkerOutput) {
    if !file.relative_path.starts_with("localisation/") {
        return;
    }

    for (line_index, line) in content.lines().enumerate() {
        if let Some(key) = localisation_key(line) {
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
}

fn localisation_key(line: &str) -> Option<&str> {
    let trimmed = line.trim_start().trim_start_matches('\u{feff}');
    let (key, rest) = trimmed.split_once(':')?;
    let key = key.trim();
    let rest = rest.trim_start();
    (!key.is_empty() && !is_localisation_language_header(key, rest)).then_some(key)
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
