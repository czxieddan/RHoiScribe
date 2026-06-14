use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use self::project_repair_media::{check_media_file, detect_ffmpeg};
#[cfg(test)]
use self::project_repair_media::{detect_ffmpeg_with_installer, ffprobe_command};
use super::project_files::{ProjectFile, collect_project_files};
use super::{ScanRoot, format_paradox_script};

#[path = "project_repair_media.rs"]
mod project_repair_media;
#[cfg(test)]
#[path = "project_repair_tests.rs"]
mod project_repair_tests;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepairHoi4ProjectRequest {
    pub roots: Vec<ScanRoot>,
    pub dry_run: bool,
    pub apply: Option<bool>,
    pub install_ffmpeg: Option<bool>,
    pub format_scripts: Option<bool>,
    pub check_media: Option<bool>,
    pub ffmpeg_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepairCheck {
    pub id: String,
    pub status: String,
    pub severity: String,
    pub path: String,
    pub message: String,
    pub quick_fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepairChange {
    pub path: String,
    pub action: String,
    pub applied: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FfmpegStatus {
    pub available: bool,
    pub command: Option<String>,
    pub install_required: bool,
    pub install_attempted: bool,
    pub install_succeeded: bool,
    pub install_error: Option<String>,
    pub install_script: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepairHoi4ProjectResult {
    pub dry_run: bool,
    pub applied: bool,
    pub status: String,
    pub checks: Vec<RepairCheck>,
    pub changes: Vec<RepairChange>,
    pub ffmpeg: FfmpegStatus,
    pub messages: Vec<String>,
}

struct RepairRunContext {
    apply: bool,
    format_scripts: bool,
    check_media: bool,
    files: Vec<ProjectFile>,
    ffmpeg: FfmpegStatus,
}

pub fn repair_hoi4_project(
    request: RepairHoi4ProjectRequest,
) -> Result<RepairHoi4ProjectResult, String> {
    if request.roots.is_empty() {
        return Err("at least one project root is required".to_string());
    }

    let context = RepairRunContext::from_request(&request)?;
    let (mut checks, mut changes) = repair_files(&context.files, &context)?;

    checks.push(check(
        "repair_scan_completed",
        "green",
        "info",
        "",
        &format!("Scanned {} project file(s).", context.files.len()),
        None,
    ));
    sort_repair_output(&mut checks, &mut changes);

    Ok(repair_result(request.dry_run, context, checks, changes))
}

impl RepairRunContext {
    fn from_request(request: &RepairHoi4ProjectRequest) -> Result<Self, String> {
        let apply = !request.dry_run && request.apply.unwrap_or(false);
        let format_scripts = request.format_scripts.unwrap_or(true);
        let check_media = request.check_media.unwrap_or(true);
        let files = collect_files(&request.roots)?;
        let ffmpeg = detect_ffmpeg(
            request.ffmpeg_path.as_deref(),
            request.install_ffmpeg.unwrap_or(false),
            needs_media_tools(check_media, &files),
            request.dry_run,
        );
        Ok(Self {
            apply,
            format_scripts,
            check_media,
            files,
            ffmpeg,
        })
    }
}

fn needs_media_tools(check_media: bool, files: &[ProjectFile]) -> bool {
    check_media
        && files.iter().any(|file| {
            file.relative_path.starts_with("music/") && has_extension(&file.relative_path, "ogg")
        })
}

fn sort_repair_output(checks: &mut [RepairCheck], changes: &mut [RepairChange]) {
    checks.sort_by(|left, right| {
        (
            status_rank(&left.status),
            &left.id,
            &left.path,
            &left.message,
        )
            .cmp(&(
                status_rank(&right.status),
                &right.id,
                &right.path,
                &right.message,
            ))
    });
    changes.sort_by(|left, right| (&left.path, &left.action).cmp(&(&right.path, &right.action)));
}

fn repair_result(
    dry_run: bool,
    context: RepairRunContext,
    checks: Vec<RepairCheck>,
    changes: Vec<RepairChange>,
) -> RepairHoi4ProjectResult {
    RepairHoi4ProjectResult {
        dry_run,
        applied: context.apply,
        status: overall_status(&checks).to_string(),
        checks,
        changes,
        ffmpeg: context.ffmpeg,
        messages: vec![repair_message(context.apply)],
    }
}

fn repair_message(apply: bool) -> String {
    if apply {
        "Repairs were applied in place. Review the diff before committing.".to_string()
    } else {
        "Dry-run only; no files were changed.".to_string()
    }
}

fn repair_files(
    files: &[ProjectFile],
    context: &RepairRunContext,
) -> Result<(Vec<RepairCheck>, Vec<RepairChange>), String> {
    let mut checks = Vec::new();
    let mut changes = Vec::new();

    for file in files {
        repair_encoding(file, context.apply, &mut checks, &mut changes)?;
        repair_formatting(
            file,
            context.apply,
            context.format_scripts,
            &mut checks,
            &mut changes,
        )?;
        if context.check_media {
            check_media_file(file, &context.ffmpeg, &mut checks);
        }
    }

    Ok((checks, changes))
}

fn repair_encoding(
    file: &ProjectFile,
    apply: bool,
    checks: &mut Vec<RepairCheck>,
    changes: &mut Vec<RepairChange>,
) -> Result<(), String> {
    if should_have_utf8_bom(&file.relative_path) {
        ensure_bom(file, apply, checks, changes)
    } else if should_have_utf8_without_bom(&file.relative_path) {
        ensure_no_bom(file, apply, checks, changes)
    } else {
        Ok(())
    }
}

fn repair_formatting(
    file: &ProjectFile,
    apply: bool,
    format_scripts: bool,
    checks: &mut Vec<RepairCheck>,
    changes: &mut Vec<RepairChange>,
) -> Result<(), String> {
    if format_scripts && should_format_script(&file.relative_path) {
        format_script_file(file, apply, checks, changes)
    } else {
        Ok(())
    }
}

fn ensure_bom(
    file: &ProjectFile,
    apply: bool,
    checks: &mut Vec<RepairCheck>,
    changes: &mut Vec<RepairChange>,
) -> Result<(), String> {
    let bytes = fs::read(&file.absolute_path)
        .map_err(|error| format!("failed to read {}: {}", file.absolute_path.display(), error))?;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok(());
    }

    checks.push(check(
        bom_check_id(&file.relative_path),
        "yellow",
        "warning",
        &file.relative_path,
        "HOI4 expects this file to be UTF-8 with BOM.",
        Some("Add UTF-8 BOM while preserving the text body.".to_string()),
    ));

    if apply {
        let mut repaired = vec![0xEF, 0xBB, 0xBF];
        repaired.extend_from_slice(strip_bom(&bytes));
        fs::write(&file.absolute_path, repaired).map_err(|error| {
            format!(
                "failed to write {}: {}",
                file.absolute_path.display(),
                error
            )
        })?;
    }

    changes.push(change(
        &file.relative_path,
        "add_utf8_bom",
        apply,
        "Add UTF-8 BOM.",
    ));
    Ok(())
}

fn ensure_no_bom(
    file: &ProjectFile,
    apply: bool,
    checks: &mut Vec<RepairCheck>,
    changes: &mut Vec<RepairChange>,
) -> Result<(), String> {
    let bytes = fs::read(&file.absolute_path)
        .map_err(|error| format!("failed to read {}: {}", file.absolute_path.display(), error))?;
    if !bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok(());
    }

    checks.push(check(
        "script_no_bom",
        "yellow",
        "warning",
        &file.relative_path,
        "Non-localisation txt/lua files should be UTF-8 without BOM.",
        Some("Remove the UTF-8 BOM from this script file.".to_string()),
    ));

    if apply {
        fs::write(&file.absolute_path, strip_bom(&bytes)).map_err(|error| {
            format!(
                "failed to write {}: {}",
                file.absolute_path.display(),
                error
            )
        })?;
    }

    changes.push(change(
        &file.relative_path,
        "remove_utf8_bom",
        apply,
        "Remove UTF-8 BOM.",
    ));
    Ok(())
}

fn format_script_file(
    file: &ProjectFile,
    apply: bool,
    checks: &mut Vec<RepairCheck>,
    changes: &mut Vec<RepairChange>,
) -> Result<(), String> {
    let bytes = fs::read(&file.absolute_path)
        .map_err(|error| format!("failed to read {}: {}", file.absolute_path.display(), error))?;
    let body = strip_bom(&bytes);
    let Ok(script) = String::from_utf8(body.to_vec()) else {
        return Ok(());
    };
    if !can_safely_format_script(&script) {
        checks.push(check(
            "script_format_skipped",
            "yellow",
            "warning",
            &file.relative_path,
            "Skipped script formatting because the file contains comments or quoted strings that require token-aware preservation.",
            Some("Use targeted editing or a token-aware formatter for this file.".to_string()),
        ));
        return Ok(());
    }
    let formatted = format_paradox_script(&script);
    if formatted.as_bytes() == body {
        return Ok(());
    }

    if apply {
        fs::write(&file.absolute_path, formatted.as_bytes()).map_err(|error| {
            format!(
                "failed to write {}: {}",
                file.absolute_path.display(),
                error
            )
        })?;
    }

    changes.push(change(
        &file.relative_path,
        "format_paradox_script",
        apply,
        "Apply basic Paradox script indentation.",
    ));
    Ok(())
}

fn can_safely_format_script(script: &str) -> bool {
    !script.lines().any(line_has_comment) && !script.contains('"')
}

fn line_has_comment(line: &str) -> bool {
    let mut escaped = false;
    let mut in_string = false;

    for character in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && in_string {
            escaped = true;
            continue;
        }
        if character == '"' {
            in_string = !in_string;
            continue;
        }
        if character == '#' && !in_string {
            return true;
        }
    }

    false
}

fn collect_files(roots: &[ScanRoot]) -> Result<Vec<ProjectFile>, String> {
    collect_project_files(roots, should_scan_file)
}

fn should_scan_file(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/");
    let Some(root) = normalized.split('/').next() else {
        return false;
    };
    if !matches!(
        root,
        "common" | "events" | "history" | "interface" | "localisation" | "sound" | "music"
    ) {
        return false;
    }
    Path::new(&normalized)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "txt" | "gui" | "gfx" | "lua" | "yml" | "yaml" | "wav" | "ogg" | "mp3" | "flac"
            )
        })
}

fn should_have_utf8_bom(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/");
    normalized.starts_with("localisation/")
        || normalized.eq_ignore_ascii_case("interface/credits.txt")
}

fn should_have_utf8_without_bom(relative_path: &str) -> bool {
    if should_have_utf8_bom(relative_path) {
        return false;
    }
    has_extension(relative_path, "txt")
        || has_extension(relative_path, "lua")
        || has_extension(relative_path, "gui")
        || has_extension(relative_path, "gfx")
}

fn should_format_script(relative_path: &str) -> bool {
    !relative_path.starts_with("localisation/")
        && !relative_path.eq_ignore_ascii_case("interface/credits.txt")
        && (has_extension(relative_path, "txt")
            || has_extension(relative_path, "gui")
            || has_extension(relative_path, "gfx"))
}

fn has_extension(relative_path: &str, extension: &str) -> bool {
    Path::new(relative_path)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
}

fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)
}

fn bom_check_id(relative_path: &str) -> &'static str {
    if relative_path.eq_ignore_ascii_case("interface/credits.txt") {
        "credits_bom"
    } else {
        "localisation_bom"
    }
}

fn check(
    id: &str,
    status: &str,
    severity: &str,
    path: &str,
    message: &str,
    quick_fix: Option<String>,
) -> RepairCheck {
    RepairCheck {
        id: id.to_string(),
        status: status.to_string(),
        severity: severity.to_string(),
        path: path.to_string(),
        message: message.to_string(),
        quick_fix,
    }
}

fn change(path: &str, action: &str, applied: bool, summary: &str) -> RepairChange {
    RepairChange {
        path: path.to_string(),
        action: action.to_string(),
        applied,
        summary: summary.to_string(),
    }
}

fn overall_status(checks: &[RepairCheck]) -> &str {
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
