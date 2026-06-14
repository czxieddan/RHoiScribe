use std::{ffi::OsStr, fs, path::Path, process::Command};

use serde::{Deserialize, Serialize};

use super::project_files::{ProjectFile, collect_project_files};
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct OggProbe {
    sample_rate: Option<u32>,
    bits_per_sample: Option<u32>,
    channels: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FfmpegDetection {
    command: Option<String>,
    install_attempted: bool,
    install_succeeded: bool,
    install_error: Option<String>,
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
        request.dry_run,
    );

    let (mut checks, mut changes) =
        repair_files(&files, apply, format_scripts, check_media, &ffmpeg)?;

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

fn repair_files(
    files: &[ProjectFile],
    apply: bool,
    format_scripts: bool,
    check_media: bool,
    ffmpeg: &FfmpegStatus,
) -> Result<(Vec<RepairCheck>, Vec<RepairChange>), String> {
    let mut checks = Vec::new();
    let mut changes = Vec::new();

    for file in files {
        repair_encoding(file, apply, &mut checks, &mut changes)?;
        repair_formatting(file, apply, format_scripts, &mut checks, &mut changes)?;
        if check_media {
            check_media_file(file, ffmpeg, &mut checks);
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

fn check_media_file(file: &ProjectFile, ffmpeg: &FfmpegStatus, checks: &mut Vec<RepairCheck>) {
    check_sound_file(file, checks);
    check_music_file(file, ffmpeg, checks);
}

fn check_sound_file(file: &ProjectFile, checks: &mut Vec<RepairCheck>) {
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
}

fn check_music_file(file: &ProjectFile, ffmpeg: &FfmpegStatus, checks: &mut Vec<RepairCheck>) {
    if !file.relative_path.starts_with("music/") || !has_extension(&file.relative_path, "ogg") {
        return;
    }
    if !ffmpeg.available {
        checks.push(music_ogg_probe_check(file));
        return;
    }

    let probe = probe_ogg(file, ffmpeg);
    if !probe.matches_hoi4_music_format() {
        checks.push(music_ogg_format_check(file, &probe));
    }
}

impl OggProbe {
    fn matches_hoi4_music_format(&self) -> bool {
        self.sample_rate == Some(44_100)
            && self.bits_per_sample == Some(32)
            && self.channels == Some(2)
    }
}

fn music_ogg_probe_check(file: &ProjectFile) -> RepairCheck {
    check(
        "music_ogg_probe",
        "yellow",
        "warning",
        &file.relative_path,
        "Cannot verify music OGG sample rate, bit depth, and channels because ffmpeg/ffprobe is not available.",
        Some(
            "Install ffmpeg, then rerun repair_hoi4_project with check_media enabled.".to_string(),
        ),
    )
}

fn music_ogg_format_check(file: &ProjectFile, probe: &OggProbe) -> RepairCheck {
    check(
        "music_ogg_format",
        "yellow",
        "warning",
        &file.relative_path,
        &format!(
            "Music OGG should be 44100 Hz, 32-bit, 2 channels; detected rate={:?}, bits={:?}, channels={:?}.",
            probe.sample_rate, probe.bits_per_sample, probe.channels
        ),
        Some("Use ffmpeg to convert the track to 44100 Hz, 32-bit, stereo OGG.".to_string()),
    )
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
    dry_run: bool,
) -> FfmpegStatus {
    detect_ffmpeg_with_installer(
        requested_path,
        install_requested,
        needed,
        dry_run,
        install_ffmpeg_silently,
    )
}

fn detect_ffmpeg_with_installer(
    requested_path: Option<&str>,
    install_requested: bool,
    needed: bool,
    dry_run: bool,
    installer: fn() -> Result<(), String>,
) -> FfmpegStatus {
    let detection = detect_or_install_ffmpeg(
        requested_path,
        install_requested,
        needed,
        dry_run,
        installer,
    );
    let available = detection.command.is_some();
    let install_required = needed && !available;
    let install_script = ffmpeg_install_script();
    let message = ffmpeg_status_message(
        available,
        detection.install_attempted,
        install_requested,
        install_required,
        dry_run,
        detection.install_error.as_deref(),
    );

    FfmpegStatus {
        available,
        command: detection.command,
        install_required,
        install_attempted: detection.install_attempted,
        install_succeeded: detection.install_succeeded,
        install_error: detection.install_error,
        install_script,
        message,
    }
}

fn detect_or_install_ffmpeg(
    requested_path: Option<&str>,
    install_requested: bool,
    needed: bool,
    dry_run: bool,
    installer: fn() -> Result<(), String>,
) -> FfmpegDetection {
    let mut detection = FfmpegDetection {
        command: ffmpeg_command(requested_path),
        install_attempted: false,
        install_succeeded: false,
        install_error: None,
    };

    if should_attempt_ffmpeg_install(
        needed,
        install_requested,
        dry_run,
        detection.command.is_some(),
    ) {
        detection.install_attempted = true;
        apply_ffmpeg_install_result(&mut detection, requested_path, installer());
    }

    detection
}

fn should_attempt_ffmpeg_install(
    needed: bool,
    install_requested: bool,
    dry_run: bool,
    already_available: bool,
) -> bool {
    needed && !already_available && install_requested && !dry_run
}

fn apply_ffmpeg_install_result(
    detection: &mut FfmpegDetection,
    requested_path: Option<&str>,
    result: Result<(), String>,
) {
    match result {
        Ok(()) => {
            detection.command = ffmpeg_command(requested_path);
            detection.install_succeeded = detection.command.is_some();
            if !detection.install_succeeded {
                detection.install_error = Some(
                    "ffmpeg installer completed, but ffmpeg was not found on PATH afterward"
                        .to_string(),
                );
            }
        }
        Err(error) => detection.install_error = Some(error),
    }
}

fn ffmpeg_status_message(
    available: bool,
    install_attempted: bool,
    install_requested: bool,
    install_required: bool,
    dry_run: bool,
    install_error: Option<&str>,
) -> String {
    if available {
        return if install_attempted {
            "ffmpeg is available after approved silent installation attempt.".to_string()
        } else {
            "ffmpeg is available for media probing.".to_string()
        };
    }

    if install_attempted {
        return format!(
            "Approved silent ffmpeg installation was attempted, but ffmpeg is still unavailable: {}",
            install_error.unwrap_or("unknown installer error")
        );
    }

    ffmpeg_missing_message(install_requested, install_required, dry_run)
}

fn ffmpeg_missing_message(
    install_requested: bool,
    install_required: bool,
    dry_run: bool,
) -> String {
    match (install_requested, install_required, dry_run) {
        (true, true, true) => "ffmpeg is required for media probing. Dry-run mode did not install it; rerun with dry_run=false and install_ffmpeg=true after user approval.",
        (true, true, false) => "ffmpeg is required for media probing. Set install_ffmpeg=true only after user approval to allow a silent installation attempt.",
        (_, true, _) => "ffmpeg is required for full music checks. Ask the user before installing it.",
        _ => "ffmpeg was not needed for this request.",
    }
    .to_string()
}

fn ffmpeg_command(requested_path: Option<&str>) -> Option<String> {
    match requested_path {
        Some(path) => Path::new(path).is_file().then(|| path.to_string()),
        None => command_available("ffmpeg")
            .then(|| "ffmpeg".to_string())
            .or_else(common_windows_ffmpeg_path),
    }
}

fn ffmpeg_install_script() -> String {
    r#"# Requires explicit user approval before running.
if (Get-Command winget -ErrorAction SilentlyContinue) {
    winget install --id Gyan.FFmpeg --source winget --silent --accept-package-agreements --accept-source-agreements
} elseif (Get-Command choco -ErrorAction SilentlyContinue) {
    choco install ffmpeg -y --no-progress
} else {
    Write-Error "Install ffmpeg manually from https://ffmpeg.org/download.html and add it to PATH."
}
"#
    .to_string()
}

fn install_ffmpeg_silently() -> Result<(), String> {
    if cfg!(target_os = "windows") {
        return install_ffmpeg_windows();
    }

    if cfg!(target_os = "macos") {
        return install_ffmpeg_macos();
    }

    install_ffmpeg_linux()
}

fn install_ffmpeg_windows() -> Result<(), String> {
    if command_available("winget") {
        return run_installer(
            "winget",
            &[
                "install",
                "--id",
                "Gyan.FFmpeg",
                "--source",
                "winget",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ],
        );
    }

    if command_available("choco") {
        return run_installer("choco", &["install", "ffmpeg", "-y", "--no-progress"]);
    }

    Err("winget and choco are not available for silent ffmpeg installation".to_string())
}

fn install_ffmpeg_macos() -> Result<(), String> {
    if command_available("brew") {
        return run_installer("brew", &["install", "ffmpeg"]);
    }

    Err("Homebrew is not available for silent ffmpeg installation".to_string())
}

fn install_ffmpeg_linux() -> Result<(), String> {
    if command_available("apt-get") {
        run_installer("sudo", &["-n", "apt-get", "update"])?;
        return run_installer("sudo", &["-n", "apt-get", "install", "-y", "ffmpeg"]);
    }

    if command_available("dnf") {
        return run_installer("sudo", &["-n", "dnf", "install", "-y", "ffmpeg"]);
    }

    if command_available("pacman") {
        return run_installer("sudo", &["-n", "pacman", "-S", "--noconfirm", "ffmpeg"]);
    }

    Err("no supported package manager was found for silent ffmpeg installation".to_string())
}

fn run_installer(command: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|error| format!("failed to run {}: {}", command, error))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = [stderr.trim(), stdout.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    Err(if detail.is_empty() {
        format!("{} exited with status {}", command, output.status)
    } else {
        format!(
            "{} exited with status {}: {}",
            command, output.status, detail
        )
    })
}

fn ffprobe_command(ffmpeg: &str) -> String {
    let path = Path::new(ffmpeg);
    if path.file_stem() == Some(OsStr::new("ffmpeg"))
        && let Some(parent) = path.parent()
    {
        let ffprobe_name = match path.extension().and_then(OsStr::to_str) {
            Some(extension) if !extension.is_empty() => format!("ffprobe.{}", extension),
            _ => "ffprobe".to_string(),
        };
        return parent.join(ffprobe_name).to_string_lossy().to_string();
    }
    "ffprobe".to_string()
}

fn common_windows_ffmpeg_path() -> Option<String> {
    if !cfg!(target_os = "windows") {
        return None;
    }

    [
        r"C:\Program Files\ffmpeg\bin\ffmpeg.exe",
        r"C:\ProgramData\chocolatey\bin\ffmpeg.exe",
        r"C:\tools\ffmpeg\bin\ffmpeg.exe",
    ]
    .into_iter()
    .find(|path| Path::new(path).is_file())
    .map(str::to_string)
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("-version")
        .output()
        .is_ok_and(|output| output.status.success())
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        RepairCheck, RepairHoi4ProjectRequest, RepairHoi4ProjectResult,
        detect_ffmpeg_with_installer, ffprobe_command, repair_hoi4_project,
    };
    use crate::tools::{ScanRoot, test_support::unique_test_dir};

    #[test]
    fn dry_run_reports_encoding_and_media_repairs_without_writing() {
        let root = unique_test_dir("project-repair");
        write_bytes(
            &root,
            "localisation/simp_chinese/repair_fixture_l_simp_chinese.yml",
            b"l_simp_chinese:\n sample_key:0 \"text\"\n",
        );
        write_bytes(
            &root,
            "common/national_focus/sample_focus.txt",
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
        assert_repair_check(&result.checks, "localisation_bom", "yellow");
        assert_repair_check(&result.checks, "script_no_bom", "yellow");
        assert_repair_check(&result.checks, "sound_wav_only", "red");
        assert_repair_check(&result.checks, "music_ogg_probe", "yellow");
        assert_change_planned(&result, "add_utf8_bom");
        assert!(
            !fs::read(root.join("localisation/simp_chinese/repair_fixture_l_simp_chinese.yml"))
                .expect("localisation should remain readable")
                .starts_with(&[0xEF, 0xBB, 0xBF])
        );

        fs::remove_dir_all(root).expect("temp output should clean up");
    }

    #[test]
    fn apply_repairs_bom_rules_and_formats_scripts() {
        let root = unique_test_dir("project-repair");
        write_bytes(
            &root,
            "localisation/english/repair_fixture_l_english.yml",
            b"l_english:\n sample_key:0 \"Text\"\n",
        );
        write_bytes(
            &root,
            "interface/credits.txt",
            b"credits = { name = Test }\n",
        );
        write_bytes(
            &root,
            "common/scripted_effects/sample_effects.txt",
            b"\xEF\xBB\xBFsample_effect={add_political_power=1}",
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
            fs::read(root.join("localisation/english/repair_fixture_l_english.yml"))
                .expect("localisation should read")
                .starts_with(&[0xEF, 0xBB, 0xBF])
        );
        assert!(
            fs::read(root.join("interface/credits.txt"))
                .expect("credits should read")
                .starts_with(&[0xEF, 0xBB, 0xBF])
        );
        let script = fs::read(root.join("common/scripted_effects/sample_effects.txt"))
            .expect("script should read");
        assert!(!script.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert!(String::from_utf8_lossy(&script).contains("sample_effect = {"));

        fs::remove_dir_all(root).expect("temp output should clean up");
    }

    #[test]
    fn format_repair_skips_comments_and_quoted_strings() {
        let root = unique_test_dir("project-repair");
        let original =
            "sample_effect={ log=\"hello world\" # keep this comment\n add_political_power=1 }\n";
        write_bytes(
            &root,
            "common/scripted_effects/sample_effects.txt",
            original.as_bytes(),
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

        assert!(
            result
                .checks
                .iter()
                .any(|check| { check.id == "script_format_skipped" && check.status == "yellow" })
        );
        assert_eq!(
            fs::read_to_string(root.join("common/scripted_effects/sample_effects.txt"))
                .expect("script should read"),
            original
        );

        fs::remove_dir_all(root).expect("temp output should clean up");
    }

    #[test]
    fn ffmpeg_install_request_returns_script_when_missing_in_dry_run() {
        let root = unique_test_dir("project-repair");
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

    #[test]
    fn approved_non_dry_run_attempts_silent_ffmpeg_install_when_missing() {
        let result =
            detect_ffmpeg_with_installer(Some("Z:/missing/ffmpeg.exe"), true, true, false, || {
                Err("installer unavailable in test".to_string())
            });

        assert!(result.install_required);
        assert!(result.install_attempted);
        assert!(!result.install_succeeded);
        assert_eq!(
            result.install_error.as_deref(),
            Some("installer unavailable in test")
        );
        assert!(result.message.contains("attempted"));
    }

    #[test]
    fn dry_run_does_not_attempt_silent_ffmpeg_install_even_when_approved() {
        let result =
            detect_ffmpeg_with_installer(Some("Z:/missing/ffmpeg.exe"), true, true, true, || {
                panic!("dry-run must not run installer")
            });

        assert!(result.install_required);
        assert!(!result.install_attempted);
        assert!(!result.install_succeeded);
        assert!(result.message.contains("Dry-run mode"));
    }

    #[test]
    fn ffprobe_preserves_windows_executable_extension() {
        assert_eq!(
            ffprobe_command(r"C:\tools\ffmpeg\bin\ffmpeg.exe"),
            r"C:\tools\ffmpeg\bin\ffprobe.exe"
        );
        assert!(ffprobe_command("/usr/local/bin/ffmpeg").ends_with("ffprobe"));
    }

    fn write_bytes(root: &std::path::Path, relative_path: &str, bytes: &[u8]) {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent should be created");
        }
        fs::write(path, bytes).expect("fixture file should be written");
    }

    fn assert_repair_check(checks: &[RepairCheck], id: &str, status: &str) {
        assert!(
            checks
                .iter()
                .any(|check| check.id == id && check.status == status)
        );
    }

    fn assert_change_planned(result: &RepairHoi4ProjectResult, action: &str) {
        assert!(
            result
                .changes
                .iter()
                .any(|change| change.action == action && !change.applied)
        );
    }
}
