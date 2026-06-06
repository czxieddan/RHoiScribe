use rhoiscribe::server::{RhoiScribeServer, SERVER_INSTRUCTIONS, SERVER_NAME};
use rhoiscribe::{hoi4, prompts, resources, tools};

#[test]
fn server_metadata_identifies_rhoiscribe() {
    let server = RhoiScribeServer::new();
    let metadata = server.metadata();

    assert_eq!(metadata.name, SERVER_NAME);
    assert_eq!(metadata.version, env!("CARGO_PKG_VERSION"));
    assert!(metadata.instructions.contains("HOI4 Modding"));
    assert_eq!(metadata.instructions, SERVER_INSTRUCTIONS);
}

#[test]
fn module_boundaries_are_named() {
    assert_eq!(prompts::MODULE_PURPOSE, "agent prompt templates");
    assert_eq!(
        resources::MODULE_PURPOSE,
        "versioned HOI4 knowledge resources"
    );
    assert_eq!(
        tools::MODULE_PURPOSE,
        "batch generation and validation tools"
    );
    assert_eq!(hoi4::MODULE_PURPOSE, "HOI4 script and file conventions");
}
