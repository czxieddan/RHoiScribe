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
    category_rules()
        .iter()
        .find(|rule| rule.matches(&lower))
        .map(|rule| rule.category.to_string())
        .unwrap_or_else(|| "other".to_string())
}

struct CategoryRule {
    category: &'static str,
    keywords: &'static [&'static str],
}

impl CategoryRule {
    fn matches(&self, line: &str) -> bool {
        self.keywords.iter().any(|keyword| line.contains(keyword))
    }
}

fn category_rules() -> &'static [CategoryRule] {
    &[
        CategoryRule {
            category: "localisation",
            keywords: &["localisation", "localization", ".yml", "invalid yaml"],
        },
        CategoryRule {
            category: "interface",
            keywords: &["gui", ".gui", "sprite", ".gfx", "texture"],
        },
        CategoryRule {
            category: "focus",
            keywords: &["focus", "national_focus"],
        },
        CategoryRule {
            category: "decision",
            keywords: &["decision", "mission"],
        },
        CategoryRule {
            category: "event",
            keywords: &["event", "namespace"],
        },
        CategoryRule {
            category: "idea_or_modifier",
            keywords: &["idea", "modifier"],
        },
        CategoryRule {
            category: "history",
            keywords: &["history", "state", "oob"],
        },
        CategoryRule {
            category: "map",
            keywords: &["map", "province", "strategic region", "adjacency"],
        },
        CategoryRule {
            category: "script_syntax",
            keywords: &["unknown command", "unexpected token", "token", "database"],
        },
    ]
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
    use crate::tools::test_support::unique_test_dir;
    use std::{fs, path::PathBuf};

    #[test]
    fn classifies_error_log_and_links_changed_paths() {
        let path = unique_temp_file();
        fs::write(
            &path,
            "[00:00:01][error.cpp:1]: Error: unexpected token in common/national_focus/sample_focus.txt\n[00:00:02][localize.cpp:2]: Failed to read localisation/simp_chinese/focus_errors_l_simp_chinese.yml\n",
        )
        .expect("log should write");

        let result = classify_error_log(ClassifyErrorLogRequest {
            error_log_path: path.to_string_lossy().to_string(),
            changed_paths: vec!["common/national_focus/sample_focus.txt".to_string()],
            limit: Some(2),
        })
        .expect("classification should succeed");

        assert_eq!(result.error_lines, 2);
        assert!(result.categories.iter().any(|category| {
            category.category == "focus"
                && category
                    .likely_changed_paths
                    .contains(&"common/national_focus/sample_focus.txt".to_string())
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
        unique_test_dir("error-log").join("error.log")
    }
}
