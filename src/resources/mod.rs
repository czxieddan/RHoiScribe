//------------------------------------------------------------------------------------
// mod.rs -- Part of RHoiScribe
//
// Copyright (C) 2026 CzXieDdan. All rights reserved.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// https://github.com/czxieddan/RHoiScribe
//------------------------------------------------------------------------------------

use std::{error::Error, fmt};

use rmcp::model::{Annotated, RawResource, ReadResourceResult, Resource, ResourceContents};

mod cwt;
mod knowledge;

pub use cwt::{CWT_CATALOG_URI, CWT_METADATA_URI, CwtResourceCatalog};
pub use knowledge::{KnowledgeCatalog, KnowledgeLoadError, KnowledgeTopic};

pub const MODULE_PURPOSE: &str = "versioned HOI4 knowledge resources";
pub const LATEST_UPDATE_URI: &str = "rhoiscribe://hoi4/latest-update";
pub const KNOWLEDGE_CATALOG_URI: &str = "rhoiscribe://hoi4/knowledge/catalog";
pub const KNOWLEDGE_TOPIC_URI_PREFIX: &str = "rhoiscribe://hoi4/knowledge/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCatalog {
    knowledge: KnowledgeCatalog,
    cwt: CwtResourceCatalog,
    latest_update: knowledge::LatestUpdateResource,
    catalog_index: String,
}

#[derive(Debug)]
pub enum ResourceReadError {
    UnknownUri(String),
}

impl ResourceCatalog {
    pub fn load_embedded() -> Result<Self, KnowledgeLoadError> {
        let knowledge = KnowledgeCatalog::load_embedded()?;
        let catalog_index = knowledge.catalog_index_toml()?;

        Ok(Self {
            knowledge,
            cwt: CwtResourceCatalog::load_embedded(),
            latest_update: knowledge::load_latest_update()?,
            catalog_index,
        })
    }

    pub fn to_mcp_resources(&self) -> Vec<Resource> {
        let mut resources = vec![
            text_resource(
                LATEST_UPDATE_URI,
                "hoi4_latest_update",
                &self.latest_update.title,
                "Static local snapshot of the latest visible official HOI4 update.",
                &self.latest_update.body,
                "text/markdown",
            ),
            text_resource(
                KNOWLEDGE_CATALOG_URI,
                "hoi4_knowledge_catalog",
                "HOI4 knowledge catalog",
                "Structured index of bundled HOI4 Modding knowledge topics.",
                &self.catalog_index,
                "application/toml",
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
        resources.extend(self.cwt.resource_entries().into_iter().map(|entry| {
            text_resource_with_size(
                &entry.uri,
                &entry.name,
                &entry.title,
                &entry.description,
                entry.size,
                entry.mime_type,
            )
        }));

        resources
    }

    pub fn read_text(&self, uri: &str) -> Result<String, ResourceReadError> {
        self.read_text_with_mime(uri).map(|resource| resource.0)
    }

    fn read_text_with_mime(&self, uri: &str) -> Result<(String, &'static str), ResourceReadError> {
        if cwt::is_cwt_resource_uri(uri) {
            let resource = self
                .cwt
                .read_text(uri)
                .ok_or_else(|| ResourceReadError::UnknownUri(uri.to_string()))?;
            return Ok((resource.text, resource.mime_type));
        }

        match uri {
            LATEST_UPDATE_URI => Ok((self.latest_update.body.clone(), "text/markdown")),
            KNOWLEDGE_CATALOG_URI => Ok((self.catalog_index.clone(), "application/toml")),
            _ => {
                let Some(topic_id) = uri.strip_prefix(KNOWLEDGE_TOPIC_URI_PREFIX) else {
                    return Err(ResourceReadError::UnknownUri(uri.to_string()));
                };

                let topic = self
                    .knowledge
                    .topic(topic_id)
                    .ok_or_else(|| ResourceReadError::UnknownUri(uri.to_string()))?;

                Ok((topic_to_markdown(topic), "text/markdown"))
            }
        }
    }

    pub fn read_mcp_resource(&self, uri: &str) -> Result<ReadResourceResult, ResourceReadError> {
        let (text, mime_type) = self.read_text_with_mime(uri)?;

        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(text, uri).with_mime_type(mime_type),
        ]))
    }
}

impl fmt::Display for ResourceReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceReadError::UnknownUri(uri) => write!(formatter, "unknown resource `{}`", uri),
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
    text_resource_with_size(uri, name, title, description, content.len(), mime_type)
}

fn text_resource_with_size(
    uri: &str,
    name: &str,
    title: &str,
    description: &str,
    size: usize,
    mime_type: &str,
) -> Resource {
    Annotated::new(
        RawResource::new(uri, name)
            .with_title(title)
            .with_description(description)
            .with_mime_type(mime_type)
            .with_size(size as u32),
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
