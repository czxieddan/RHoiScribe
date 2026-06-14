use std::{collections::HashSet, fs, path::Path};

use super::{ProjectValidationCheck, check};
use crate::tools::paradox_lexer::{Token, TokenKind, tokenize};
use crate::tools::project_files::ProjectFile;

pub(super) fn missing_localisation_checks(
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
        .map(|(key, value, line)| missing_localisation_check(&file.relative_path, key, value, line))
        .collect()
}

fn missing_localisation_check(
    path: &str,
    key: String,
    value: String,
    line: usize,
) -> ProjectValidationCheck {
    check(
        "missing_localisation",
        "yellow",
        "warning",
        path,
        line,
        &format!(
            "`{} = {}` looks like a localisation key but was not found in localisation files.",
            key, value
        ),
        Some(format!(
            "Add localisation key `{}` or update the script reference.",
            value
        )),
    )
}

fn localisation_references(content: &str, is_txt: bool) -> Vec<(String, String, usize)> {
    let tokens = tokenize(content);
    let mut references = Vec::new();

    for window in tokens.windows(3) {
        if is_localisation_reference_window(window, is_txt) {
            references.push((
                window[0].text.clone(),
                window[2].text.clone(),
                window[0].line,
            ));
        }
    }

    references
}

fn is_localisation_reference_window(window: &[Token], is_txt: bool) -> bool {
    window[1].kind == TokenKind::Equals
        && matches!(window[2].kind, TokenKind::Word | TokenKind::String)
        && is_localisation_reference_key(&window[0].text, is_txt)
}

fn extension_is(path: &str, expected: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
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
    value.is_empty()
        || is_known_non_localisation_value(value)
        || is_numeric_value(value)
        || value.contains(' ')
}

fn is_known_non_localisation_value(value: &str) -> bool {
    value.starts_with("GFX_")
        || value.starts_with("generic_")
        || matches!(value, "yes" | "no" | "always" | "ROOT" | "FROM" | "THIS")
}

fn is_numeric_value(value: &str) -> bool {
    value
        .chars()
        .all(|character| character.is_ascii_digit() || matches!(character, '.' | '-' | '+' | '%'))
}
