use std::fs;

use super::{GenerateGuiGfxAssetRequest, crc32, generate_gui_gfx_asset};
use crate::tools::test_support::unique_test_dir;

#[test]
fn refuses_generation_without_approval() {
    let result = generate_gui_gfx_asset(asset_request(
        "sample_panel",
        false,
        true,
        Some("button"),
        None,
    ))
    .expect("request should be handled");

    assert!(!result.approved);
    assert!(!result.files.iter().any(|file| file.kind == "png"));
    assert!(
        result
            .messages
            .iter()
            .any(|message| message.contains("approved=true"))
    );
}

#[test]
fn approved_dry_run_returns_png_svg_gfx_and_gui() {
    let result = generate_gui_gfx_asset(GenerateGuiGfxAssetRequest {
        output_root: None,
        asset_name: "sample_command_button".to_string(),
        sprite_name: Some("GFX_sample_command_button".to_string()),
        gui_name: Some("sample_command_button".to_string()),
        width: 128,
        height: 64,
        style: Some("button".to_string()),
        primary_color: Some("#214a67".to_string()),
        secondary_color: Some("#d5b261".to_string()),
        texture: Some("brushed".to_string()),
        shadow: Some(true),
        glow: Some(true),
        emboss: Some(true),
        write_gui: Some(true),
        approved: true,
        dry_run: true,
        relative_directory: Some("gfx/interface/sample".to_string()),
    })
    .expect("asset generation should complete");

    assert!(result.experimental);
    assert!(result.dry_run);
    assert_eq!(result.files.len(), 4);
    assert!(result.files.iter().any(|file| {
        file.kind == "png"
            && file
                .path
                .ends_with("gfx/interface/sample/sample_command_button.png")
            && file
                .content_base64
                .as_deref()
                .is_some_and(|content| content.starts_with("iVBORw0KGgo"))
    }));
    assert!(result.files.iter().any(|file| {
        file.kind == "gfx"
            && file
                .text_content
                .as_deref()
                .unwrap_or("")
                .contains("GFX_sample_command_button")
    }));
    assert!(result.files.iter().any(|file| {
        file.kind == "gui"
            && file
                .text_content
                .as_deref()
                .unwrap_or("")
                .contains("quadTextureSprite")
    }));
}

#[test]
fn tiny_asset_size_does_not_panic() {
    let result = generate_gui_gfx_asset(asset_request(
        "sample_tiny",
        true,
        true,
        Some("button"),
        None,
    ))
    .expect("tiny asset should render safely");

    assert!(result.files.iter().any(|file| file.kind == "png"));
}

#[test]
fn rejects_unsafe_asset_directories() {
    for directory in [
        "gfx/interface/../evil",
        "gfx/interface/bad\"path",
        "gfx/interface/bad\npath",
        r"gfx\interface\bad",
    ] {
        let error = generate_gui_gfx_asset(asset_request(
            "sample_bad",
            true,
            true,
            Some("button"),
            Some(directory),
        ))
        .expect_err("unsafe path should be rejected");

        assert!(error.contains("unsafe relative path"));
    }
}

#[test]
fn uses_relative_directory_for_svg_source_path_and_png_crc() {
    let result = generate_gui_gfx_asset(asset_request(
        "sample_button",
        true,
        true,
        Some("button"),
        Some("gfx/interface/custom"),
    ))
    .expect("asset should render");

    assert!(result.files.iter().any(|file| {
        file.kind == "svg" && file.path == "gfx/interface/custom/source/sample_button.svg"
    }));
    assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
}

#[test]
fn approved_apply_writes_game_files() {
    let root = unique_test_dir("gui-gfx-asset");
    let result = generate_gui_gfx_asset(GenerateGuiGfxAssetRequest {
        output_root: Some(root.to_string_lossy().to_string()),
        asset_name: "sample_status_panel".to_string(),
        sprite_name: None,
        gui_name: None,
        width: 64,
        height: 64,
        style: Some("panel".to_string()),
        primary_color: Some("#34495e".to_string()),
        secondary_color: Some("#9f7f3a".to_string()),
        texture: Some("grid".to_string()),
        shadow: Some(true),
        glow: Some(false),
        emboss: Some(true),
        write_gui: Some(false),
        approved: true,
        dry_run: false,
        relative_directory: None,
    })
    .expect("asset should write");

    assert!(result.applied);
    assert!(
        root.join("gfx/interface/rhoiscribe/sample_status_panel.png")
            .is_file()
    );
    assert!(root.join("interface/sample_status_panel.gfx").is_file());
    let bytes = fs::read(root.join("gfx/interface/rhoiscribe/sample_status_panel.png"))
        .expect("png should read");
    assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));

    fs::remove_dir_all(root).expect("temp output should clean up");
}

fn asset_request(
    asset_name: &str,
    approved: bool,
    dry_run: bool,
    style: Option<&str>,
    relative_directory: Option<&str>,
) -> GenerateGuiGfxAssetRequest {
    GenerateGuiGfxAssetRequest {
        output_root: None,
        asset_name: asset_name.to_string(),
        sprite_name: None,
        gui_name: None,
        width: 64,
        height: 64,
        style: style.map(str::to_string),
        primary_color: Some("#214a67".to_string()),
        secondary_color: Some("#d5b261".to_string()),
        texture: Some("none".to_string()),
        shadow: Some(true),
        glow: Some(true),
        emboss: Some(true),
        write_gui: Some(false),
        approved,
        dry_run,
        relative_directory: relative_directory.map(str::to_string),
    }
}
