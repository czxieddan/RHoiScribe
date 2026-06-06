use rhoiscribe::resources::KnowledgeCatalog;

#[test]
fn embedded_catalog_contains_required_hoi4_topics() {
    let catalog = KnowledgeCatalog::load_embedded().expect("embedded knowledge should parse");

    assert!(catalog.topic("structure.mod_tree").is_some());
    assert!(catalog.topic("localisation.encoding").is_some());
    assert!(catalog.topic("scripted_gui.dynamic_lists").is_some());
    assert!(catalog.topic("gui.gfx_sprites").is_some());
    assert!(catalog.topic("focus.basic_tree").is_some());
    assert!(catalog.topic("decision.basic_category").is_some());
    assert!(catalog.topic("events.country_event").is_some());
    assert!(catalog.topic("ideas.country_ideas").is_some());
    assert!(catalog.topic("debug.common_errors").is_some());
}

#[test]
fn localisation_topic_records_utf8_bom_requirement() {
    let catalog = KnowledgeCatalog::load_embedded().expect("embedded knowledge should parse");
    let topic = catalog
        .topic("localisation.encoding")
        .expect("localisation topic should exist");

    assert_eq!(topic.category, "localisation");
    assert!(topic.body.contains("UTF-8 BOM"));
    assert!(topic.file_types.contains(&"yml".to_string()));
}

#[test]
fn keyword_search_finds_scripted_gui_temp_variable_guidance() {
    let catalog = KnowledgeCatalog::load_embedded().expect("embedded knowledge should parse");
    let matches = catalog.search("scripted gui temp variable dynamic list");

    assert!(
        matches
            .iter()
            .any(|topic| topic.id == "scripted_gui.dynamic_lists")
    );
}

#[test]
fn file_type_lookup_returns_related_topics() {
    let catalog = KnowledgeCatalog::load_embedded().expect("embedded knowledge should parse");
    let matches = catalog.by_file_type("gui");

    assert!(matches.iter().any(|topic| topic.id == "gui.gfx_sprites"));
    assert!(
        matches
            .iter()
            .any(|topic| topic.id == "scripted_gui.dynamic_lists")
    );
}
