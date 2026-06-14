pub(crate) fn flag_entity_type(key: &str) -> Option<&'static str> {
    let flag_owner = key
        .strip_prefix("set_")
        .or_else(|| key.strip_prefix("has_"))
        .or_else(|| key.strip_prefix("clr_"))
        .or_else(|| key.strip_prefix("modify_"))?
        .strip_suffix("_flag")?;

    flag_owner_kind(flag_owner)
}

fn flag_owner_kind(owner: &str) -> Option<&'static str> {
    [
        (["country"].as_slice(), "country_flag"),
        (["global"].as_slice(), "global_flag"),
        (["state"].as_slice(), "state_flag"),
        (["character", "unit_leader"].as_slice(), "character_flag"),
        (["mio"].as_slice(), "mio_flag"),
        (["project", "facility"].as_slice(), "project_flag"),
    ]
    .into_iter()
    .find(|(aliases, _)| aliases.contains(&owner))
    .map(|(_, entity_type)| entity_type)
}

pub(crate) fn normalize_entity_type(entity_type: &str) -> String {
    let lowered = entity_type.to_ascii_lowercase();
    entity_type_aliases()
        .iter()
        .find(|aliases| aliases.inputs.contains(&lowered.as_str()))
        .map(|aliases| aliases.canonical)
        .unwrap_or(&lowered)
        .to_string()
}

struct EntityTypeAliases {
    canonical: &'static str,
    inputs: &'static [&'static str],
}

const ENTITY_TYPE_ALIASES: &[EntityTypeAliases] = &[
    EntityTypeAliases {
        canonical: "focus_id",
        inputs: &["focus", "national_focus", "focus_id"],
    },
    EntityTypeAliases {
        canonical: "focus_tree_id",
        inputs: &["focus_tree", "focus_tree_id"],
    },
    EntityTypeAliases {
        canonical: "country_tag",
        inputs: &["tag", "country", "country_tag"],
    },
    EntityTypeAliases {
        canonical: "idea_token",
        inputs: &["idea", "idea_token", "national_spirit"],
    },
    EntityTypeAliases {
        canonical: "dynamic_modifier",
        inputs: &["dynamic_modifier", "dynamic_modifier_token"],
    },
    EntityTypeAliases {
        canonical: "decision_category",
        inputs: &["decision_category", "decision_category_id"],
    },
    EntityTypeAliases {
        canonical: "decision",
        inputs: &["decision", "decision_id"],
    },
    EntityTypeAliases {
        canonical: "event_id",
        inputs: &["event", "event_id"],
    },
    EntityTypeAliases {
        canonical: "event_namespace",
        inputs: &["namespace", "event_namespace"],
    },
    EntityTypeAliases {
        canonical: "character_flag",
        inputs: &["character_flag", "unit_leader_flag"],
    },
    EntityTypeAliases {
        canonical: "project_flag",
        inputs: &["project_flag", "facility_flag"],
    },
    EntityTypeAliases {
        canonical: "variable",
        inputs: &["var", "variable", "temp_variable"],
    },
    EntityTypeAliases {
        canonical: "localisation_key",
        inputs: &[
            "loc",
            "localisation",
            "localisation_key",
            "localization_key",
        ],
    },
    EntityTypeAliases {
        canonical: "scripted_effect",
        inputs: &["scripted_effect", "scripted_effect_id"],
    },
    EntityTypeAliases {
        canonical: "scripted_trigger",
        inputs: &["scripted_trigger", "scripted_trigger_id"],
    },
    EntityTypeAliases {
        canonical: "character",
        inputs: &["character", "character_id"],
    },
];

fn entity_type_aliases() -> &'static [EntityTypeAliases] {
    ENTITY_TYPE_ALIASES
}
