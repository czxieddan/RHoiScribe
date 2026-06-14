use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

#[cfg(test)]
use self::gui_gfx_asset_render::crc32;
use self::gui_gfx_asset_render::{
    RenderOptions, base64_decode, base64_encode, encode_png_rgba, render_asset,
};

#[path = "gui_gfx_asset_render.rs"]
mod gui_gfx_asset_render;
#[cfg(test)]
#[path = "gui_gfx_asset_tests.rs"]
mod gui_gfx_asset_tests;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerateGuiGfxAssetRequest {
    pub output_root: Option<String>,
    pub asset_name: String,
    pub sprite_name: Option<String>,
    pub gui_name: Option<String>,
    pub width: u32,
    pub height: u32,
    pub style: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub texture: Option<String>,
    pub shadow: Option<bool>,
    pub glow: Option<bool>,
    pub emboss: Option<bool>,
    pub write_gui: Option<bool>,
    pub approved: bool,
    pub dry_run: bool,
    pub relative_directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeneratedGuiGfxAssetFile {
    pub kind: String,
    pub path: String,
    pub text_content: Option<String>,
    pub content_base64: Option<String>,
    pub encoding: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerateGuiGfxAssetResult {
    pub experimental: bool,
    pub dry_run: bool,
    pub approved: bool,
    pub applied: bool,
    pub files: Vec<GeneratedGuiGfxAssetFile>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Color {
    pub(super) red: u8,
    pub(super) green: u8,
    pub(super) blue: u8,
    pub(super) alpha: u8,
}

#[derive(Debug, Clone)]
struct AssetPlan {
    sprite_name: String,
    gui_name: String,
    width: u32,
    height: u32,
    style: String,
    texture: String,
    primary: Color,
    secondary: Color,
    shadow: bool,
    glow: bool,
    emboss: bool,
    write_gui: bool,
    png_path: String,
    svg_path: String,
    gfx_path: String,
    gui_path: String,
}

pub fn generate_gui_gfx_asset(
    request: GenerateGuiGfxAssetRequest,
) -> Result<GenerateGuiGfxAssetResult, String> {
    if !request.approved {
        return Ok(unapproved_result(request.dry_run));
    }

    let plan = AssetPlan::from_request(&request)?;
    let pixels = render_asset(RenderOptions {
        width: plan.width,
        height: plan.height,
        primary: plan.primary,
        secondary: plan.secondary,
        style: &plan.style,
        texture: &plan.texture,
        shadow: plan.shadow,
        glow: plan.glow,
        emboss: plan.emboss,
    });
    let png = encode_png_rgba(plan.width, plan.height, &pixels)?;
    let files = plan.generated_files(png);

    if !request.dry_run {
        let output_root = request.output_root.as_deref().ok_or_else(|| {
            "output_root is required when dry_run is false for GUI/GFX asset generation".to_string()
        })?;
        write_asset_files(output_root, &files)?;
    }

    Ok(GenerateGuiGfxAssetResult {
        experimental: true,
        dry_run: request.dry_run,
        approved: true,
        applied: !request.dry_run,
        files,
        messages: vec![
            "Experimental local procedural asset generation completed without external image models.".to_string(),
            "Review the generated PNG in game and adjust existing project style if needed.".to_string(),
        ],
    })
}

impl AssetPlan {
    fn from_request(request: &GenerateGuiGfxAssetRequest) -> Result<Self, String> {
        validate_dimension(request.width, "width")?;
        validate_dimension(request.height, "height")?;
        validate_token(&request.asset_name, "asset_name")?;

        let names = AssetNames::from_request(request)?;
        let relative_directory = normalize_asset_directory(request.relative_directory.as_deref())?;
        Ok(Self {
            sprite_name: names.sprite_name,
            gui_name: names.gui_name,
            width: request.width,
            height: request.height,
            style: optional_text(&request.style, "panel"),
            texture: optional_text(&request.texture, "noise"),
            primary: request_color(request.primary_color.as_deref(), DEFAULT_PRIMARY),
            secondary: request_color(request.secondary_color.as_deref(), DEFAULT_SECONDARY),
            shadow: request.shadow.unwrap_or(true),
            glow: request.glow.unwrap_or(false),
            emboss: request.emboss.unwrap_or(true),
            write_gui: request.write_gui.unwrap_or(false),
            png_path: format!("{}/{}.png", relative_directory, request.asset_name),
            svg_path: format!("{}/source/{}.svg", relative_directory, request.asset_name),
            gfx_path: format!("interface/{}.gfx", request.asset_name),
            gui_path: format!("interface/{}.gui", request.asset_name),
        })
    }

    fn generated_files(&self, png: Vec<u8>) -> Vec<GeneratedGuiGfxAssetFile> {
        let mut files = vec![
            binary_file(
                "png",
                &self.png_path,
                base64_encode(&png),
                "Procedural RGBA PNG texture.",
            ),
            text_file(
                "svg",
                &self.svg_path,
                generate_svg(
                    self.width,
                    self.height,
                    self.primary,
                    self.secondary,
                    &self.style,
                    &self.texture,
                ),
                "Editable procedural SVG source approximation.",
            ),
            text_file(
                "gfx",
                &self.gfx_path,
                generate_gfx(&self.sprite_name, &self.png_path),
                "HOI4 spriteType registration.",
            ),
        ];

        if self.write_gui {
            files.push(text_file(
                "gui",
                &self.gui_path,
                generate_gui(&self.gui_name, &self.sprite_name, self.width, self.height),
                "Optional HOI4 GUI iconType block using the generated sprite.",
            ));
        }

        files
    }
}

struct AssetNames {
    sprite_name: String,
    gui_name: String,
}

impl AssetNames {
    fn from_request(request: &GenerateGuiGfxAssetRequest) -> Result<Self, String> {
        let sprite_name = request
            .sprite_name
            .clone()
            .unwrap_or_else(|| format!("GFX_{}", request.asset_name));
        let gui_name = request
            .gui_name
            .clone()
            .unwrap_or_else(|| request.asset_name.clone());
        validate_token(&sprite_name, "sprite_name")?;
        validate_token(&gui_name, "gui_name")?;
        Ok(Self {
            sprite_name,
            gui_name,
        })
    }
}

fn optional_text(value: &Option<String>, default: &str) -> String {
    value.clone().unwrap_or_else(|| default.to_string())
}

fn request_color(value: Option<&str>, default: Color) -> Color {
    parse_color(value).unwrap_or(default)
}

const DEFAULT_PRIMARY: Color = Color {
    red: 49,
    green: 82,
    blue: 113,
    alpha: 255,
};

const DEFAULT_SECONDARY: Color = Color {
    red: 205,
    green: 170,
    blue: 92,
    alpha: 255,
};

fn unapproved_result(dry_run: bool) -> GenerateGuiGfxAssetResult {
    GenerateGuiGfxAssetResult {
        experimental: true,
        dry_run,
        approved: false,
        applied: false,
        files: Vec::new(),
        messages: vec![
            "Experimental GUI/GFX generation requires approved=true. Prefer existing project assets unless the user approved new procedural art.".to_string(),
        ],
    }
}

fn binary_file(
    kind: &str,
    path: &str,
    content_base64: String,
    summary: &str,
) -> GeneratedGuiGfxAssetFile {
    GeneratedGuiGfxAssetFile {
        kind: kind.to_string(),
        path: path.to_string(),
        text_content: None,
        content_base64: Some(content_base64),
        encoding: Some("binary".to_string()),
        summary: summary.to_string(),
    }
}

fn text_file(
    kind: &str,
    path: &str,
    text_content: String,
    summary: &str,
) -> GeneratedGuiGfxAssetFile {
    GeneratedGuiGfxAssetFile {
        kind: kind.to_string(),
        path: path.to_string(),
        text_content: Some(text_content),
        content_base64: None,
        encoding: Some("utf-8".to_string()),
        summary: summary.to_string(),
    }
}

fn generate_svg(
    width: u32,
    height: u32,
    primary: Color,
    secondary: Color,
    style: &str,
    texture: &str,
) -> String {
    let radius = if style == "button" {
        height / 5
    } else {
        width.min(height) / 12
    };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n\
  <defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0\" stop-color=\"#{p}\"/><stop offset=\"1\" stop-color=\"#{s}\"/></linearGradient></defs>\n\
  <rect x=\"2\" y=\"2\" width=\"{w}\" height=\"{h}\" rx=\"{radius}\" fill=\"url(#g)\" stroke=\"#111820\" stroke-width=\"2\"/>\n\
  <path d=\"M 6 6 L {edge} 6\" stroke=\"#ffffff\" stroke-opacity=\"0.28\"/>\n\
  <text x=\"8\" y=\"{label_y}\" font-family=\"sans-serif\" font-size=\"8\" fill=\"#ffffff\" opacity=\"0.45\">{texture}</text>\n\
</svg>\n",
        width = width,
        height = height,
        w = width.saturating_sub(4),
        h = height.saturating_sub(4),
        edge = width.saturating_sub(6),
        label_y = height.saturating_sub(8),
        p = color_hex(primary),
        s = color_hex(secondary),
        texture = escape_xml(texture)
    )
}

fn generate_gfx(sprite_name: &str, texture_path: &str) -> String {
    format!(
        "spriteTypes = {{\n\tSpriteType = {{\n\t\tname = \"{}\"\n\t\ttexturefile = \"{}\"\n\t}}\n}}\n",
        sprite_name, texture_path
    )
}

fn generate_gui(gui_name: &str, sprite_name: &str, width: u32, height: u32) -> String {
    format!(
        "guiTypes = {{\n\ticonType = {{\n\t\tname = \"{}\"\n\t\tposition = {{ x = 0 y = 0 }}\n\t\tquadTextureSprite = \"{}\"\n\t\tOrientation = \"UPPER_LEFT\"\n\t\talwaystransparent = no\n\t\tscale = 1.0\n\t}}\n}}\n# size: {}x{}\n",
        gui_name, sprite_name, width, height
    )
}

fn write_asset_files(output_root: &str, files: &[GeneratedGuiGfxAssetFile]) -> Result<(), String> {
    for file in files {
        write_asset_file(output_root, file)?;
    }
    Ok(())
}

fn write_asset_file(output_root: &str, file: &GeneratedGuiGfxAssetFile) -> Result<(), String> {
    validate_relative_path(&file.path)?;
    let path = Path::new(output_root).join(&file.path);
    ensure_parent_directory(&path)?;
    if let Some(bytes) = asset_file_bytes(file)? {
        fs::write(&path, bytes)
            .map_err(|error| format!("failed to write {}: {}", path.display(), error))?;
    }
    Ok(())
}

fn ensure_parent_directory(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {}", parent.display(), error))?;
    }
    Ok(())
}

fn asset_file_bytes(file: &GeneratedGuiGfxAssetFile) -> Result<Option<Vec<u8>>, String> {
    if let Some(text) = &file.text_content {
        return Ok(Some(text.as_bytes().to_vec()));
    }
    file.content_base64
        .as_deref()
        .map(base64_decode)
        .transpose()
}

fn normalize_asset_directory(directory: Option<&str>) -> Result<String, String> {
    let raw_directory = directory.unwrap_or("gfx/interface/rhoiscribe");
    let directory = raw_directory.trim_matches('/').to_string();
    validate_relative_path(&directory)?;
    if !directory.starts_with("gfx/") {
        return Err("relative_directory must stay under gfx/".to_string());
    }
    Ok(directory)
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty()
        || path.starts_with('/')
        || path.contains(':')
        || path.contains('\\')
        || path
            .chars()
            .any(|character| character.is_control() || character == '"')
        || path.split('/').any(|segment| segment == "..")
    {
        return Err(format!("unsafe relative path `{}`", path));
    }
    Ok(())
}

fn validate_dimension(value: u32, name: &str) -> Result<(), String> {
    if !(1..=1024).contains(&value) {
        return Err(format!("{} must be between 1 and 1024", name));
    }
    Ok(())
}

fn validate_token(value: &str, name: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(format!("{} must be a non-empty ASCII token", name));
    }
    Ok(())
}

fn parse_color(value: Option<&str>) -> Option<Color> {
    let value = value?.trim().strip_prefix('#').unwrap_or(value?.trim());
    if value.len() != 6 {
        return None;
    }
    Some(Color {
        red: u8::from_str_radix(&value[0..2], 16).ok()?,
        green: u8::from_str_radix(&value[2..4], 16).ok()?,
        blue: u8::from_str_radix(&value[4..6], 16).ok()?,
        alpha: 255,
    })
}

pub(super) fn color_hex(color: Color) -> String {
    format!("{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
