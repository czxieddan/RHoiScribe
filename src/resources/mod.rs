use serde::{Deserialize, Serialize};

pub const MODULE_PURPOSE: &str = "versioned HOI4 knowledge resources";

const EMBEDDED_CATALOG: &str = include_str!("../../knowledge/hoi4/catalog.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeTopic {
    pub id: String,
    pub title: String,
    pub category: String,
    pub file_types: Vec<String>,
    pub tags: Vec<String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeCatalog {
    pub topics: Vec<KnowledgeTopic>,
}

impl KnowledgeCatalog {
    pub fn load_embedded() -> Result<Self, serde_json::Error> {
        serde_json::from_str(EMBEDDED_CATALOG)
    }

    pub fn topic(&self, id: &str) -> Option<&KnowledgeTopic> {
        self.topics.iter().find(|topic| topic.id == id)
    }

    pub fn by_file_type(&self, file_type: &str) -> Vec<&KnowledgeTopic> {
        let needle = file_type.to_ascii_lowercase();

        self.topics
            .iter()
            .filter(|topic| {
                topic
                    .file_types
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(&needle))
            })
            .collect()
    }

    pub fn search(&self, query: &str) -> Vec<&KnowledgeTopic> {
        let terms = query
            .split_whitespace()
            .map(str::to_ascii_lowercase)
            .collect::<Vec<_>>();

        if terms.is_empty() {
            return Vec::new();
        }

        self.topics
            .iter()
            .filter(|topic| {
                let haystack = topic.search_haystack();
                terms.iter().all(|term| haystack.contains(term))
            })
            .collect()
    }
}

impl KnowledgeTopic {
    fn search_haystack(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.id,
            self.title,
            self.category,
            self.file_types.join(" "),
            self.tags.join(" ")
        )
        .to_ascii_lowercase()
            + " "
            + &self.body.to_ascii_lowercase()
    }
}
