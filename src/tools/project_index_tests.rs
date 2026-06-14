use std::fs;

use super::{ProjectIndexItem, ProjectIndexRequest, index_hoi4_project};
use crate::tools::{ScanRoot, test_support::unique_test_dir};

#[test]
fn indexes_hoi4_definitions_and_references() {
    let root = unique_test_dir("project-index");
    write_file(
        &root,
        "common/scripted_triggers/sample_triggers.txt",
        "sample_has_system_ready = { has_country_flag = sample_system_ready check_variable = { sample_score > 0 } }\n",
    );
    write_file(
        &root,
        "common/scripted_effects/sample_effects.txt",
        "sample_apply_system = { set_country_flag = sample_system_ready set_variable = { sample_score = 1 } }\n",
    );
    write_file(
        &root,
        "interface/sample_interface.gfx",
        "spriteTypes = { spriteType = { name = \"GFX_sample_panel\" texturefile = \"gfx/interface/sample/panel.png\" } }\n",
    );
    write_file(
        &root,
        "interface/sample_interface.gui",
        "guiTypes = { containerWindowType = { name = \"sample_panel_window\" background = { quadTextureSprite = \"GFX_sample_panel\" } } }\n",
    );
    write_file(
        &root,
        "localisation/simp_chinese/project_index_l_simp_chinese.yml",
        "\u{feff}l_simp_chinese:\n sample_system_ready:0 \"系统\"\n",
    );

    let index = index_hoi4_project(ProjectIndexRequest {
        roots: vec![ScanRoot {
            path: root.to_string_lossy().to_string(),
            role: Some("mod".to_string()),
        }],
        include_game_roots: Some(true),
    })
    .expect("index should build");

    assert_eq!(index.scanned_roots, 1);
    assert!(index.scanned_files >= 5);
    assert_index_item(
        &index.definitions,
        "scripted_trigger",
        "sample_has_system_ready",
    );
    assert_index_item(&index.definitions, "scripted_effect", "sample_apply_system");
    assert_index_item(&index.definitions, "gfx_sprite", "GFX_sample_panel");
    assert_index_item(&index.definitions, "gui_element", "sample_panel_window");
    assert_index_item(&index.references, "country_flag", "sample_system_ready");
    assert_index_item(&index.references, "variable", "sample_score");
    assert_index_item(&index.references, "gfx_sprite", "GFX_sample_panel");

    fs::remove_dir_all(root).expect("temp output should clean up");
}

fn write_file(root: &std::path::Path, relative_path: &str, content: &str) {
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixture parent should be created");
    }
    fs::write(path, content).expect("fixture file should be written");
}

fn assert_index_item(items: &[ProjectIndexItem], kind: &str, name: &str) {
    assert!(
        items
            .iter()
            .any(|item| item.kind == kind && item.name == name)
    );
}
