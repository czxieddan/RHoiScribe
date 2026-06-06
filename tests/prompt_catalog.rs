use rhoiscribe::prompts::PromptCatalog;
use rmcp::model::PromptMessageContent;
use serde_json::{Map, Value};

#[test]
fn builtin_prompt_catalog_contains_planned_prompts() {
    let catalog = PromptCatalog::builtin();
    let names = catalog.names();

    assert_eq!(
        names,
        vec![
            "hoi4_mod_planner",
            "hoi4_script_writer",
            "hoi4_localisation_writer",
            "hoi4_gui_assistant",
            "hoi4_review"
        ]
    );
}

#[test]
fn mcp_prompt_definitions_include_argument_schema() {
    let catalog = PromptCatalog::builtin();
    let prompts = catalog.to_mcp_prompts();
    let localisation = prompts
        .iter()
        .find(|prompt| prompt.name == "hoi4_localisation_writer")
        .expect("localisation prompt should exist");

    let arguments = localisation
        .arguments
        .as_ref()
        .expect("localisation prompt should declare arguments");

    assert!(
        arguments
            .iter()
            .any(|argument| argument.name == "request" && argument.required == Some(true))
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument.name == "language" && argument.required == Some(false))
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument.name == "key_prefix" && argument.required == Some(false))
    );
}

#[test]
fn rendering_prompt_includes_user_request_and_knowledge_constraints() {
    let catalog = PromptCatalog::builtin();
    let mut arguments = Map::new();
    arguments.insert(
        "request".to_string(),
        Value::String("generate a focus branch for economic recovery".to_string()),
    );

    let result = catalog
        .render("hoi4_mod_planner", &arguments)
        .expect("prompt should render");

    assert_eq!(result.messages.len(), 1);
    let PromptMessageContent::Text { text } = &result.messages[0].content else {
        panic!("prompt should render as text");
    };

    assert!(text.contains("generate a focus branch for economic recovery"));
    assert!(text.contains("HOI4"));
    assert!(text.contains("game-readable"));
}
