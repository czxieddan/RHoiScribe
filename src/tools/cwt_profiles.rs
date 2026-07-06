//------------------------------------------------------------------------------------
// cwt_profiles.rs -- Part of RHoiScribe
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

use crate::cwt::rules::read_embedded_hoi4_cwt_sources;

#[derive(Debug, Clone)]
pub(super) struct RuleProfile {
    pub(super) rule_name: &'static str,
    pub(super) type_name: &'static str,
    pub(super) path_kind: &'static str,
    pub(super) scope_context: &'static str,
    pub(super) source_hint: &'static str,
    pub(super) allowed_effects: Vec<&'static str>,
    pub(super) allowed_triggers: Vec<&'static str>,
    pub(super) completions: Vec<CompletionProfile>,
    pub(super) confidence: &'static str,
}

#[derive(Debug, Clone)]
pub(super) struct CompletionProfile {
    pub(super) label: &'static str,
    pub(super) kind: &'static str,
    pub(super) detail: &'static str,
}

pub(super) struct DiagnosticExplanation {
    pub(super) severity: &'static str,
    pub(super) meaning: &'static str,
    pub(super) repair_guidance: &'static str,
    pub(super) source: &'static str,
    pub(super) confidence: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleProfileKind {
    Events,
    NationalFocus,
    ScriptedEffects,
    OnActions,
    Localisation,
    Interface,
    Generic,
}

struct PrefixProfileRule {
    prefixes: &'static [&'static str],
    kind: RuleProfileKind,
}

const PREFIX_PROFILE_RULES: &[PrefixProfileRule] = &[
    PrefixProfileRule {
        prefixes: &["events/"],
        kind: RuleProfileKind::Events,
    },
    PrefixProfileRule {
        prefixes: &["common/national_focus/"],
        kind: RuleProfileKind::NationalFocus,
    },
    PrefixProfileRule {
        prefixes: &["common/scripted_effects/"],
        kind: RuleProfileKind::ScriptedEffects,
    },
    PrefixProfileRule {
        prefixes: &["common/on_actions/"],
        kind: RuleProfileKind::OnActions,
    },
    PrefixProfileRule {
        prefixes: &["localisation/"],
        kind: RuleProfileKind::Localisation,
    },
    PrefixProfileRule {
        prefixes: &["interface/", "gfx/"],
        kind: RuleProfileKind::Interface,
    },
];

pub(super) fn rule_profile_for_path(path: &str) -> RuleProfile {
    profile_kind_for_path(path).profile()
}

pub(super) fn embedded_source_path(source_hint: &str) -> Option<String> {
    read_embedded_hoi4_cwt_sources()
        .ok()?
        .into_iter()
        .find(|source| source.path.ends_with(source_hint) || source.path.contains(source_hint))
        .map(|source| source.path)
}

pub(super) fn diagnostic_explanation(
    code: Option<&str>,
    message: Option<&str>,
) -> DiagnosticExplanation {
    if let Some(explanation) = known_code_explanation(code) {
        return explanation;
    }
    if diagnostic_mentions_unexpected(message) {
        return unexpected_message_explanation();
    }
    generic_diagnostic_explanation()
}

pub(super) fn normalized_code(code: Option<&str>, message: Option<&str>) -> Option<String> {
    code.filter(|code| !code.trim().is_empty())
        .map(str::to_string)
        .or_else(|| message.and_then(code_from_message))
}

pub(super) fn string_vec(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

impl PrefixProfileRule {
    fn matches(&self, path: &str) -> bool {
        self.prefixes.iter().any(|prefix| path.starts_with(prefix))
    }
}

impl RuleProfileKind {
    fn profile(self) -> RuleProfile {
        match self {
            Self::Events => events_profile(),
            Self::NationalFocus => national_focus_profile(),
            Self::ScriptedEffects => scripted_effects_profile(),
            Self::OnActions => on_actions_profile(),
            Self::Localisation => localisation_profile(),
            Self::Interface => interface_profile(),
            Self::Generic => generic_script_profile(),
        }
    }
}

fn profile_kind_for_path(path: &str) -> RuleProfileKind {
    let normalized = normalize_path(path);
    PREFIX_PROFILE_RULES
        .iter()
        .find(|rule| rule.matches(&normalized))
        .map_or(RuleProfileKind::Generic, |rule| rule.kind)
}

fn known_code_explanation(code: Option<&str>) -> Option<DiagnosticExplanation> {
    match code {
        Some("CW263") => Some(DiagnosticExplanation {
            severity: "error",
            meaning: "CWT schema validation found an unexpected field or key for the rule that applies at this path.",
            repair_guidance: "Compare the field against the CWT type rule with inspect_hoi4_type_rule, move it to an allowed block, rename it to a valid HOI4 key, or remove it if it is not supported.",
            source: "cwt_diagnostic_code",
            confidence: "high",
        }),
        Some("CW262") => Some(DiagnosticExplanation {
            severity: "error",
            meaning: "CWT schema validation found a value shape or type that does not match the rule for this key.",
            repair_guidance: "Inspect the type rule, then change the value to the expected scalar, enum, block, scope, or reference form.",
            source: "cwt_diagnostic_code",
            confidence: "high",
        }),
        Some("CW242") => Some(DiagnosticExplanation {
            severity: "warning",
            meaning: "CWT validation reported a missing or unresolved required reference or localisation-style value.",
            repair_guidance: "Use find_hoi4_definition or list_hoi4_workspace_symbols to confirm the referenced identifier exists, then add the missing definition or correct the key.",
            source: "cwt_diagnostic_code",
            confidence: "medium",
        }),
        Some("cwt_parse_error") => Some(DiagnosticExplanation {
            severity: "error",
            meaning: "The Paradox script parser could not build an AST, so schema validation cannot run for this content.",
            repair_guidance: "Fix unmatched braces, missing equals signs, malformed quoted strings, or other script syntax first, then rerun validate_hoi4_file.",
            source: "rhoiscribe_cwt_mapping",
            confidence: "high",
        }),
        _ => None,
    }
}

fn events_profile() -> RuleProfile {
    RuleProfile {
        rule_name: "events",
        type_name: "event",
        path_kind: "event script",
        scope_context: "country/event scope",
        source_hint: "events.cwt",
        allowed_effects: vec![
            "add_political_power",
            "country_event",
            "news_event",
            "set_country_flag",
        ],
        allowed_triggers: vec!["has_country_flag", "tag", "exists"],
        completions: completion_profiles(&[
            ("add_namespace", "keyword", "event namespace declaration"),
            ("country_event", "block", "country event definition"),
            ("news_event", "block", "news event definition"),
            ("state_event", "block", "state event definition"),
            ("unit_event", "block", "unit event definition"),
            ("id", "property", "event id"),
            ("title", "property", "event title localisation key"),
            ("desc", "property", "event description localisation key"),
            ("picture", "property", "event picture"),
            ("is_triggered_only", "property", "event trigger mode"),
            ("trigger", "block", "event trigger block"),
            ("immediate", "block", "event immediate effect block"),
            ("option", "block", "event option block"),
            ("name", "property", "option localisation key"),
            ("ai_chance", "block", "option AI chance block"),
        ]),
        confidence: "medium",
    }
}

fn national_focus_profile() -> RuleProfile {
    RuleProfile {
        rule_name: "national_focus",
        type_name: "focus_tree",
        path_kind: "national focus script",
        scope_context: "country/focus scope",
        source_hint: "national_focus.cwt",
        allowed_effects: vec!["add_political_power", "add_stability", "add_war_support"],
        allowed_triggers: vec!["has_completed_focus", "has_country_flag", "tag"],
        completions: completion_profiles(&[
            ("focus_tree", "block", "focus tree definition"),
            ("focus", "block", "focus definition"),
            ("id", "property", "focus id"),
            ("icon", "property", "focus icon sprite"),
            ("x", "property", "focus x position"),
            ("y", "property", "focus y position"),
            ("cost", "property", "focus cost"),
            ("prerequisite", "block", "focus prerequisite"),
            ("mutually_exclusive", "block", "mutually exclusive focus"),
            ("available", "block", "focus availability trigger"),
            ("completion_reward", "block", "focus reward effect"),
            ("ai_will_do", "block", "AI weighting"),
        ]),
        confidence: "medium",
    }
}

fn scripted_effects_profile() -> RuleProfile {
    RuleProfile {
        rule_name: "scripted_effects",
        type_name: "scripted_effect",
        path_kind: "scripted effect script",
        scope_context: "effect scope",
        source_hint: "effects.cwt",
        allowed_effects: vec!["add_political_power", "set_country_flag", "hidden_effect"],
        allowed_triggers: vec!["has_country_flag", "always"],
        completions: completion_profiles(&[
            (
                "add_political_power",
                "effect",
                "country political power effect",
            ),
            ("set_country_flag", "effect", "set country flag effect"),
            ("hidden_effect", "block", "hidden effect block"),
            ("if", "block", "conditional effect"),
            ("limit", "block", "condition block inside an effect"),
        ]),
        confidence: "medium",
    }
}

fn on_actions_profile() -> RuleProfile {
    RuleProfile {
        rule_name: "on_actions",
        type_name: "on_action",
        path_kind: "on_action script",
        scope_context: "on_action effect scope",
        source_hint: "on_actions.cwt",
        allowed_effects: vec!["country_event", "news_event", "random_events"],
        allowed_triggers: vec!["always", "has_country_flag"],
        completions: completion_profiles(&[
            ("on_startup", "block", "startup on_action"),
            ("effect", "block", "effect payload"),
            ("country_event", "block", "fire country event"),
            ("news_event", "block", "fire news event"),
            ("random_events", "block", "weighted event list"),
        ]),
        confidence: "medium",
    }
}

fn localisation_profile() -> RuleProfile {
    RuleProfile {
        rule_name: "localisation",
        type_name: "localisation_key",
        path_kind: "localisation",
        scope_context: "localisation key/value table",
        source_hint: "localisation.cwt",
        allowed_effects: Vec::new(),
        allowed_triggers: Vec::new(),
        completions: completion_profiles(&[
            (
                "l_english:",
                "keyword",
                "English localisation language header",
            ),
            (":0", "property", "localisation version suffix"),
        ]),
        confidence: "medium",
    }
}

fn interface_profile() -> RuleProfile {
    RuleProfile {
        rule_name: "interface",
        type_name: "gui_or_gfx",
        path_kind: "interface asset",
        scope_context: "GUI/GFX asset scope",
        source_hint: "interface.cwt",
        allowed_effects: Vec::new(),
        allowed_triggers: Vec::new(),
        completions: completion_profiles(&[
            ("spriteType", "block", "sprite definition"),
            ("name", "property", "GUI or sprite name"),
            ("texturefile", "property", "sprite texture path"),
            ("quadTextureSprite", "property", "GUI sprite reference"),
        ]),
        confidence: "medium",
    }
}

fn generic_script_profile() -> RuleProfile {
    RuleProfile {
        rule_name: "generic_hoi4_script",
        type_name: "script",
        path_kind: "HOI4 script",
        scope_context: "unknown or generic script scope",
        source_hint: "settings.cwt",
        allowed_effects: vec!["add_political_power", "set_country_flag"],
        allowed_triggers: vec!["always", "has_country_flag"],
        completions: completion_profiles(&[
            ("if", "block", "conditional block"),
            ("limit", "block", "condition block"),
            (
                "add_political_power",
                "effect",
                "country political power effect",
            ),
            ("set_country_flag", "effect", "set country flag effect"),
        ]),
        confidence: "low",
    }
}

fn completion_profiles(
    items: &[(&'static str, &'static str, &'static str)],
) -> Vec<CompletionProfile> {
    items
        .iter()
        .map(|(label, kind, detail)| CompletionProfile {
            label,
            kind,
            detail,
        })
        .collect()
}

fn diagnostic_mentions_unexpected(message: Option<&str>) -> bool {
    message
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains("unexpected")
}

fn unexpected_message_explanation() -> DiagnosticExplanation {
    DiagnosticExplanation {
        severity: "error",
        meaning: "The diagnostic text indicates the key or value is outside the CWT rule for this context.",
        repair_guidance: "Inspect the applicable type rule and scope, then adjust the key or move it under a valid parent block.",
        source: "diagnostic_message_heuristic",
        confidence: "medium",
    }
}

fn generic_diagnostic_explanation() -> DiagnosticExplanation {
    DiagnosticExplanation {
        severity: "warning",
        meaning: "The diagnostic came from CWT or RHoiScribe language analysis, but this code does not yet have a specialized explanation.",
        repair_guidance: "Use validate_hoi4_file together with inspect_hoi4_type_rule and inspect_hoi4_scope to identify the expected structure.",
        source: "generic_cwt_diagnostic_mapping",
        confidence: "low",
    }
}

fn code_from_message(message: &str) -> Option<String> {
    message
        .split_whitespace()
        .find(|part| part.len() > 2 && part.starts_with("CW") && has_numeric_code_suffix(part))
        .map(str::to_string)
}

fn has_numeric_code_suffix(value: &str) -> bool {
    value[2..]
        .chars()
        .all(|character| character.is_ascii_digit())
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}
