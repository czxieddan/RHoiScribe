use std::{collections::BTreeMap, fs, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClassifyErrorLogRequest {
    pub error_log_path: String,
    #[serde(default)]
    pub changed_paths: Vec<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorLogCategory {
    pub category: String,
    pub count: usize,
    pub examples: Vec<ErrorLogEntry>,
    pub likely_changed_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorLogEntry {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorLogClassificationResult {
    pub categories: Vec<ErrorLogCategory>,
    pub total_lines: usize,
    pub error_lines: usize,
    pub messages: Vec<String>,
}

pub fn classify_error_log(
    request: ClassifyErrorLogRequest,
) -> Result<ErrorLogClassificationResult, String> {
    let path = Path::new(&request.error_log_path);
    let content = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {}", path.display(), error))?;
    let limit = request.limit.unwrap_or(5).clamp(1, 20);
    let changed_paths = request
        .changed_paths
        .iter()
        .map(|path| path.replace('\\', "/"))
        .collect::<Vec<_>>();
    let mut buckets = BTreeMap::<String, Vec<ErrorLogEntry>>::new();
    let mut total_lines = 0usize;

    for (index, line) in content.lines().enumerate() {
        total_lines += 1;
        if !looks_like_error_line(line) {
            continue;
        }

        let category = classify_line(line);
        buckets.entry(category).or_default().push(ErrorLogEntry {
            line: index + 1,
            message: line.trim().to_string(),
        });
    }

    let error_lines = buckets.values().map(Vec::len).sum();
    let categories = buckets
        .into_iter()
        .map(|(category, entries)| {
            let likely_changed_paths = likely_changed_paths(&entries, &changed_paths);
            ErrorLogCategory {
                category,
                count: entries.len(),
                examples: entries.into_iter().take(limit).collect(),
                likely_changed_paths,
            }
        })
        .collect();

    Ok(ErrorLogClassificationResult {
        categories,
        total_lines,
        error_lines,
        messages: vec![
            "Use this summary to target the changed files that introduced errors; do not rewrite unrelated files or reset git state.".to_string(),
        ],
    })
}

fn looks_like_error_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("[error")
        || lower.contains(" error ")
        || lower.starts_with("error")
        || lower.contains("exception")
        || lower.contains("failed")
}

fn classify_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();

    if lower.contains("localisation")
        || lower.contains("localization")
        || lower.contains(".yml")
        || lower.contains("invalid yaml")
    {
        "localisation".to_string()
    } else if lower.contains("gui")
        || lower.contains(".gui")
        || lower.contains("sprite")
        || lower.contains(".gfx")
        || lower.contains("texture")
    {
        "interface".to_string()
    } else if lower.contains("focus") || lower.contains("national_focus") {
        "focus".to_string()
    } else if lower.contains("decision") || lower.contains("mission") {
        "decision".to_string()
    } else if lower.contains("event") || lower.contains("namespace") {
        "event".to_string()
    } else if lower.contains("idea") || lower.contains("modifier") {
        "idea_or_modifier".to_string()
    } else if lower.contains("history") || lower.contains("state") || lower.contains("oob") {
        "history".to_string()
    } else if lower.contains("map")
        || lower.contains("province")
        || lower.contains("strategic region")
        || lower.contains("adjacency")
    {
        "map".to_string()
    } else if lower.contains("unknown command")
        || lower.contains("unexpected token")
        || lower.contains("token")
        || lower.contains("database")
    {
        "script_syntax".to_string()
    } else {
        "other".to_string()
    }
}

fn likely_changed_paths(entries: &[ErrorLogEntry], changed_paths: &[String]) -> Vec<String> {
    let messages = entries
        .iter()
        .map(|entry| entry.message.replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    changed_paths
        .iter()
        .filter(|path| messages.contains(&path.to_ascii_lowercase()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ClassifyErrorLogRequest, classify_error_log};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn classifies_error_log_and_links_changed_paths() {
        let path = unique_temp_file();
        fs::write(
            &path,
            "[00:00:01][error.cpp:1]: Error: unexpected token in common/national_focus/CHI.txt\n[00:00:02][localize.cpp:2]: Failed to read localisation/simp_chinese/CHI_l_simp_chinese.yml\n",
        )
        .expect("log should write");

        let result = classify_error_log(ClassifyErrorLogRequest {
            error_log_path: path.to_string_lossy().to_string(),
            changed_paths: vec!["common/national_focus/CHI.txt".to_string()],
            limit: Some(2),
        })
        .expect("classification should succeed");

        assert_eq!(result.error_lines, 2);
        assert!(result.categories.iter().any(|category| {
            category.category == "focus"
                && category
                    .likely_changed_paths
                    .contains(&"common/national_focus/CHI.txt".to_string())
        }));
        assert!(
            result
                .categories
                .iter()
                .any(|category| category.category == "localisation")
        );

        fs::remove_file(path).expect("temp log should clean up");
    }

    fn unique_temp_file() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rhoiscribe-error-log-{}.log", suffix))
    }
}
