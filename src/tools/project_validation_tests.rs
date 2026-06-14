use std::fs;

use super::{ProjectValidationCheck, ProjectValidationRequest, validate_hoi4_project};
use crate::tools::{ScanRoot, test_support::unique_test_dir};

#[test]
fn validation_reports_red_yellow_and_green_checks() {
    let root = unique_test_dir("project-validation");
    write_file(
        &root,
        "common/national_focus/sample_tree.txt",
        "focus_tree = {\n\tid = sample_tree\n\tfocus = { id = sample_rebuild title = sample_rebuild desc = sample_rebuild_desc }\n\tfocus = { id = sample_rebuild }\n",
    );
    write_file(
        &root,
        "interface/sample_interface.gfx",
        "spriteTypes = { spriteType = { name = \"GFX_sample_panel\" texturefile = \"gfx/interface/sample/missing_panel.png\" } }\n",
    );
    write_file(
        &root,
        "interface/sample_interface.gui",
        "guiTypes = { containerWindowType = { name = \"sample_panel\" background = { quadTextureSprite = \"GFX_sample_missing\" } } }\n",
    );
    write_file(
        &root,
        "localisation/simp_chinese/validation_fixture_l_simp_chinese.yml",
        "\u{feff}l_simp_chinese:\n sample_rebuild:0 \"重建\"\n",
    );

    let result = validate_hoi4_project(ProjectValidationRequest {
        roots: vec![ScanRoot {
            path: root.to_string_lossy().to_string(),
            role: Some("mod".to_string()),
        }],
        include_game_roots: Some(true),
    })
    .expect("validation should complete");

    assert_eq!(result.status, "red");
    assert!(result.index_summary.contains("file"));
    assert_red_yellow_green_checks(&result.checks);

    fs::remove_dir_all(root).expect("temp output should clean up");
}

#[test]
fn validation_avoids_gui_name_and_vanilla_texture_false_positives() {
    let mod_root = unique_test_dir("project-validation-mod");
    let game_root = unique_test_dir("project-validation-game");
    write_file(
        &mod_root,
        "interface/sample_interface.gfx",
        "spriteTypes = { spriteType = { name = \"GFX_sample_panel\" texturefile = \"gfx/interface/vanilla/panel.dds\" } }\n",
    );
    write_file(
        &mod_root,
        "interface/sample_interface.gui",
        "guiTypes = { containerWindowType = { name = \"sample_panel\" background = { quadTextureSprite = \"GFX_sample_panel\" } } }\n",
    );
    write_file(
        &game_root,
        "gfx/interface/vanilla/panel.dds",
        "fake texture",
    );

    let result = validate_hoi4_project(ProjectValidationRequest {
        roots: vec![
            ScanRoot {
                path: mod_root.to_string_lossy().to_string(),
                role: Some("mod".to_string()),
            },
            ScanRoot {
                path: game_root.to_string_lossy().to_string(),
                role: Some("game".to_string()),
            },
        ],
        include_game_roots: Some(false),
    })
    .expect("validation should complete");

    assert!(
        !result
            .checks
            .iter()
            .any(|check| check.id == "missing_gfx_texture")
    );
    assert!(
        !result
            .checks
            .iter()
            .any(|check| check.id == "missing_localisation"
                && check.message.contains("sample_panel"))
    );

    fs::remove_dir_all(mod_root).expect("temp output should clean up");
    fs::remove_dir_all(game_root).expect("temp output should clean up");
}

fn write_file(root: &std::path::Path, relative_path: &str, content: &str) {
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixture parent should be created");
    }
    fs::write(path, content).expect("fixture file should be written");
}

fn assert_check(checks: &[ProjectValidationCheck], id: &str, status: &str, text: &str) {
    assert!(checks.iter().any(|check| {
        check.id == id
            && check.status == status
            && (text.is_empty() || check.message.contains(text))
    }));
}

fn assert_red_yellow_green_checks(checks: &[ProjectValidationCheck]) {
    assert_check(checks, "duplicate_definition", "red", "sample_rebuild");
    assert_check(checks, "brace_balance", "red", "");
    assert_check(checks, "missing_gfx_texture", "red", "missing_panel");
    assert_check(checks, "missing_gfx_sprite", "yellow", "GFX_sample_missing");
    assert_check(
        checks,
        "missing_localisation",
        "yellow",
        "sample_rebuild_desc",
    );
    assert_check(checks, "index_completed", "green", "");
}
