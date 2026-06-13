use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};

use super::{ScanRoot, format_paradox_script};

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectFile {
    absolute_path: PathBuf,
    relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OggProbe {
    sample_rate: Option<u32>,
    bits_per_sample: Option<u32>,
    channels: Option<u32>,
}

pub fn repair_hoi4_project(
    request: RepairHoi4ProjectRequest,
) -> Result<RepairHoi4ProjectResult, String> {
    if request.roots.is_empty() {
        return Err("at least one project root is required".to_string());
    }

    let apply = !request.dry_run && request.apply.unwrap_or(false);
    let format_scripts = request.format_scripts.unwrap_or(true);
    let check_media = request.check_media.unwrap_or(true);
    let files = collect_files(&request.roots)?;
    let needs_media_tools = check_media
        && files.iter().any(|file| {
            file.relative_path.starts_with("music/") && has_extension(&file.relative_path, "ogg")
        });
    let ffmpeg = detect_ffmpeg(
        request.ffmpeg_path.as_deref(),
        request.install_ffmpeg.unwrap_or(false),
        needs_media_tools,
    );

    let mut checks = Vec::new();
    let mut changes = Vec::new();

    for file in &files {
        if should_have_utf8_bom(&file.relative_path) {
            ensure_bom(file, apply, &mut checks, &mut changes)?;
        } else if should_have_utf8_without_bom(&file.relative_path) {
            ensure_no_bom(file, apply, &mut checks, &mut changes)?;
        }

        if format_scripts && should_format_script(&file.relative_path) {
            format_script_file(file, apply, &mut changes)?;
        }

        if check_media {
            check_media_file(file, &ffmpeg, &mut checks);
        }
    }

    checks.push(check(
        "repair_scan_completed",
        "green",
        "info",
        "",
        &format!("Scanned {} project file(s).", files.len()),
        None,
    ));
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

    Ok(RepairHoi4ProjectResult {
        dry_run: request.dry_run,
        applied: apply,
        status: overall_status(&checks).to_string(),
        checks,
        changes,
        ffmpeg,
        messages: vec![if apply {
            "Repairs were applied in place. Review the diff before committing.".to_string()
        } else {
            "Dry-run only; no files were changed.".to_string()
        }],
    })
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
    changes: &mut Vec<RepairChange>,
) -> Result<(), String> {
    let bytes = fs::read(&file.absolute_path)
        .map_err(|error| format!("failed to read {}: {}", file.absolute_path.display(), error))?;
    let had_bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
    let body = strip_bom(&bytes);
    let Ok(script) = String::from_utf8(body.to_vec()) else {
        return Ok(());
    };
    let formatted = format_paradox_script(&script);
    if formatted.as_bytes() == body {
        return Ok(());
    }

    if apply {
        let mut repaired = Vec::new();
        if had_bom && should_have_utf8_bom(&file.relative_path) {
            repaired.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        repaired.extend_from_slice(formatted.as_bytes());
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
        "format_paradox_script",
        apply,
        "Apply basic Paradox script indentation.",
    ));
    Ok(())
}

fn check_media_file(file: &ProjectFile, ffmpeg: &FfmpegStatus, checks: &mut Vec<RepairCheck>) {
    if file.relative_path.starts_with("sound/") && !has_extension(&file.relative_path, "wav") {
        checks.push(check(
            "sound_wav_only",
            "red",
            "error",
            &file.relative_path,
            "Files under sound/ should be wav for HOI4 sound effects.",
            Some("Move this file out of sound/ or convert it to .wav.".to_string()),
        ));
    }

    if file.relative_path.starts_with("music/") && has_extension(&file.relative_path, "ogg") {
        if !ffmpeg.available {
            checks.push(check(
                "music_ogg_probe",
                "yellow",
                "warning",
                &file.relative_path,
                "Cannot verify music OGG sample rate, bit depth, and channels because ffmpeg/ffprobe is not available.",
                Some("Install ffmpeg, then rerun repair_hoi4_project with check_media enabled.".to_string()),
            ));
            return;
        }

        let probe = probe_ogg(file, ffmpeg);
        if probe.sample_rate != Some(44_100)
            || probe.bits_per_sample != Some(32)
            || probe.channels != Some(2)
        {
            checks.push(check(
                "music_ogg_format",
                "yellow",
                "warning",
                &file.relative_path,
                &format!(
                    "Music OGG should be 44100 Hz, 32-bit, 2 channels; detected rate={:?}, bits={:?}, channels={:?}.",
                    probe.sample_rate, probe.bits_per_sample, probe.channels
                ),
                Some("Use ffmpeg to convert the track to 44100 Hz, 32-bit, stereo OGG.".to_string()),
            ));
        }
    }
}

fn probe_ogg(file: &ProjectFile, ffmpeg: &FfmpegStatus) -> OggProbe {
    let Some(command) = ffmpeg.command.as_deref() else {
        return OggProbe {
            sample_rate: None,
            bits_per_sample: None,
            channels: None,
        };
    };
    let ffprobe = ffprobe_command(command);
    let Ok(output) = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=sample_rate,bits_per_sample,channels",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(&file.absolute_path)
        .output()
    else {
        return OggProbe {
            sample_rate: None,
            bits_per_sample: None,
            channels: None,
        };
    };

    if !output.status.success() {
        return OggProbe {
            sample_rate: None,
            bits_per_sample: None,
            channels: None,
        };
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut probe = OggProbe {
        sample_rate: None,
        bits_per_sample: None,
        channels: None,
    };
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("sample_rate=") {
            probe.sample_rate = value.parse().ok();
        }
        if let Some(value) = line.strip_prefix("bits_per_sample=") {
            probe.bits_per_sample = value.parse().ok();
        }
        if let Some(value) = line.strip_prefix("channels=") {
            probe.channels = value.parse().ok();
        }
    }
    probe
}

fn detect_ffmpeg(
    requested_path: Option<&str>,
    install_requested: bool,
    needed: bool,
) -> FfmpegStatus {
    let command = match requested_path {
        Some(path) => Path::new(path).is_file().then(|| path.to_string()),
        None => command_available("ffmpeg").then(|| "ffmpeg".to_string()),
    };

    let available = command.is_some();
    let install_required = needed && !available;
    let install_script = ffmpeg_install_script();

    FfmpegStatus {
        available,
        command,
        install_required,
        install_attempted: false,
        install_script,
        message: if available {
            "ffmpeg is available for media probing.".to_string()
        } else if install_requested && install_required {
            "ffmpeg is required for media probing, but RHoiScribe returned an install script instead of modifying the system automatically. Run it only after explicit user approval.".to_string()
        } else if install_required {
            "ffmpeg is required for full music checks. Ask the user before installing it."
                .to_string()
        } else {
            "ffmpeg was not needed for this request.".to_string()
        },
    }
}

fn ffmpeg_install_script() -> String {
    r#"# Requires explicit user approval before running.
if (Get-Command winget -ErrorAction SilentlyContinue) {
    winget install --id Gyan.FFmpeg --source winget
} elseif (Get-Command choco -ErrorAction SilentlyContinue) {
    choco install ffmpeg -y
} else {
    Write-Error "Install ffmpeg manually from https://ffmpeg.org/download.html and add it to PATH."
}
"#
    .to_string()
}

fn ffprobe_command(ffmpeg: &str) -> String {
    let path = Path::new(ffmpeg);
    if path.file_stem() == Some(OsStr::new("ffmpeg"))
        && let Some(parent) = path.parent()
    {
        return parent.join("ffprobe").to_string_lossy().to_string();
    }
    "ffprobe".to_string()
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("-version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn collect_files(roots: &[ScanRoot]) -> Result<Vec<ProjectFile>, String> {
    let mut files = Vec::new();

    for root in roots {
        let root_path = PathBuf::from(&root.path);
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
                let relative_path = entry_path
                    .strip_prefix(&root_path)
                    .unwrap_or(&entry_path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if should_scan_file(&relative_path) {
                    files.push(ProjectFile {
                        absolute_path: entry_path,
                        relative_path,
                    });
                }
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{RepairHoi4ProjectRequest, repair_hoi4_project};
    use crate::tools::ScanRoot;

    #[test]
    fn dry_run_reports_encoding_and_media_repairs_without_writing() {
        let root = unique_temp_dir();
        write_bytes(
            &root,
            "localisation/simp_chinese/CHI_l_simp_chinese.yml",
            b"l_simp_chinese:\n CHI_key:0 \"text\"\n",
        );
        write_bytes(
            &root,
            "common/national_focus/CHI.txt",
            &[0xEF, 0xBB, 0xBF, b'f', b'o', b'c', b'u', b's'],
        );
        write_bytes(&root, "sound/effect.ogg", b"not real audio");
        write_bytes(&root, "music/theme.ogg", b"not real audio");

        let result = repair_hoi4_project(RepairHoi4ProjectRequest {
            roots: vec![ScanRoot {
                path: root.to_string_lossy().to_string(),
                role: Some("mod".to_string()),
            }],
            dry_run: true,
            apply: Some(false),
            install_ffmpeg: Some(false),
            format_scripts: Some(false),
            check_media: Some(true),
            ffmpeg_path: Some(
                root.join("missing-ffmpeg.exe")
                    .to_string_lossy()
                    .to_string(),
            ),
        })
        .expect("repair dry-run should complete");

        assert!(result.dry_run);
        assert!(!result.applied);
        assert!(!result.ffmpeg.available);
        assert!(
            result
                .checks
                .iter()
                .any(|check| check.id == "localisation_bom"
                    && check.status == "yellow"
                    && check.path.ends_with("CHI_l_simp_chinese.yml"))
        );
        assert!(result.checks.iter().any(|check| check.id == "script_no_bom"
            && check.status == "yellow"
            && check.path == "common/national_focus/CHI.txt"));
        assert!(
            result
                .checks
                .iter()
                .any(|check| check.id == "sound_wav_only" && check.status == "red")
        );
        assert!(
            result
                .checks
                .iter()
                .any(|check| check.id == "music_ogg_probe" && check.status == "yellow")
        );
        assert!(
            result
                .changes
                .iter()
                .any(|change| change.action == "add_utf8_bom" && !change.applied)
        );
        assert!(
            !fs::read(root.join("localisation/simp_chinese/CHI_l_simp_chinese.yml"))
                .expect("localisation should remain readable")
                .starts_with(&[0xEF, 0xBB, 0xBF])
        );

        fs::remove_dir_all(root).expect("temp output should clean up");
    }

    #[test]
    fn apply_repairs_bom_rules_and_formats_scripts() {
        let root = unique_temp_dir();
        write_bytes(
            &root,
            "localisation/english/CHI_l_english.yml",
            b"l_english:\n CHI_key:0 \"Text\"\n",
        );
        write_bytes(
            &root,
            "interface/credits.txt",
            b"credits = { name = Test }\n",
        );
        write_bytes(
            &root,
            "common/scripted_effects/CHI_effects.txt",
            &[
                0xEF, 0xBB, 0xBF, b'C', b'H', b'I', b'_', b'e', b'f', b'f', b'e', b'c', b't', b'=',
                b'{', b'a', b'd', b'd', b'_', b'p', b'o', b'l', b'i', b't', b'i', b'c', b'a', b'l',
                b'_', b'p', b'o', b'w', b'e', b'r', b'=', b'1', b'}',
            ],
        );

        let result = repair_hoi4_project(RepairHoi4ProjectRequest {
            roots: vec![ScanRoot {
                path: root.to_string_lossy().to_string(),
                role: Some("mod".to_string()),
            }],
            dry_run: false,
            apply: Some(true),
            install_ffmpeg: Some(false),
            format_scripts: Some(true),
            check_media: Some(false),
            ffmpeg_path: None,
        })
        .expect("repair apply should complete");

        assert!(result.applied);
        assert!(
            fs::read(root.join("localisation/english/CHI_l_english.yml"))
                .expect("localisation should read")
                .starts_with(&[0xEF, 0xBB, 0xBF])
        );
        assert!(
            fs::read(root.join("interface/credits.txt"))
                .expect("credits should read")
                .starts_with(&[0xEF, 0xBB, 0xBF])
        );
        let script = fs::read(root.join("common/scripted_effects/CHI_effects.txt"))
            .expect("script should read");
        assert!(!script.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert!(String::from_utf8_lossy(&script).contains("CHI_effect = {"));

        fs::remove_dir_all(root).expect("temp output should clean up");
    }

    #[test]
    fn ffmpeg_install_request_returns_script_when_missing() {
        let root = unique_temp_dir();
        write_bytes(&root, "music/theme.ogg", b"not real audio");

        let result = repair_hoi4_project(RepairHoi4ProjectRequest {
            roots: vec![ScanRoot {
                path: root.to_string_lossy().to_string(),
                role: Some("mod".to_string()),
            }],
            dry_run: true,
            apply: Some(false),
            install_ffmpeg: Some(true),
            format_scripts: Some(false),
            check_media: Some(true),
            ffmpeg_path: Some(
                root.join("missing-ffmpeg.exe")
                    .to_string_lossy()
                    .to_string(),
            ),
        })
        .expect("repair should return ffmpeg status");

        assert!(result.ffmpeg.install_required);
        assert!(result.ffmpeg.install_script.contains("ffmpeg"));
        assert!(!result.ffmpeg.install_attempted);

        fs::remove_dir_all(root).expect("temp output should clean up");
    }

    fn write_bytes(root: &std::path::Path, relative_path: &str, bytes: &[u8]) {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent should be created");
        }
        fs::write(path, bytes).expect("fixture file should be written");
    }

    fn unique_temp_dir() -> std::path::PathBuf {
        static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "rhoiscribe-project-repair-test-{}-{}-{}",
            std::process::id(),
            suffix,
            counter
        ))
    }
}
