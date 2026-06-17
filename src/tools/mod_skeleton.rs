//------------------------------------------------------------------------------------
// mod_skeleton.rs -- Part of RHoiScribe
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

use serde::{Deserialize, Serialize};

use super::{GeneratedFile, ToolError, ToolExecutionResult, finish_generation};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hoi4ModSkeletonRequest {
    pub mod_name: String,
    pub namespace: Option<String>,
    pub supported_version: Option<String>,
    pub language: Option<String>,
    pub dry_run: bool,
    pub output_root: Option<String>,
}

pub fn setup_hoi4_mod_skeleton(
    request: Hoi4ModSkeletonRequest,
) -> Result<ToolExecutionResult, ToolError> {
    let namespace = skeleton_namespace(&request);
    let language = localisation_language_key(request.language.as_deref().unwrap_or("english"));
    let language_dir = language_directory(&language);
    let files = skeleton_files(&request, &namespace, &language, &language_dir);

    finish_generation(request.dry_run, request.output_root.as_deref(), files)
}

fn skeleton_files(
    request: &Hoi4ModSkeletonRequest,
    namespace: &str,
    language: &str,
    language_dir: &str,
) -> Vec<GeneratedFile> {
    vec![
        descriptor_file(request),
        decisions_file(namespace),
        decision_categories_file(namespace),
        events_file(namespace),
        localisation_file(request, namespace, language, language_dir),
        placeholder_file("common/scripted_effects", namespace),
        placeholder_file("common/scripted_triggers", namespace),
        placeholder_file("common/ideas", namespace),
        placeholder_file("interface", namespace),
        placeholder_file("history/countries", namespace),
    ]
}

fn descriptor_file(request: &Hoi4ModSkeletonRequest) -> GeneratedFile {
    let supported_version = request.supported_version.as_deref().unwrap_or("1.19.*");
    GeneratedFile {
        path: "descriptor.mod".to_string(),
        content: format!(
            "name=\"{}\"\nsupported_version=\"{}\"\ntags={{\n\t\"Alternative History\"\n}}\n",
            request.mod_name, supported_version
        ),
        encoding: None,
        summary: "HOI4 descriptor metadata".to_string(),
    }
}

fn decisions_file(namespace: &str) -> GeneratedFile {
    GeneratedFile {
        path: format!("common/decisions/{}_decisions.txt", namespace),
        content: format!(
            "{}_decisions = {{\n\ticon = generic_decisions\n\n\t{}_starter_decision = {{\n\t\ticon = generic_decision\n\t\tcost = 25\n\t\tavailable = {{ always = yes }}\n\t\tcomplete_effect = {{\n\t\t\tadd_political_power = -25\n\t\t}}\n\t}}\n}}\n",
            namespace, namespace
        ),
        encoding: None,
        summary: "Starter HOI4 decision category file".to_string(),
    }
}

fn decision_categories_file(namespace: &str) -> GeneratedFile {
    GeneratedFile {
        path: format!("common/decisions/categories/{}_categories.txt", namespace),
        content: format!(
            "{}_decisions = {{\n\ticon = generic_decisions\n\tpriority = 100\n}}\n",
            namespace
        ),
        encoding: None,
        summary: "Starter HOI4 decision category metadata".to_string(),
    }
}

fn events_file(namespace: &str) -> GeneratedFile {
    GeneratedFile {
        path: format!("events/{}_events.txt", namespace),
        content: format!(
            "namespace = {}\n\ncountry_event = {{\n\tid = {}.1\n\ttitle = {}.1.t\n\tdesc = {}.1.d\n\tis_triggered_only = yes\n\n\toption = {{\n\t\tname = {}.1.a\n\t}}\n}}\n",
            namespace, namespace, namespace, namespace, namespace
        ),
        encoding: None,
        summary: "Starter HOI4 event namespace file".to_string(),
    }
}

fn localisation_file(
    request: &Hoi4ModSkeletonRequest,
    namespace: &str,
    language: &str,
    language_dir: &str,
) -> GeneratedFile {
    GeneratedFile {
        path: format!(
            "localisation/{}/{}_core_{}.yml",
            language_dir, namespace, language
        ),
        content: format!(
            "{}:\n {}_decisions:0 \"{} Decisions\"\n {}_starter_decision:0 \"Open the First Ledger\"\n {}_starter_decision_desc:0 \"The first pages are clean, the ink is fresh, and every office waits for the hand that will give this new order its shape.\"\n {}.1.t:0 \"A Beginning on Paper\"\n {}.1.d:0 \"The work has begun not with thunder, but with ledgers, seals, and quiet rooms where a new design learns to breathe.\"\n {}.1.a:0 \"Let the work begin.\"\n",
            language,
            namespace,
            request.mod_name,
            namespace,
            namespace,
            namespace,
            namespace,
            namespace
        ),
        encoding: Some("utf-8-bom".to_string()),
        summary: "Starter HOI4 localisation file".to_string(),
    }
}

fn placeholder_file(folder: &str, namespace: &str) -> GeneratedFile {
    GeneratedFile {
        path: format!("{}/{}_placeholder.txt", folder, namespace),
        content: format!(
            "# RHoiScribe starter file for {}.\n# Replace this with game-readable HOI4 content when this subsystem is needed.\n",
            folder
        ),
        encoding: None,
        summary: format!("Starter {} marker file", folder),
    }
}

fn skeleton_namespace(request: &Hoi4ModSkeletonRequest) -> String {
    request
        .namespace
        .as_deref()
        .map(ascii_token)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| ascii_token(&request.mod_name))
}

fn ascii_token(value: &str) -> String {
    let token = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();

    token
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn localisation_language_key(language: &str) -> String {
    if language.starts_with("l_") {
        language.to_string()
    } else {
        format!("l_{}", language)
    }
}

fn language_directory(language: &str) -> String {
    language.strip_prefix("l_").unwrap_or(language).to_string()
}
