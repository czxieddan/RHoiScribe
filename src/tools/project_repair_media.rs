use std::{ffi::OsStr, path::Path, process::Command};

use super::{FfmpegStatus, RepairCheck, check, has_extension};
use crate::tools::project_files::ProjectFile;

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

pub(super) fn check_media_file(
    file: &ProjectFile,
    ffmpeg: &FfmpegStatus,
    checks: &mut Vec<RepairCheck>,
) {
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
        return empty_ogg_probe();
    };

    run_ffprobe(command, file)
        .as_deref()
        .map(parse_ogg_probe)
        .unwrap_or_else(empty_ogg_probe)
}

fn run_ffprobe(ffmpeg: &str, file: &ProjectFile) -> Option<String> {
    let output = Command::new(ffprobe_command(ffmpeg))
        .args(ffprobe_args())
        .arg(&file.absolute_path)
        .output()
        .ok()?;

    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

fn ffprobe_args() -> [&'static str; 8] {
    [
        "-v",
        "error",
        "-select_streams",
        "a:0",
        "-show_entries",
        "stream=sample_rate,bits_per_sample,channels",
        "-of",
        "default=noprint_wrappers=1",
    ]
}

fn parse_ogg_probe(text: &str) -> OggProbe {
    let mut probe = empty_ogg_probe();
    for line in text.lines() {
        update_ogg_probe(&mut probe, line);
    }
    probe
}

fn update_ogg_probe(probe: &mut OggProbe, line: &str) {
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

fn empty_ogg_probe() -> OggProbe {
    OggProbe {
        sample_rate: None,
        bits_per_sample: None,
        channels: None,
    }
}

pub(super) fn detect_ffmpeg(
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

pub(super) fn detect_ffmpeg_with_installer(
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
        install_script: ffmpeg_install_script(),
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

    let detail = installer_error_detail(&output.stdout, &output.stderr);
    Err(if detail.is_empty() {
        format!("{} exited with status {}", command, output.status)
    } else {
        format!(
            "{} exited with status {}: {}",
            command, output.status, detail
        )
    })
}

fn installer_error_detail(stdout: &[u8], stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = String::from_utf8_lossy(stdout);
    [stderr.trim(), stdout.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn ffprobe_command(ffmpeg: &str) -> String {
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
