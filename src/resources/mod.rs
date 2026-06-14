use std::{error::Error, fmt};

use rmcp::model::{Annotated, RawResource, ReadResourceResult, Resource, ResourceContents};
use serde::{Deserialize, Serialize};

pub const MODULE_PURPOSE: &str = "versioned HOI4 knowledge resources";
pub const LATEST_UPDATE_URI: &str = "rhoiscribe://hoi4/latest-update";
pub const KNOWLEDGE_CATALOG_URI: &str = "rhoiscribe://hoi4/knowledge/catalog";
pub const KNOWLEDGE_TOPIC_URI_PREFIX: &str = "rhoiscribe://hoi4/knowledge/";

const EMBEDDED_CATALOG: &str = include_str!("../../resources/knowledge/hoi4/catalog.json");
const LATEST_UPDATE: &str = include_str!("../../resources/knowledge/hoi4/latest-update.md");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeTopic {
    pub id: String,
    pub title: String,
    pub category: String,
    pub file_types: Vec<String>,
    pub tags: Vec<String>,
    pub body: String,
    #[serde(default)]
    pub syntax_blocks: Vec<String>,
    #[serde(default)]
    pub relationships: Vec<String>,
    #[serde(default)]
    pub validation: Vec<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
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
            self.tags.join(" "),
        )
        .to_ascii_lowercase()
            + " "
            + &self.body.to_ascii_lowercase()
            + " "
            + &self.syntax_blocks.join(" ").to_ascii_lowercase()
            + " "
            + &self.relationships.join(" ").to_ascii_lowercase()
            + " "
            + &self.validation.join(" ").to_ascii_lowercase()
            + " "
            + &self.source_refs.join(" ").to_ascii_lowercase()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCatalog {
    knowledge: KnowledgeCatalog,
    latest_update: &'static str,
}

#[derive(Debug)]
pub enum ResourceReadError {
    UnknownUri(String),
    SerializeCatalog(serde_json::Error),
}

impl ResourceCatalog {
    pub fn load_embedded() -> Result<Self, serde_json::Error> {
        Ok(Self {
            knowledge: KnowledgeCatalog::load_embedded()?,
            latest_update: LATEST_UPDATE,
        })
    }

    pub fn to_mcp_resources(&self) -> Vec<Resource> {
        let mut resources = vec![
            text_resource(
                LATEST_UPDATE_URI,
                "hoi4_latest_update",
                "HOI4 latest update snapshot",
                "Static local snapshot of the latest visible official HOI4 update.",
                self.latest_update,
                "text/markdown",
            ),
            text_resource(
                KNOWLEDGE_CATALOG_URI,
                "hoi4_knowledge_catalog",
                "HOI4 knowledge catalog",
                "Structured index of bundled HOI4 Modding knowledge topics.",
                EMBEDDED_CATALOG,
                "application/json",
            ),
        ];

        resources.extend(self.knowledge.topics.iter().map(|topic| {
            let uri = topic_uri(&topic.id);
            text_resource(
                &uri,
                &format!("hoi4_{}", topic.id.replace('.', "_")),
                &topic.title,
                &format!("HOI4 {} guidance.", topic.category),
                &topic_to_markdown(topic),
                "text/markdown",
            )
        }));

        resources
    }

    pub fn read_text(&self, uri: &str) -> Result<String, ResourceReadError> {
        match uri {
            LATEST_UPDATE_URI => Ok(self.latest_update.to_string()),
            KNOWLEDGE_CATALOG_URI => serde_json::to_string_pretty(&self.knowledge)
                .map_err(ResourceReadError::SerializeCatalog),
            _ => {
                let Some(topic_id) = uri.strip_prefix(KNOWLEDGE_TOPIC_URI_PREFIX) else {
                    return Err(ResourceReadError::UnknownUri(uri.to_string()));
                };

                let topic = self
                    .knowledge
                    .topic(topic_id)
                    .ok_or_else(|| ResourceReadError::UnknownUri(uri.to_string()))?;

                Ok(topic_to_markdown(topic))
            }
        }
    }

    pub fn read_mcp_resource(&self, uri: &str) -> Result<ReadResourceResult, ResourceReadError> {
        let mime_type = if uri == KNOWLEDGE_CATALOG_URI {
            "application/json"
        } else {
            "text/markdown"
        };

        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(self.read_text(uri)?, uri).with_mime_type(mime_type),
        ]))
    }
}

impl fmt::Display for ResourceReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceReadError::UnknownUri(uri) => write!(formatter, "unknown resource `{}`", uri),
            ResourceReadError::SerializeCatalog(error) => {
                write!(
                    formatter,
                    "failed to serialize knowledge catalog: {}",
                    error
                )
            }
        }
    }
}

impl Error for ResourceReadError {}

fn text_resource(
    uri: &str,
    name: &str,
    title: &str,
    description: &str,
    content: &str,
    mime_type: &str,
) -> Resource {
    Annotated::new(
        RawResource::new(uri, name)
            .with_title(title)
            .with_description(description)
            .with_mime_type(mime_type)
            .with_size(content.len() as u32),
        None,
    )
}

fn topic_uri(topic_id: &str) -> String {
    format!("{}{}", KNOWLEDGE_TOPIC_URI_PREFIX, topic_id)
}

fn topic_to_markdown(topic: &KnowledgeTopic) -> String {
    let syntax = markdown_list("Syntax blocks", &topic.syntax_blocks);
    let relationships = markdown_list("Relationships", &topic.relationships);
    let validation = markdown_list("Validation", &topic.validation);
    let source_refs = markdown_list("Source references", &topic.source_refs);

    format!(
        "# {}\n\n- ID: {}\n- Category: {}\n- File types: {}\n- Tags: {}\n\n{}\n\n{}{}{}{}",
        topic.title,
        topic.id,
        topic.category,
        topic.file_types.join(", "),
        topic.tags.join(", "),
        topic.body,
        syntax,
        relationships,
        validation,
        source_refs
    )
}

fn markdown_list(title: &str, items: &[String]) -> String {
    if items.is_empty() {
        return String::new();
    }

    let list = items
        .iter()
        .map(|item| format!("- {}", item))
        .collect::<Vec<_>>()
        .join("\n");

    format!("## {}\n\n{}\n\n", title, list)
}

#[cfg(test)]
mod tests {
    use super::{KnowledgeCatalog, LATEST_UPDATE_URI, ResourceCatalog};

    #[test]
    fn latest_update_snapshot_tracks_hoi4_1_19_release() {
        let resources = ResourceCatalog::load_embedded().expect("resources should load");
        let latest = resources
            .read_text(LATEST_UPDATE_URI)
            .expect("latest update should be readable");

        assert!(latest.contains("Snapshot date: 2026-06-12"));
        assert!(latest.contains("1.19.0"));
        assert!(latest.contains("1.19.0.1"));
        assert!(latest.contains("Thunder at Our Gates"));
    }

    #[test]
    fn workflow_topic_documents_red_green_verify_delivery() {
        let catalog = KnowledgeCatalog::load_embedded().expect("catalog should load");
        let topic = catalog
            .topic("workflow.agent_delivery_rules")
            .expect("workflow topic should exist");
        let searchable = format!(
            "{} {} {}",
            topic.body,
            topic.tags.join(" "),
            topic.validation.join(" ")
        );

        assert!(searchable.contains("RED/GREEN/VERIFY"));
        assert!(searchable.contains("verifiable checklist"));
        assert!(searchable.contains("fresh verification"));
    }

    #[test]
    fn visitor_docs_link_to_skill_setup() {
        let readmes = [
            include_str!("../../README.md"),
            include_str!("../../docs/README.zh-CN.md"),
            include_str!("../../docs/README.ru.md"),
            include_str!("../../docs/README.ja.md"),
        ];

        for doc in readmes {
            assert!(doc.contains("SKILL.md"));
            assert!(doc.contains("client-setup.md"));
        }

        let client_setup = include_str!("../../docs/client-setup.md");
        for command in [
            "--skill list-tools",
            "--skill list-resources",
            "--skill list-prompts",
        ] {
            assert!(client_setup.contains(command));
        }

        assert!(client_setup.contains("rhoiscribe-skill-windows-x86_64.zip"));
        assert!(client_setup.contains("rhoiscribe-skill-linux-x86_64.zip"));
        assert!(client_setup.contains("rhoiscribe-skill-macos-universal.zip"));
    }
}
