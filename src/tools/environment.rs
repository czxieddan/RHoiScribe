use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const HOI4_APP_ID: &str = "394360";
const HOI4_FOLDER_NAME: &str = "Hearts of Iron IV";
const DEBUG_EXE_ARGS: [&str; 2] = ["-gdpr-compliant", "-debug_mode"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DiscoverHoi4EnvironmentRequest {
    #[serde(default)]
    pub steam_roots: Vec<String>,
    #[serde(default)]
    pub scan_roots: Vec<String>,
    pub scan_fallback: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hoi4EnvironmentResult {
    pub game_path: Option<String>,
    pub document_path: Option<String>,
    pub version: Option<String>,
    pub source: Option<String>,
    pub valid_game_path: bool,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hoi4DebugRunRequest {
    pub game_path: String,
    pub document_path: String,
    pub workspace_mod_path: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub launch: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hoi4QualityCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hoi4DebugRunResult {
    pub ready: bool,
    pub launched: bool,
    pub pid: Option<u32>,
    pub exe_args: Vec<String>,
    pub checks: Vec<Hoi4QualityCheck>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GamePathCandidate {
    path: PathBuf,
    source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModDescriptor {
    name: Option<String>,
    path: Option<String>,
    dependencies: Vec<String>,
}

pub fn discover_hoi4_environment(
    request: DiscoverHoi4EnvironmentRequest,
) -> Result<Hoi4EnvironmentResult, String> {
    let mut messages = Vec::new();
    let game_candidate = find_from_steam(&request, &mut messages).or_else(|| {
        if request.scan_fallback.unwrap_or(true) {
            find_by_folder_scan(&request, &mut messages)
        } else {
            messages.push("folder scan fallback disabled".to_string());
            None
        }
    });

    let Some(candidate) = game_candidate else {
        messages.push("HOI4 game directory was not found".to_string());
        return Ok(Hoi4EnvironmentResult {
            game_path: None,
            document_path: None,
            version: None,
            source: None,
            valid_game_path: false,
            messages,
        });
    };

    let launcher = launcher_settings(&candidate.path, &mut messages);

    Ok(Hoi4EnvironmentResult {
        game_path: Some(clean_display_path(&candidate.path)),
        document_path: launcher
            .as_ref()
            .and_then(|settings| settings.game_data_path.clone()),
        version: launcher.and_then(|settings| settings.version),
        source: Some(candidate.source),
        valid_game_path: true,
        messages,
    })
}

pub fn validate_hoi4_debug_run(request: Hoi4DebugRunRequest) -> Hoi4DebugRunResult {
    let game_path = PathBuf::from(&request.game_path);
    let document_path = PathBuf::from(&request.document_path);
    let workspace_mod_path = PathBuf::from(&request.workspace_mod_path);
    let mut checks = Vec::new();

    push_check(
        &mut checks,
        "game_path",
        is_valid_game_path(&game_path),
        format!(
            "{} must contain hoi4.exe plus common, history, events, localisation, and map",
            clean_display_path(&game_path)
        ),
    );

    push_check(
        &mut checks,
        "document_path",
        document_path.is_dir(),
        format!(
            "{} must be the HOI4 document data path",
            document_path.display()
        ),
    );

    for folder in ["map", "localisation", "history"] {
        let folder_path = document_path.join(folder);
        let empty = folder_absent_or_empty(&folder_path);
        push_check(
            &mut checks,
            &format!("document_{}_empty", folder),
            empty,
            if folder_path.exists() {
                format!(
                    "{} must be empty before debug launch",
                    folder_path.display()
                )
            } else {
                format!(
                    "{} is absent and has no files to load",
                    folder_path.display()
                )
            },
        );
    }

    let workspace_descriptor_path = workspace_mod_path.join("descriptor.mod");
    let workspace_descriptor = read_descriptor(&workspace_descriptor_path);
    push_check(
        &mut checks,
        "workspace_descriptor",
        workspace_descriptor.is_some(),
        format!("{} must exist", workspace_descriptor_path.display()),
    );

    let mut expected_mod_names = BTreeSet::new();
    if let Some(descriptor) = &workspace_descriptor {
        if let Some(name) = &descriptor.name {
            expected_mod_names.insert(name.clone());
        }
        expected_mod_names.extend(descriptor.dependencies.iter().cloned());
    }
    expected_mod_names.extend(request.dependencies.iter().cloned());

    let document_mod_dir = document_path.join("mod");
    push_check(
        &mut checks,
        "document_mod_folder",
        document_mod_dir.is_dir(),
        format!(
            "{} must contain launcher .mod descriptors",
            document_mod_dir.display()
        ),
    );

    let document_descriptors = read_document_mod_descriptors(&document_mod_dir);
    for expected in &expected_mod_names {
        let exists = document_descriptors
            .iter()
            .any(|(_, descriptor)| descriptor.name.as_deref() == Some(expected));
        push_check(
            &mut checks,
            &format!("mod_descriptor_{}", safe_check_name(expected)),
            exists,
            format!("document mod descriptor for `{}` must exist", expected),
        );
    }

    check_workspace_mod_pointer(
        &mut checks,
        &workspace_mod_path,
        &workspace_descriptor,
        &document_descriptors,
    );
    check_playset(
        &mut checks,
        &document_path,
        &document_descriptors,
        &expected_mod_names,
    );

    let ready = checks.iter().all(|check| check.status == "green");
    let mut launched = false;
    let mut pid = None;
    let mut messages = Vec::new();

    if request.launch.unwrap_or(false) {
        if ready {
            match launch_hoi4(&game_path) {
                Ok(process_id) => {
                    launched = true;
                    pid = Some(process_id);
                    messages.push("hoi4.exe launched with debug arguments".to_string());
                }
                Err(error) => {
                    messages.push(format!("failed to launch hoi4.exe: {}", error));
                }
            }
        } else {
            messages.push("launch skipped because at least one preflight check is red".to_string());
        }
    }

    Hoi4DebugRunResult {
        ready,
        launched,
        pid,
        exe_args: DEBUG_EXE_ARGS
            .iter()
            .map(|arg| (*arg).to_string())
            .collect(),
        checks,
        messages,
    }
}

fn find_from_steam(
    request: &DiscoverHoi4EnvironmentRequest,
    messages: &mut Vec<String>,
) -> Option<GamePathCandidate> {
    let mut steam_roots = request
        .steam_roots
        .iter()
        .map(|path| clean_input_path(path))
        .map(PathBuf::from)
        .collect::<Vec<_>>();

    if steam_roots.is_empty() {
        steam_roots.extend(steam_paths_from_registry(messages));
    }

    let libraries = steam_roots
        .iter()
        .flat_map(|root| steam_library_paths(root, messages))
        .collect::<BTreeSet<_>>();

    for library in libraries {
        let manifest_path = library
            .join("steamapps")
            .join(format!("appmanifest_{}.acf", HOI4_APP_ID));
        let Ok(manifest) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Some(install_dir) = parse_vdf_value(&manifest, "installdir") else {
            messages.push(format!(
                "{} exists but has no installdir field",
                manifest_path.display()
            ));
            continue;
        };
        let candidate = library
            .join("steamapps")
            .join("common")
            .join(clean_input_path(&install_dir));
        if is_valid_game_path(&candidate) {
            return Some(GamePathCandidate {
                path: candidate,
                source: format!("steam manifest {}", manifest_path.display()),
            });
        }
        messages.push(format!(
            "Steam manifest candidate failed validation: {}",
            candidate.display()
        ));
    }

    None
}

#[cfg(windows)]
fn steam_paths_from_registry(messages: &mut Vec<String>) -> Vec<PathBuf> {
    [
        (r"HKCU\Software\Valve\Steam", "SteamPath", "HKCU SteamPath"),
        (
            r"HKLM\SOFTWARE\WOW6432Node\Valve\Steam",
            "InstallPath",
            "HKLM InstallPath",
        ),
    ]
    .iter()
    .filter_map(|(key, value, label)| {
        let path = query_registry_value(key, value);
        if path.is_none() {
            messages.push(format!("Steam registry value not found: {}", label));
        }
        path.map(PathBuf::from)
    })
    .collect()
}

#[cfg(not(windows))]
fn steam_paths_from_registry(messages: &mut Vec<String>) -> Vec<PathBuf> {
    messages.push("Steam registry lookup is only available on Windows".to_string());
    Vec::new()
}

#[cfg(windows)]
fn query_registry_value(key: &str, value: &str) -> Option<String> {
    let output = Command::new("reg")
        .args(["query", key, "/v", value])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(value) {
            continue;
        }
        let fields = trimmed.split_whitespace().collect::<Vec<_>>();
        let registry_type = fields.iter().position(|field| field.starts_with("REG_"))?;
        let path = fields.get(registry_type + 1..)?.join(" ");
        if !path.trim().is_empty() {
            return Some(path);
        }
    }

    None
}

fn steam_library_paths(root: &Path, messages: &mut Vec<String>) -> Vec<PathBuf> {
    let mut libraries = BTreeSet::from([root.to_path_buf()]);
    let library_file = root.join("steamapps").join("libraryfolders.vdf");
    let Ok(content) = fs::read_to_string(&library_file) else {
        messages.push(format!(
            "Steam library file not found: {}",
            library_file.display()
        ));
        return libraries.into_iter().collect();
    };

    for value in parse_vdf_values(&content, "path") {
        libraries.insert(PathBuf::from(clean_input_path(&value)));
    }

    libraries.into_iter().collect()
}

fn find_by_folder_scan(
    request: &DiscoverHoi4EnvironmentRequest,
    messages: &mut Vec<String>,
) -> Option<GamePathCandidate> {
    let roots = if request.scan_roots.is_empty() {
        default_scan_roots()
    } else {
        request
            .scan_roots
            .iter()
            .map(|path| clean_input_path(path))
            .map(PathBuf::from)
            .collect()
    };

    if roots.is_empty() {
        messages.push("folder scan skipped because no scan roots are available".to_string());
        return None;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let handles = roots
        .into_iter()
        .filter(|root| root.is_dir())
        .map(|root| {
            let stop = Arc::clone(&stop);
            thread::spawn(move || scan_root_for_hoi4(root, stop))
        })
        .collect::<Vec<_>>();

    for handle in handles {
        if let Ok(Some(path)) = handle.join() {
            return Some(GamePathCandidate {
                source: format!("folder scan {}", path.display()),
                path,
            });
        }
    }

    None
}

#[cfg(windows)]
fn default_scan_roots() -> Vec<PathBuf> {
    (b'A'..=b'Z')
        .map(|letter| PathBuf::from(format!("{}:\\", letter as char)))
        .filter(|path| path.exists())
        .collect()
}

#[cfg(not(windows))]
fn default_scan_roots() -> Vec<PathBuf> {
    Vec::new()
}

fn scan_root_for_hoi4(root: PathBuf, stop: Arc<AtomicBool>) -> Option<PathBuf> {
    let mut pending = vec![root];

    while let Some(path) = pending.pop() {
        if stop.load(Ordering::Relaxed) {
            return None;
        }

        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(HOI4_FOLDER_NAME))
            && is_valid_game_path(&path)
        {
            stop.store(true, Ordering::Relaxed);
            return Some(path);
        }

        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };

        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() && should_scan_descend(&entry.path()) {
                pending.push(entry.path());
            }
        }
    }

    None
}

fn should_scan_descend(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    !matches!(
        name.to_ascii_lowercase().as_str(),
        "$recycle.bin" | "system volume information" | ".git" | "target" | "node_modules"
    )
}

pub fn is_valid_game_path(path: &Path) -> bool {
    let cleaned = PathBuf::from(clean_input_path(&path.to_string_lossy()));
    cleaned.is_dir()
        && cleaned.join("hoi4.exe").is_file()
        && ["common", "history", "events", "localisation", "map"]
            .iter()
            .all(|relative| cleaned.join(relative).is_dir())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LauncherSettings {
    game_data_path: Option<String>,
    version: Option<String>,
}

fn launcher_settings(path: &Path, messages: &mut Vec<String>) -> Option<LauncherSettings> {
    let settings_path = path.join("launcher-settings.json");
    let content = fs::read_to_string(&settings_path).ok()?;
    let value = serde_json::from_str::<Value>(&content).ok()?;
    let game_data_path = value
        .get("gameDataPath")
        .and_then(Value::as_str)
        .map(clean_input_path);
    let version = value
        .get("version")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    if game_data_path.is_none() {
        messages.push(format!(
            "{} has no gameDataPath field",
            settings_path.display()
        ));
    }

    Some(LauncherSettings {
        game_data_path,
        version,
    })
}

fn folder_absent_or_empty(path: &Path) -> bool {
    if !path.exists() {
        return true;
    }
    if !path.is_dir() {
        return false;
    }
    fs::read_dir(path)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
}

fn read_descriptor(path: &Path) -> Option<ModDescriptor> {
    let content = fs::read_to_string(path).ok()?;
    Some(ModDescriptor {
        name: parse_descriptor_value(&content, "name"),
        path: parse_descriptor_value(&content, "path"),
        dependencies: parse_descriptor_dependencies(&content),
    })
}

fn read_document_mod_descriptors(mod_dir: &Path) -> Vec<(PathBuf, ModDescriptor)> {
    let Ok(entries) = fs::read_dir(mod_dir) else {
        return Vec::new();
    };

    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("mod"))
        .filter_map(|path| read_descriptor(&path).map(|descriptor| (path, descriptor)))
        .collect()
}

fn check_workspace_mod_pointer(
    checks: &mut Vec<Hoi4QualityCheck>,
    workspace_mod_path: &Path,
    workspace_descriptor: &Option<ModDescriptor>,
    document_descriptors: &[(PathBuf, ModDescriptor)],
) {
    let workspace_name = workspace_descriptor
        .as_ref()
        .and_then(|descriptor| descriptor.name.as_deref());
    let matching = document_descriptors.iter().find(|(_, descriptor)| {
        workspace_name
            .map(|name| descriptor.name.as_deref() == Some(name))
            .unwrap_or(false)
    });

    let Some((descriptor_path, descriptor)) = matching else {
        push_check(
            checks,
            "workspace_mod_launcher_descriptor",
            false,
            "launcher .mod descriptor for the workspace mod must exist".to_string(),
        );
        return;
    };

    let Some(path_value) = &descriptor.path else {
        push_check(
            checks,
            "workspace_mod_launcher_path",
            false,
            format!("{} must define path", descriptor_path.display()),
        );
        return;
    };

    push_check(
        checks,
        "workspace_mod_path_slashes",
        !path_value.contains('\\'),
        format!(
            "{} path must use / instead of \\",
            descriptor_path.display()
        ),
    );

    let points_to_workspace = paths_point_to_same_location(path_value, workspace_mod_path);
    push_check(
        checks,
        "workspace_mod_path_target",
        points_to_workspace,
        format!(
            "{} path must point to {}",
            descriptor_path.display(),
            workspace_mod_path.display()
        ),
    );
}

fn check_playset(
    checks: &mut Vec<Hoi4QualityCheck>,
    document_path: &Path,
    document_descriptors: &[(PathBuf, ModDescriptor)],
    expected_mod_names: &BTreeSet<String>,
) {
    let dlc_load_path = document_path.join("dlc_load.json");
    let Ok(content) = fs::read_to_string(&dlc_load_path) else {
        push_check(
            checks,
            "playset_enabled_mods",
            false,
            format!(
                "{} must exist to verify enabled mods",
                dlc_load_path.display()
            ),
        );
        return;
    };

    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        push_check(
            checks,
            "playset_enabled_mods",
            false,
            format!("{} must be readable JSON", dlc_load_path.display()),
        );
        return;
    };

    let enabled_mods = value
        .get("enabled_mods")
        .and_then(Value::as_array)
        .map(|mods| {
            mods.iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let enabled_names = enabled_mods
        .iter()
        .filter_map(|enabled| {
            let file_name = enabled.replace('\\', "/").rsplit('/').next()?.to_string();
            document_descriptors
                .iter()
                .find(|(path, _)| {
                    path.file_name().and_then(|name| name.to_str()) == Some(&file_name)
                })
                .and_then(|(_, descriptor)| descriptor.name.clone())
        })
        .collect::<BTreeSet<_>>();

    push_check(
        checks,
        "playset_enabled_mods",
        &enabled_names == expected_mod_names,
        format!(
            "enabled mods must be exactly [{}], found [{}]",
            expected_mod_names
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            enabled_names.iter().cloned().collect::<Vec<_>>().join(", ")
        ),
    );
}

fn launch_hoi4(game_path: &Path) -> Result<u32, String> {
    Command::new(game_path.join("hoi4.exe"))
        .args(DEBUG_EXE_ARGS)
        .current_dir(game_path)
        .spawn()
        .map(|child| child.id())
        .map_err(|error| error.to_string())
}

fn push_check(checks: &mut Vec<Hoi4QualityCheck>, name: &str, passed: bool, detail: String) {
    checks.push(Hoi4QualityCheck {
        name: name.to_string(),
        status: if passed { "green" } else { "red" }.to_string(),
        detail,
    });
}

fn parse_descriptor_value(content: &str, key: &str) -> Option<String> {
    parse_vdf_value(content, key).or_else(|| parse_assignment_value(content, key))
}

fn parse_descriptor_dependencies(content: &str) -> Vec<String> {
    let tokens = quoted_strings(content);
    if tokens.iter().any(|token| token == "dependencies") {
        return tokens
            .windows(2)
            .filter(|window| window[0] != "name" && window[0] != "path")
            .filter_map(|window| {
                if window[0] == "dependencies" {
                    Some(window[1].clone())
                } else {
                    None
                }
            })
            .collect();
    }

    let Some(start) = content.find("dependencies") else {
        return Vec::new();
    };
    let Some(open) = content[start..].find('{').map(|index| start + index) else {
        return Vec::new();
    };
    let Some(close) = content[open..].find('}').map(|index| open + index) else {
        return Vec::new();
    };

    quoted_strings(&content[open..=close])
}

fn parse_vdf_value(content: &str, key: &str) -> Option<String> {
    parse_vdf_values(content, key).into_iter().next()
}

fn parse_vdf_values(content: &str, key: &str) -> Vec<String> {
    quoted_strings(content)
        .windows(2)
        .filter(|pair| pair[0].eq_ignore_ascii_case(key))
        .map(|pair| pair[1].clone())
        .collect()
}

fn parse_assignment_value(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line
            .split_once('#')
            .map(|(before_comment, _)| before_comment)
            .unwrap_or(line)
            .trim();
        let Some((left, right)) = trimmed.split_once('=') else {
            continue;
        };
        if left.trim() != key {
            continue;
        }
        let value = right.trim().trim_matches('"').trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    None
}

fn quoted_strings(content: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut chars = content.chars().peekable();

    while let Some(character) = chars.next() {
        if character != '"' {
            continue;
        }

        let mut value = String::new();
        let mut escaped = false;
        for next in chars.by_ref() {
            if escaped {
                value.push(match next {
                    'n' => '\n',
                    't' => '\t',
                    '"' => '"',
                    '\\' => '\\',
                    other => other,
                });
                escaped = false;
                continue;
            }
            if next == '\\' {
                escaped = true;
                continue;
            }
            if next == '"' {
                break;
            }
            value.push(next);
        }
        values.push(value);
    }

    values
}

fn paths_point_to_same_location(path_value: &str, expected: &Path) -> bool {
    let path = PathBuf::from(clean_input_path(path_value));
    match (path.canonicalize(), expected.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => {
            clean_input_path(path_value).replace('\\', "/")
                == expected.to_string_lossy().replace('\\', "/")
        }
    }
}

fn safe_check_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn clean_input_path(path: &str) -> String {
    path.trim().trim_matches('"').replace("\\\\", "\\")
}

fn clean_display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::{
        DiscoverHoi4EnvironmentRequest, Hoi4DebugRunRequest, discover_hoi4_environment,
        parse_vdf_value, validate_hoi4_debug_run,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn steam_manifest_discovers_game_path_and_launcher_settings() {
        let root = unique_temp_dir("environment-steam");
        let steam = root.join("Steam");
        let game = steam
            .join("steamapps")
            .join("common")
            .join("Hearts of Iron IV");
        create_valid_game(&game);
        fs::write(
            game.join("launcher-settings.json"),
            r#"{"gameDataPath":"C:\\Users\\Example\\Documents\\Paradox Interactive\\Hearts of Iron IV","version":"1.16.9"}"#,
        )
        .expect("launcher settings should write");
        fs::write(
            steam
                .join("steamapps")
                .join(format!("appmanifest_{}.acf", super::HOI4_APP_ID)),
            r#""AppState" { "appid" "394360" "installdir" "Hearts of Iron IV" }"#,
        )
        .expect("manifest should write");

        let result = discover_hoi4_environment(DiscoverHoi4EnvironmentRequest {
            steam_roots: vec![steam.to_string_lossy().to_string()],
            scan_roots: Vec::new(),
            scan_fallback: Some(false),
        })
        .expect("discovery should succeed");

        assert!(result.valid_game_path);
        assert!(
            result
                .game_path
                .as_deref()
                .unwrap()
                .ends_with("Hearts of Iron IV")
        );
        assert_eq!(result.version.as_deref(), Some("1.16.9"));
        assert!(
            result
                .document_path
                .as_deref()
                .unwrap()
                .contains("Paradox Interactive")
        );

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn folder_scan_fallback_discovers_valid_game_directory() {
        let root = unique_temp_dir("environment-scan");
        let game = root.join("nested").join("Hearts of Iron IV");
        create_valid_game(&game);

        let result = discover_hoi4_environment(DiscoverHoi4EnvironmentRequest {
            steam_roots: vec![root.join("empty_steam").to_string_lossy().to_string()],
            scan_roots: vec![root.to_string_lossy().to_string()],
            scan_fallback: Some(true),
        })
        .expect("discovery should succeed");

        assert!(result.valid_game_path);
        assert!(result.source.as_deref().unwrap().starts_with("folder scan"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn debug_run_validation_checks_document_folders_and_playset() {
        let root = unique_temp_dir("environment-debug");
        let game = root.join("game");
        let docs = root.join("docs");
        let workspace = root.join("workspace_mod");
        create_valid_game(&game);
        fs::create_dir_all(docs.join("mod")).expect("document mod folder should exist");
        fs::create_dir_all(docs.join("map")).expect("map folder should exist");
        fs::create_dir_all(docs.join("localisation")).expect("localisation folder should exist");
        fs::create_dir_all(docs.join("history")).expect("history folder should exist");
        fs::create_dir_all(&workspace).expect("workspace mod should exist");
        fs::write(
            workspace.join("descriptor.mod"),
            "name=\"Workspace Mod\"\ndependencies={ \"Dependency Mod\" }\n",
        )
        .expect("workspace descriptor should write");
        let workspace_path = workspace.to_string_lossy().replace('\\', "/");
        fs::write(
            docs.join("mod").join("workspace.mod"),
            format!("name=\"Workspace Mod\"\npath=\"{}\"\n", workspace_path),
        )
        .expect("workspace launcher mod should write");
        fs::write(
            docs.join("mod").join("dependency.mod"),
            "name=\"Dependency Mod\"\npath=\"C:/mods/dependency\"\n",
        )
        .expect("dependency launcher mod should write");
        fs::write(
            docs.join("dlc_load.json"),
            r#"{"enabled_mods":["mod/workspace.mod","mod/dependency.mod"]}"#,
        )
        .expect("dlc load should write");

        let result = validate_hoi4_debug_run(Hoi4DebugRunRequest {
            game_path: game.to_string_lossy().to_string(),
            document_path: docs.to_string_lossy().to_string(),
            workspace_mod_path: workspace.to_string_lossy().to_string(),
            dependencies: Vec::new(),
            launch: Some(false),
        });

        assert!(result.ready, "{:#?}", result.checks);
        assert!(!result.launched);
        assert_eq!(result.exe_args, vec!["-gdpr-compliant", "-debug_mode"]);

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn vdf_value_parser_reads_installdir() {
        assert_eq!(
            parse_vdf_value(
                r#""AppState" { "installdir" "Hearts of Iron IV" }"#,
                "installdir"
            )
            .as_deref(),
            Some("Hearts of Iron IV")
        );
    }

    fn create_valid_game(path: &Path) {
        fs::create_dir_all(path).expect("game dir should exist");
        fs::write(path.join("hoi4.exe"), "").expect("hoi4 exe should write");
        for folder in ["common", "history", "events", "localisation", "map"] {
            fs::create_dir_all(path.join(folder)).expect("game folder should exist");
        }
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rhoiscribe-{}-{}", prefix, suffix))
    }
}
