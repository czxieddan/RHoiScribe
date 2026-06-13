use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::format_paradox_script;
use super::paradox_lexer::{TokenKind, tokenize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditHoi4ScriptFileRequest {
    pub path: String,
    pub workspace_root: Option<String>,
    pub operation: ScriptEditOperation,
    pub dry_run: bool,
    pub format: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScriptEditOperation {
    ReplaceNamedBlock {
        block_name: String,
        content: String,
    },
    InsertIntoBlock {
        parent_block: String,
        content: String,
        position: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditHoi4ScriptFileResult {
    pub dry_run: bool,
    pub applied: bool,
    pub changed: bool,
    pub path: String,
    pub encoding: String,
    pub preview: String,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BlockSpan {
    name: String,
    start: usize,
    open: usize,
    close: usize,
}

pub fn edit_hoi4_script_file(
    request: EditHoi4ScriptFileRequest,
) -> Result<EditHoi4ScriptFileResult, String> {
    let path = validate_workspace_script_path(
        Path::new(&request.path),
        request.workspace_root.as_deref(),
    )?;
    if !path.is_file() {
        return Err(format!("script file does not exist: {}", request.path));
    }
    if !is_supported_script_path(&path) {
        return Err("only HOI4 txt/gui/gfx/lua script files can be edited".to_string());
    }

    let bytes =
        fs::read(&path).map_err(|error| format!("failed to read {}: {}", request.path, error))?;
    let body = strip_bom(&bytes);
    let text = String::from_utf8(body.to_vec())
        .map_err(|error| format!("script file must be valid UTF-8: {}", error))?;
    ensure_balanced_braces(&text)?;

    let edited = match &request.operation {
        ScriptEditOperation::ReplaceNamedBlock {
            block_name,
            content,
        } => replace_named_block(&text, block_name, content)?,
        ScriptEditOperation::InsertIntoBlock {
            parent_block,
            content,
            position,
        } => insert_into_block(&text, parent_block, content, position.as_deref())?,
    };

    let formatted = if request.format.unwrap_or(true) {
        format_paradox_script(&edited)
    } else {
        edited
    };
    ensure_balanced_braces(&formatted)?;

    let mut output = Vec::new();
    if should_have_utf8_bom(&path) {
        output.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    output.extend_from_slice(formatted.as_bytes());
    let changed = output != bytes;
    let applied = changed && !request.dry_run;

    if applied {
        fs::write(&path, &output)
            .map_err(|error| format!("failed to write {}: {}", request.path, error))?;
    }

    Ok(EditHoi4ScriptFileResult {
        dry_run: request.dry_run,
        applied,
        changed,
        path: path.to_string_lossy().to_string(),
        encoding: if output.starts_with(&[0xEF, 0xBB, 0xBF]) {
            "utf-8-bom".to_string()
        } else {
            "utf-8".to_string()
        },
        preview: String::from_utf8_lossy(strip_bom(&output)).to_string(),
        messages: vec![if request.dry_run {
            "Dry-run only; no file was changed.".to_string()
        } else if changed {
            "File edited in place. Review the diff before committing.".to_string()
        } else {
            "Requested edit produced no content change.".to_string()
        }],
    })
}

fn replace_named_block(text: &str, block_name: &str, content: &str) -> Result<String, String> {
    let spans = named_block_spans(text, block_name);
    if spans.is_empty() {
        return Err(format!("block `{}` was not found", block_name));
    }
    if spans.len() > 1 {
        return Err(format!(
            "block `{}` appears {} times; refusing ambiguous replacement",
            block_name,
            spans.len()
        ));
    }

    let span = &spans[0];
    let replacement = normalized_block_content(content)?;
    let mut edited = String::new();
    edited.push_str(&text[..span.start]);
    edited.push_str(&replacement);
    edited.push_str(&text[span.close + 1..]);
    Ok(edited)
}

fn insert_into_block(
    text: &str,
    parent_block: &str,
    content: &str,
    position: Option<&str>,
) -> Result<String, String> {
    let block_name = first_block_name(content)?;
    if !named_block_spans(text, &block_name).is_empty() {
        return Err(format!("block `{}` already exists", block_name));
    }

    let spans = named_block_spans(text, parent_block);
    if spans.is_empty() {
        return Err(format!("parent block `{}` was not found", parent_block));
    }
    if spans.len() > 1 {
        return Err(format!(
            "parent block `{}` appears {} times; refusing ambiguous insertion",
            parent_block,
            spans.len()
        ));
    }

    let parent = &spans[0];
    let insertion = normalized_block_content(content)?;
    let insert_at = match position.unwrap_or("end") {
        "start" => parent.open + 1,
        "end" => parent.close,
        other => return Err(format!("unsupported insert position `{}`", other)),
    };

    let mut edited = String::new();
    edited.push_str(&text[..insert_at]);
    if !text[..insert_at].ends_with('\n') {
        edited.push('\n');
    }
    edited.push_str(&insertion);
    if !insertion.ends_with('\n') {
        edited.push('\n');
    }
    edited.push_str(&text[insert_at..]);
    Ok(edited)
}

fn normalized_block_content(content: &str) -> Result<String, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("replacement content is empty".to_string());
    }
    ensure_balanced_braces(trimmed)?;
    first_block_name(trimmed)?;
    Ok(trimmed.to_string())
}

fn first_block_name(content: &str) -> Result<String, String> {
    let tokens = tokenize(content);
    for window in tokens.windows(3) {
        if matches!(window[0].kind, TokenKind::Word | TokenKind::String)
            && window[1].kind == TokenKind::Equals
            && window[2].kind == TokenKind::Open
        {
            return Ok(window[0].text.clone());
        }
    }
    Err("content must contain a named `key = { ... }` block".to_string())
}

fn named_block_spans(text: &str, name: &str) -> Vec<BlockSpan> {
    let tokens = tokenize(text);
    let mut spans = Vec::new();
    let mut stack = Vec::<BlockSpan>::new();
    let mut index = 0usize;

    while index < tokens.len() {
        if index + 2 < tokens.len()
            && matches!(tokens[index].kind, TokenKind::Word | TokenKind::String)
            && tokens[index + 1].kind == TokenKind::Equals
            && tokens[index + 2].kind == TokenKind::Open
        {
            stack.push(BlockSpan {
                name: tokens[index].text.clone(),
                start: tokens[index].start,
                open: tokens[index + 2].start,
                close: tokens[index + 2].start,
            });
            index += 3;
            continue;
        }

        if tokens[index].kind == TokenKind::Close
            && let Some(mut span) = stack.pop()
        {
            span.close = tokens[index].start;
            if span.name == name {
                spans.push(span);
            }
        }
        index += 1;
    }

    spans
}

fn ensure_balanced_braces(text: &str) -> Result<(), String> {
    let mut depth = 0isize;
    for token in tokenize(text) {
        match token.kind {
            TokenKind::Open => depth += 1,
            TokenKind::Close => {
                depth -= 1;
                if depth < 0 {
                    return Err("closing brace appears before a matching opening brace".to_string());
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(format!("brace balance ends at {}", depth));
    }
    Ok(())
}

fn is_supported_script_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "txt" | "gui" | "gfx" | "lua"
            )
        })
}

fn validate_workspace_script_path(
    path: &Path,
    requested_workspace_root: Option<&str>,
) -> Result<PathBuf, String> {
    let workspace_root = workspace_root(requested_workspace_root)?;
    let canonical_path = path
        .canonicalize()
        .map_err(|error| format!("script file does not exist or is not accessible: {}", error))?;
    if !canonical_path.starts_with(&workspace_root) {
        return Err(format!(
            "script file must be inside workspace root `{}`",
            workspace_root.display()
        ));
    }
    Ok(canonical_path)
}

fn workspace_root(requested_workspace_root: Option<&str>) -> Result<PathBuf, String> {
    let root = if let Some(root) = requested_workspace_root {
        PathBuf::from(root)
    } else if let Some(root) = env::var_os("RHOISCRIBE_WORKSPACE_ROOT") {
        PathBuf::from(root)
    } else {
        env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };
    root.canonicalize()
        .map_err(|error| format!("workspace root is not accessible: {}", error))
}

fn should_have_utf8_bom(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized.contains("/localisation/") || normalized.ends_with("/interface/credits.txt")
}

fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{EditHoi4ScriptFileRequest, ScriptEditOperation, edit_hoi4_script_file};

    #[test]
    fn dry_run_replaces_named_block_without_writing() {
        let root = unique_temp_dir();
        let path = root.join("common/decisions/CHI_decisions.txt");
        write_file(
            &path,
            "CHI_category = {\n\tCHI_old_decision = {\n\t\tavailable = { always = yes }\n\t}\n}\n",
        );

        let result = edit_hoi4_script_file(EditHoi4ScriptFileRequest {
            path: path.to_string_lossy().to_string(),
            workspace_root: Some(root.to_string_lossy().to_string()),
            operation: ScriptEditOperation::ReplaceNamedBlock {
                block_name: "CHI_old_decision".to_string(),
                content: "CHI_old_decision = { complete_effect = { add_political_power = 25 } }"
                    .to_string(),
            },
            dry_run: true,
            format: Some(true),
        })
        .expect("dry-run edit should succeed");

        assert!(result.dry_run);
        assert!(!result.applied);
        assert!(result.changed);
        assert!(result.preview.contains("complete_effect = {"));
        assert!(
            fs::read_to_string(&path)
                .expect("source should remain readable")
                .contains("available")
        );

        fs::remove_dir_all(root).expect("temp output should clean up");
    }

    #[test]
    fn apply_inserts_block_and_preserves_no_bom_script_encoding() {
        let root = unique_temp_dir();
        let path = root.join("common/scripted_effects/CHI_effects.txt");
        write_file(&path, "effects = {\n}\n");

        let result = edit_hoi4_script_file(EditHoi4ScriptFileRequest {
            path: path.to_string_lossy().to_string(),
            workspace_root: Some(root.to_string_lossy().to_string()),
            operation: ScriptEditOperation::InsertIntoBlock {
                parent_block: "effects".to_string(),
                content: "CHI_new_effect = { add_stability = 0.05 }".to_string(),
                position: Some("end".to_string()),
            },
            dry_run: false,
            format: Some(true),
        })
        .expect("apply edit should succeed");

        assert!(result.applied);
        assert!(result.changed);
        let bytes = fs::read(&path).expect("edited file should read");
        assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        let text = String::from_utf8(bytes).expect("script should remain utf-8");
        assert!(text.contains("CHI_new_effect = {"));
        assert!(text.contains("\n\t\tadd_stability = 0.05"));

        fs::remove_dir_all(root).expect("temp output should clean up");
    }

    #[test]
    fn rejects_duplicate_inserted_block_names() {
        let root = unique_temp_dir();
        let path = root.join("common/decisions/CHI_decisions.txt");
        write_file(
            &path,
            "CHI_category = {\n\tCHI_decision = { available = { always = yes } }\n}\n",
        );

        let error = edit_hoi4_script_file(EditHoi4ScriptFileRequest {
            path: path.to_string_lossy().to_string(),
            workspace_root: Some(root.to_string_lossy().to_string()),
            operation: ScriptEditOperation::InsertIntoBlock {
                parent_block: "CHI_category".to_string(),
                content: "CHI_decision = { complete_effect = { add_political_power = 5 } }"
                    .to_string(),
                position: Some("end".to_string()),
            },
            dry_run: true,
            format: Some(true),
        })
        .expect_err("duplicate block insert should fail");

        assert!(error.contains("already exists"));

        fs::remove_dir_all(root).expect("temp output should clean up");
    }

    #[test]
    fn rejects_paths_outside_workspace_root() {
        let workspace = unique_temp_dir();
        let outside = unique_temp_dir();
        let path = outside.join("common/decisions/CHI_decisions.txt");
        write_file(&path, "CHI_category = {\n}\n");

        let error = edit_hoi4_script_file(EditHoi4ScriptFileRequest {
            path: path.to_string_lossy().to_string(),
            workspace_root: Some(workspace.to_string_lossy().to_string()),
            operation: ScriptEditOperation::InsertIntoBlock {
                parent_block: "CHI_category".to_string(),
                content: "CHI_decision = { available = { always = yes } }".to_string(),
                position: Some("end".to_string()),
            },
            dry_run: true,
            format: Some(false),
        })
        .expect_err("workspace escape should fail");

        assert!(error.contains("inside workspace root"));

        fs::remove_dir_all(workspace).expect("workspace temp should clean up");
        fs::remove_dir_all(outside).expect("outside temp should clean up");
    }

    #[test]
    fn handles_multibyte_utf8_tokens_without_panicking() {
        let root = unique_temp_dir();
        let path = root.join("common/scripted_effects/CHI_effects.txt");
        write_file(
            &path,
            "effects = {\n\tCHI_effect = { log = \"中文内容\" }\n}\n",
        );

        let result = edit_hoi4_script_file(EditHoi4ScriptFileRequest {
            path: path.to_string_lossy().to_string(),
            workspace_root: Some(root.to_string_lossy().to_string()),
            operation: ScriptEditOperation::ReplaceNamedBlock {
                block_name: "CHI_effect".to_string(),
                content: "CHI_effect = { log = \"新的中文内容\" }".to_string(),
            },
            dry_run: true,
            format: Some(false),
        })
        .expect("multibyte edit should not panic");

        assert!(result.preview.contains("新的中文内容"));

        fs::remove_dir_all(root).expect("temp output should clean up");
    }

    fn write_file(path: &std::path::Path, content: &str) {
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
                "rhoiscribe-script-edit-test-{}-{}-{}",
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
