//------------------------------------------------------------------------------------
// knowledge.rs -- Part of RHoiScribe
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

use std::{collections::HashMap, error::Error, fmt};

use rnmdb_common::ids::PageId;
use rnmdb_storage::{MemoryBackend, Page, PageSize, StorageBackend};
use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/knowledge_sources.rs"));

const SOURCE_FORMAT: &str = "toml";
const DATABASE_BACKEND: &str = "RNMDB in-memory page store";
const LATEST_UPDATE_SOURCE_PATH: &str = "updates/latest-update.toml";
const KNOWLEDGE_PAGE_SIZE_BYTES: usize = 16 * 1024;
const SNAPSHOT_HEADER_PAGE_ID: u64 = 1;
const SNAPSHOT_DATA_START_PAGE_ID: u64 = 2;
const SNAPSHOT_MAGIC: &str = "RHOISCRIBE_KNOWLEDGE_V1";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeCatalog {
    pub topics: Vec<KnowledgeTopic>,
    source_paths: Vec<String>,
    topic_index: HashMap<String, usize>,
    file_type_index: HashMap<String, Vec<usize>>,
    search_documents: Vec<String>,
    runtime_page_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LatestUpdateResource {
    pub(crate) title: String,
    pub(crate) body: String,
}

#[derive(Debug)]
pub enum KnowledgeLoadError {
    MissingLatestUpdate,
    NoTopicSources,
    ParseToml {
        path: String,
        source: toml::de::Error,
    },
    SerializeSnapshot(toml::ser::Error),
    SerializeCatalogIndex(toml::ser::Error),
    DeserializeSnapshot(toml::de::Error),
    DuplicateTopicId(String),
    SourceMismatch(String),
    Rnmdb(String),
    InvalidSnapshot(String),
}

#[derive(Debug, Serialize)]
struct KnowledgeCatalogIndex<'a> {
    source_format: &'static str,
    database_backend: &'static str,
    source_paths: &'a [String],
    topics: Vec<KnowledgeCatalogIndexTopic<'a>>,
}

#[derive(Debug, Serialize)]
struct KnowledgeCatalogIndexTopic<'a> {
    id: &'a str,
    title: &'a str,
    category: &'a str,
    source_path: &'a str,
    tags: &'a [String],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct KnowledgeDatabaseSnapshot {
    topics: Vec<KnowledgeTopic>,
    source_paths: Vec<String>,
}

#[derive(Debug)]
struct LoadedKnowledgeSnapshot {
    snapshot: KnowledgeDatabaseSnapshot,
    page_count: usize,
}

struct KnowledgeMemoryPageDatabase {
    backend: MemoryBackend,
    page_size_bytes: usize,
    page_count: usize,
}

impl KnowledgeCatalog {
    pub fn load_embedded() -> Result<Self, KnowledgeLoadError> {
        let snapshot = parse_embedded_topic_snapshot()?;
        let loaded = KnowledgeMemoryPageDatabase::store_and_load(&snapshot)?;

        Self::from_snapshot(loaded.snapshot, loaded.page_count)
    }

    pub fn source_format(&self) -> &'static str {
        SOURCE_FORMAT
    }

    pub fn database_backend(&self) -> &'static str {
        DATABASE_BACKEND
    }

    pub fn source_paths(&self) -> &[String] {
        &self.source_paths
    }

    pub fn runtime_page_count(&self) -> usize {
        self.runtime_page_count
    }

    pub fn topic(&self, id: &str) -> Option<&KnowledgeTopic> {
        self.topic_index
            .get(id)
            .and_then(|index| self.topics.get(*index))
    }

    pub fn by_file_type(&self, file_type: &str) -> Vec<&KnowledgeTopic> {
        let needle = file_type.to_ascii_lowercase();

        self.file_type_index
            .get(&needle)
            .into_iter()
            .flat_map(|indexes| indexes.iter())
            .filter_map(|index| self.topics.get(*index))
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

        self.search_documents
            .iter()
            .enumerate()
            .filter(|(_, document)| terms.iter().all(|term| document.contains(term)))
            .filter_map(|(index, _)| self.topics.get(index))
            .collect()
    }

    pub(crate) fn catalog_index_toml(&self) -> Result<String, KnowledgeLoadError> {
        let topics = self
            .topics
            .iter()
            .zip(&self.source_paths)
            .map(|(topic, source_path)| KnowledgeCatalogIndexTopic {
                id: &topic.id,
                title: &topic.title,
                category: &topic.category,
                source_path,
                tags: &topic.tags,
            })
            .collect();

        let index = KnowledgeCatalogIndex {
            source_format: self.source_format(),
            database_backend: self.database_backend(),
            source_paths: &self.source_paths,
            topics,
        };

        toml::to_string_pretty(&index).map_err(KnowledgeLoadError::SerializeCatalogIndex)
    }

    fn from_snapshot(
        snapshot: KnowledgeDatabaseSnapshot,
        page_count: usize,
    ) -> Result<Self, KnowledgeLoadError> {
        if snapshot.topics.len() != snapshot.source_paths.len() {
            return Err(KnowledgeLoadError::SourceMismatch(
                "topic and source path counts differ".to_string(),
            ));
        }

        let topic_index = build_topic_index(&snapshot.topics)?;
        let file_type_index = build_file_type_index(&snapshot.topics);
        let search_documents = build_search_documents(&snapshot.topics, &snapshot.source_paths);

        Ok(Self {
            topics: snapshot.topics,
            source_paths: snapshot.source_paths,
            topic_index,
            file_type_index,
            search_documents,
            runtime_page_count: page_count,
        })
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

impl fmt::Display for KnowledgeLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KnowledgeLoadError::MissingLatestUpdate => {
                write!(formatter, "embedded latest update TOML was not found")
            }
            KnowledgeLoadError::NoTopicSources => {
                write!(
                    formatter,
                    "no embedded knowledge topic TOML sources were found"
                )
            }
            KnowledgeLoadError::ParseToml { path, source } => {
                write!(
                    formatter,
                    "failed to parse knowledge TOML `{path}`: {source}"
                )
            }
            KnowledgeLoadError::SerializeSnapshot(error) => {
                write!(
                    formatter,
                    "failed to serialize RNMDB knowledge snapshot: {error}"
                )
            }
            KnowledgeLoadError::SerializeCatalogIndex(error) => {
                write!(
                    formatter,
                    "failed to serialize knowledge catalog index: {error}"
                )
            }
            KnowledgeLoadError::DeserializeSnapshot(error) => {
                write!(
                    formatter,
                    "failed to deserialize RNMDB knowledge snapshot: {error}"
                )
            }
            KnowledgeLoadError::DuplicateTopicId(id) => {
                write!(formatter, "duplicate knowledge topic id `{id}`")
            }
            KnowledgeLoadError::SourceMismatch(message) => {
                write!(formatter, "invalid knowledge source mapping: {message}")
            }
            KnowledgeLoadError::Rnmdb(message) => {
                write!(formatter, "RNMDB knowledge memory store failed: {message}")
            }
            KnowledgeLoadError::InvalidSnapshot(message) => {
                write!(formatter, "invalid RNMDB knowledge snapshot: {message}")
            }
        }
    }
}

impl Error for KnowledgeLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            KnowledgeLoadError::ParseToml { source, .. } => Some(source),
            KnowledgeLoadError::SerializeSnapshot(source) => Some(source),
            KnowledgeLoadError::SerializeCatalogIndex(source) => Some(source),
            KnowledgeLoadError::DeserializeSnapshot(source) => Some(source),
            _ => None,
        }
    }
}

pub(crate) fn load_latest_update() -> Result<LatestUpdateResource, KnowledgeLoadError> {
    let source = EMBEDDED_KNOWLEDGE_SOURCES
        .iter()
        .find(|source| source.path == LATEST_UPDATE_SOURCE_PATH)
        .ok_or(KnowledgeLoadError::MissingLatestUpdate)?;

    parse_toml_source(source)
}

fn parse_embedded_topic_snapshot() -> Result<KnowledgeDatabaseSnapshot, KnowledgeLoadError> {
    let sources = EMBEDDED_KNOWLEDGE_SOURCES
        .iter()
        .filter(|source| is_topic_source(source.path))
        .collect::<Vec<_>>();

    if sources.is_empty() {
        return Err(KnowledgeLoadError::NoTopicSources);
    }

    let mut topics = Vec::with_capacity(sources.len());
    let mut source_paths = Vec::with_capacity(sources.len());
    for source in sources {
        topics.push(parse_toml_source(source)?);
        source_paths.push(source.path.to_string());
    }

    Ok(KnowledgeDatabaseSnapshot {
        topics,
        source_paths,
    })
}

fn parse_toml_source<T>(source: &EmbeddedKnowledgeSource) -> Result<T, KnowledgeLoadError>
where
    T: for<'de> Deserialize<'de>,
{
    toml::from_str(source.content).map_err(|error| KnowledgeLoadError::ParseToml {
        path: source.path.to_string(),
        source: error,
    })
}

fn is_topic_source(path: &str) -> bool {
    path.ends_with(".toml") && !path.starts_with("updates/")
}

fn build_topic_index(
    topics: &[KnowledgeTopic],
) -> Result<HashMap<String, usize>, KnowledgeLoadError> {
    let mut index = HashMap::with_capacity(topics.len());
    for (position, topic) in topics.iter().enumerate() {
        if index.insert(topic.id.clone(), position).is_some() {
            return Err(KnowledgeLoadError::DuplicateTopicId(topic.id.clone()));
        }
    }
    Ok(index)
}

fn build_file_type_index(topics: &[KnowledgeTopic]) -> HashMap<String, Vec<usize>> {
    let mut index: HashMap<String, Vec<usize>> = HashMap::new();
    for (position, topic) in topics.iter().enumerate() {
        for file_type in &topic.file_types {
            index
                .entry(file_type.to_ascii_lowercase())
                .or_default()
                .push(position);
        }
    }
    index
}

fn build_search_documents(topics: &[KnowledgeTopic], source_paths: &[String]) -> Vec<String> {
    topics
        .iter()
        .zip(source_paths)
        .map(|(topic, source_path)| {
            topic.search_haystack() + " " + &source_path.to_ascii_lowercase()
        })
        .collect()
}

impl KnowledgeMemoryPageDatabase {
    fn store_and_load(
        snapshot: &KnowledgeDatabaseSnapshot,
    ) -> Result<LoadedKnowledgeSnapshot, KnowledgeLoadError> {
        let mut database = Self::new();
        database.write_snapshot(snapshot)?;
        let snapshot = database.read_snapshot()?;

        Ok(LoadedKnowledgeSnapshot {
            snapshot,
            page_count: database.page_count,
        })
    }

    fn new() -> Self {
        Self {
            backend: MemoryBackend::new(PageSize::new(KNOWLEDGE_PAGE_SIZE_BYTES)),
            page_size_bytes: KNOWLEDGE_PAGE_SIZE_BYTES,
            page_count: 0,
        }
    }

    fn write_snapshot(
        &mut self,
        snapshot: &KnowledgeDatabaseSnapshot,
    ) -> Result<(), KnowledgeLoadError> {
        let bytes = toml::to_string(snapshot)
            .map_err(KnowledgeLoadError::SerializeSnapshot)?
            .into_bytes();
        let data_page_count = bytes.len().div_ceil(self.page_size_bytes);

        self.write_header(bytes.len(), data_page_count)?;
        for (offset, chunk) in bytes.chunks(self.page_size_bytes).enumerate() {
            self.write_payload_page(SNAPSHOT_DATA_START_PAGE_ID + offset as u64, chunk)?;
        }

        self.backend
            .sync()
            .map_err(|error| KnowledgeLoadError::Rnmdb(error.to_string()))?;
        self.page_count = data_page_count + 1;
        Ok(())
    }

    fn read_snapshot(&self) -> Result<KnowledgeDatabaseSnapshot, KnowledgeLoadError> {
        let header = self
            .read_payload_page(SNAPSHOT_HEADER_PAGE_ID)?
            .ok_or_else(|| {
                KnowledgeLoadError::InvalidSnapshot("missing header page".to_string())
            })?;
        let (byte_len, data_page_count) = parse_header_page(&header)?;
        let mut bytes = Vec::with_capacity(data_page_count * self.page_size_bytes);

        for offset in 0..data_page_count {
            let page_id = SNAPSHOT_DATA_START_PAGE_ID + offset as u64;
            let page = self.read_payload_page(page_id)?.ok_or_else(|| {
                KnowledgeLoadError::InvalidSnapshot(format!("missing data page {page_id}"))
            })?;
            bytes.extend_from_slice(&page);
        }

        bytes.truncate(byte_len);
        let text = String::from_utf8(bytes)
            .map_err(|error| KnowledgeLoadError::InvalidSnapshot(error.to_string()))?;
        toml::from_str(&text).map_err(KnowledgeLoadError::DeserializeSnapshot)
    }

    fn write_header(
        &self,
        byte_len: usize,
        data_page_count: usize,
    ) -> Result<(), KnowledgeLoadError> {
        let header = format!("{SNAPSHOT_MAGIC}\n{byte_len}\n{data_page_count}\n");
        if header.len() > self.page_size_bytes {
            return Err(KnowledgeLoadError::InvalidSnapshot(
                "header does not fit in one RNMDB page".to_string(),
            ));
        }

        self.write_fixed_page(SNAPSHOT_HEADER_PAGE_ID, header.as_bytes())
    }

    fn write_payload_page(&self, page_id: u64, chunk: &[u8]) -> Result<(), KnowledgeLoadError> {
        self.write_fixed_page(page_id, chunk)
    }

    fn write_fixed_page(&self, page_id: u64, content: &[u8]) -> Result<(), KnowledgeLoadError> {
        let mut payload = vec![0_u8; self.page_size_bytes];
        payload[..content.len()].copy_from_slice(content);
        let page = Page::new(PageId::new(page_id), payload)
            .map_err(|error| KnowledgeLoadError::Rnmdb(error.to_string()))?;

        self.backend
            .write_page(page)
            .map_err(|error| KnowledgeLoadError::Rnmdb(error.to_string()))
    }

    fn read_payload_page(&self, page_id: u64) -> Result<Option<Vec<u8>>, KnowledgeLoadError> {
        self.backend
            .read_page(PageId::new(page_id))
            .map(|page| page.map(|page| page.payload().to_vec()))
            .map_err(|error| KnowledgeLoadError::Rnmdb(error.to_string()))
    }
}

fn parse_header_page(page: &[u8]) -> Result<(usize, usize), KnowledgeLoadError> {
    let header_end = page
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(page.len());
    let header = std::str::from_utf8(&page[..header_end])
        .map_err(|error| KnowledgeLoadError::InvalidSnapshot(error.to_string()))?;
    let mut lines = header.lines();

    match lines.next() {
        Some(SNAPSHOT_MAGIC) => {}
        _ => {
            return Err(KnowledgeLoadError::InvalidSnapshot(
                "unexpected snapshot magic".to_string(),
            ));
        }
    }

    let byte_len = parse_header_number(lines.next(), "byte length")?;
    let data_page_count = parse_header_number(lines.next(), "data page count")?;
    Ok((byte_len, data_page_count))
}

fn parse_header_number(
    value: Option<&str>,
    label: &'static str,
) -> Result<usize, KnowledgeLoadError> {
    value
        .ok_or_else(|| KnowledgeLoadError::InvalidSnapshot(format!("missing {label}")))?
        .parse()
        .map_err(|error| KnowledgeLoadError::InvalidSnapshot(format!("invalid {label}: {error}")))
}
