use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

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
struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
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

        let relative_directory = normalize_asset_directory(request.relative_directory.as_deref())?;
        Ok(Self {
            sprite_name,
            gui_name,
            width: request.width,
            height: request.height,
            style: request.style.clone().unwrap_or_else(|| "panel".to_string()),
            texture: request
                .texture
                .clone()
                .unwrap_or_else(|| "noise".to_string()),
            primary: parse_color(request.primary_color.as_deref()).unwrap_or(DEFAULT_PRIMARY),
            secondary: parse_color(request.secondary_color.as_deref()).unwrap_or(DEFAULT_SECONDARY),
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

struct RenderOptions<'a> {
    width: u32,
    height: u32,
    primary: Color,
    secondary: Color,
    style: &'a str,
    texture: &'a str,
    shadow: bool,
    glow: bool,
    emboss: bool,
}

fn render_asset(options: RenderOptions<'_>) -> Vec<u8> {
    let pixel_count = (options.width * options.height) as usize;
    let mut pixels = vec![0u8; pixel_count * 4];
    let radius = corner_radius(&options);

    for y in 0..options.height {
        for x in 0..options.width {
            let color = render_pixel(&options, x, y, radius);
            let index = ((y * options.width + x) * 4) as usize;
            pixels[index] = color.red;
            pixels[index + 1] = color.green;
            pixels[index + 2] = color.blue;
            pixels[index + 3] = color.alpha;
        }
    }

    pixels
}

fn corner_radius(options: &RenderOptions<'_>) -> f32 {
    match options.style {
        "button" => (options.height as f32 * 0.22).clamp(3.0, 14.0),
        "badge" => (options.width.min(options.height) as f32) * 0.5,
        _ => (options.width.min(options.height) as f32 * 0.08).clamp(2.0, 10.0),
    }
}

fn render_pixel(options: &RenderOptions<'_>, x: u32, y: u32, radius: f32) -> Color {
    let fx = normalized_coordinate(x, options.width);
    let fy = normalized_coordinate(y, options.height);
    let inside = rounded_rect_contains(
        x as f32,
        y as f32,
        options.width as f32,
        options.height as f32,
        radius,
    );

    if inside {
        render_inner_pixel(options, x, y, fx, fy, radius)
    } else if options.shadow || options.glow {
        render_outer_pixel(options, x, y, radius)
    } else {
        TRANSPARENT
    }
}

fn normalized_coordinate(value: u32, size: u32) -> f32 {
    if size <= 1 {
        0.0
    } else {
        value as f32 / (size - 1) as f32
    }
}

fn render_inner_pixel(
    options: &RenderOptions<'_>,
    x: u32,
    y: u32,
    fx: f32,
    fy: f32,
    radius: f32,
) -> Color {
    let mut color = mix(
        options.primary,
        options.secondary,
        (fx * 0.35 + fy * 0.65).clamp(0.0, 1.0),
    );
    apply_texture(&mut color, options.texture, x, y);
    if options.emboss {
        apply_emboss(&mut color, fx, fy);
    }
    apply_border(&mut color, x, y, options.width, options.height, radius);
    color
}

fn render_outer_pixel(options: &RenderOptions<'_>, x: u32, y: u32, radius: f32) -> Color {
    outer_effect_color(OuterEffectOptions {
        x: x as f32,
        y: y as f32,
        width: options.width as f32,
        height: options.height as f32,
        radius,
        shadow: options.shadow,
        glow: options.glow,
        secondary: options.secondary,
    })
}

const TRANSPARENT: Color = Color {
    red: 0,
    green: 0,
    blue: 0,
    alpha: 0,
};

fn rounded_rect_contains(x: f32, y: f32, width: f32, height: f32, radius: f32) -> bool {
    if width <= 0.0 || height <= 0.0 {
        return false;
    }

    let max_radius = ((width - 1.0).max(0.0).min((height - 1.0).max(0.0))) * 0.5;
    let radius = radius.clamp(0.0, max_radius);
    let min_x = radius;
    let max_x = (width - radius - 1.0).max(min_x);
    let min_y = radius;
    let max_y = (height - radius - 1.0).max(min_y);
    let inner_x = x.clamp(min_x, max_x);
    let inner_y = y.clamp(min_y, max_y);
    let dx = x - inner_x;
    let dy = y - inner_y;
    dx * dx + dy * dy <= radius * radius
}

struct OuterEffectOptions {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    shadow: bool,
    glow: bool,
    secondary: Color,
}

fn outer_effect_color(options: OuterEffectOptions) -> Color {
    let shifted_x = if options.shadow {
        options.x - 2.0
    } else {
        options.x
    };
    let shifted_y = if options.shadow {
        options.y - 2.0
    } else {
        options.y
    };
    let near = rounded_rect_contains(
        shifted_x,
        shifted_y,
        options.width,
        options.height,
        options.radius + 2.0,
    );
    if options.shadow && near {
        return Color {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 75,
        };
    }

    if options.glow
        && rounded_rect_contains(
            options.x,
            options.y,
            options.width,
            options.height,
            options.radius + 4.0,
        )
    {
        return Color {
            red: options.secondary.red,
            green: options.secondary.green,
            blue: options.secondary.blue,
            alpha: 55,
        };
    }

    Color {
        red: 0,
        green: 0,
        blue: 0,
        alpha: 0,
    }
}

fn apply_texture(color: &mut Color, texture: &str, x: u32, y: u32) {
    let noise = pseudo_noise(x, y) as i16 - 128;
    let delta = match texture {
        "grid" if x.is_multiple_of(8) || y.is_multiple_of(8) => 18,
        "brushed" => ((x as i16 % 9) - 4) * 3,
        "none" => 0,
        _ => noise / 12,
    };
    color.red = add_channel(color.red, delta);
    color.green = add_channel(color.green, delta);
    color.blue = add_channel(color.blue, delta);
}

fn apply_emboss(color: &mut Color, fx: f32, fy: f32) {
    let delta = if fx + fy < 0.45 {
        24
    } else if fx + fy > 1.55 {
        -24
    } else {
        0
    };
    color.red = add_channel(color.red, delta);
    color.green = add_channel(color.green, delta);
    color.blue = add_channel(color.blue, delta);
}

fn apply_border(color: &mut Color, x: u32, y: u32, width: u32, height: u32, radius: f32) {
    let edge = x < 2
        || y < 2
        || x + 3 > width
        || y + 3 > height
        || !rounded_rect_contains(
            x as f32,
            y as f32,
            width as f32,
            height as f32,
            (radius - 2.0).max(1.0),
        );
    if edge {
        color.red = add_channel(color.red, -38);
        color.green = add_channel(color.green, -38);
        color.blue = add_channel(color.blue, -38);
    }
}

fn pseudo_noise(x: u32, y: u32) -> u8 {
    let mut value = x
        .wrapping_mul(1_103_515_245)
        .wrapping_add(y.wrapping_mul(12_345))
        .wrapping_add(0x9e37_79b9);
    value ^= value >> 16;
    (value & 0xff) as u8
}

fn add_channel(channel: u8, delta: i16) -> u8 {
    (channel as i16 + delta).clamp(0, 255) as u8
}

fn mix(left: Color, right: Color, amount: f32) -> Color {
    let inverse = 1.0 - amount;
    Color {
        red: (left.red as f32 * inverse + right.red as f32 * amount) as u8,
        green: (left.green as f32 * inverse + right.green as f32 * amount) as u8,
        blue: (left.blue as f32 * inverse + right.blue as f32 * amount) as u8,
        alpha: (left.alpha as f32 * inverse + right.alpha as f32 * amount) as u8,
    }
}

fn encode_png_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    if rgba.len() != (width as usize * height as usize * 4) {
        return Err("rgba buffer length does not match dimensions".to_string());
    }

    let stride = width as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for row in rgba.chunks(stride) {
        raw.push(0);
        raw.extend_from_slice(row);
    }

    let mut png = Vec::new();
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_png_chunk(&mut png, b"IHDR", &ihdr);

    let zlib = zlib_store(&raw);
    write_png_chunk(&mut png, b"IDAT", &zlib);
    write_png_chunk(&mut png, b"IEND", &[]);
    Ok(png)
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut output = vec![0x78, 0x01];
    let mut offset = 0usize;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let block_len = remaining.min(65_535);
        let final_block = offset + block_len >= data.len();
        output.push(if final_block { 0x01 } else { 0x00 });
        let len = block_len as u16;
        output.extend_from_slice(&len.to_le_bytes());
        output.extend_from_slice(&(!len).to_le_bytes());
        output.extend_from_slice(&data[offset..offset + block_len]);
        offset += block_len;
    }
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

fn write_png_chunk(output: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(chunk_type);
    output.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(chunk_type.len() + data.len());
    crc_input.extend_from_slice(chunk_type);
    crc_input.extend_from_slice(data);
    output.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + u32::from(*byte)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = if crc & 1 == 1 { 0xedb88320 } else { 0 };
            crc = (crc >> 1) ^ mask;
        }
    }
    !crc
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
        validate_relative_path(&file.path)?;
        let path = Path::new(output_root).join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {}", parent.display(), error))?;
        }

        if let Some(text) = &file.text_content {
            fs::write(&path, text.as_bytes())
                .map_err(|error| format!("failed to write {}: {}", path.display(), error))?;
        } else if let Some(content) = &file.content_base64 {
            let bytes = base64_decode(content)?;
            fs::write(&path, bytes)
                .map_err(|error| format!("failed to write {}: {}", path.display(), error))?;
        }
    }
    Ok(())
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

fn color_hex(color: Color) -> String {
    format!("{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        let triple = (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third);
        output.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        output.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(triple & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for character in text.chars().filter(|character| !character.is_whitespace()) {
        if character == '=' {
            break;
        }
        let value = base64_value(character)
            .ok_or_else(|| format!("invalid base64 character `{}`", character))?;
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    Ok(output)
}

fn base64_value(character: char) -> Option<u8> {
    match character {
        'A'..='Z' => Some(character as u8 - b'A'),
        'a'..='z' => Some(character as u8 - b'a' + 26),
        '0'..='9' => Some(character as u8 - b'0' + 52),
        '+' => Some(62),
        '/' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{GenerateGuiGfxAssetRequest, crc32, generate_gui_gfx_asset};
    use crate::tools::test_support::unique_test_dir;

    #[test]
    fn refuses_generation_without_approval() {
        let result = generate_gui_gfx_asset(GenerateGuiGfxAssetRequest {
            output_root: None,
            asset_name: "CHI_panel".to_string(),
            sprite_name: None,
            gui_name: None,
            width: 96,
            height: 48,
            style: Some("button".to_string()),
            primary_color: Some("#315f8c".to_string()),
            secondary_color: Some("#c8a75d".to_string()),
            texture: Some("noise".to_string()),
            shadow: Some(true),
            glow: Some(true),
            emboss: Some(true),
            write_gui: Some(true),
            approved: false,
            dry_run: true,
            relative_directory: None,
        })
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
            asset_name: "CHI_command_button".to_string(),
            sprite_name: Some("GFX_CHI_command_button".to_string()),
            gui_name: Some("CHI_command_button".to_string()),
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
            relative_directory: Some("gfx/interface/CHI".to_string()),
        })
        .expect("asset generation should complete");

        assert!(result.experimental);
        assert!(result.dry_run);
        assert_eq!(result.files.len(), 4);
        let png = result
            .files
            .iter()
            .find(|file| file.kind == "png")
            .expect("png should be returned");
        assert!(
            png.path
                .ends_with("gfx/interface/CHI/CHI_command_button.png")
        );
        assert!(
            png.content_base64
                .as_ref()
                .expect("png base64")
                .starts_with("iVBORw0KGgo")
        );
        assert!(result.files.iter().any(|file| {
            file.kind == "gfx"
                && file
                    .text_content
                    .as_deref()
                    .unwrap_or("")
                    .contains("GFX_CHI_command_button")
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
        let result = generate_gui_gfx_asset(GenerateGuiGfxAssetRequest {
            output_root: None,
            asset_name: "CHI_tiny".to_string(),
            sprite_name: None,
            gui_name: None,
            width: 1,
            height: 1,
            style: Some("button".to_string()),
            primary_color: Some("#214a67".to_string()),
            secondary_color: Some("#d5b261".to_string()),
            texture: Some("none".to_string()),
            shadow: Some(true),
            glow: Some(true),
            emboss: Some(true),
            write_gui: Some(false),
            approved: true,
            dry_run: true,
            relative_directory: None,
        })
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
            let error = generate_gui_gfx_asset(GenerateGuiGfxAssetRequest {
                output_root: None,
                asset_name: "CHI_bad".to_string(),
                sprite_name: None,
                gui_name: None,
                width: 64,
                height: 64,
                style: Some("button".to_string()),
                primary_color: None,
                secondary_color: None,
                texture: None,
                shadow: Some(false),
                glow: Some(false),
                emboss: Some(false),
                write_gui: Some(false),
                approved: true,
                dry_run: true,
                relative_directory: Some(directory.to_string()),
            })
            .expect_err("unsafe path should be rejected");

            assert!(error.contains("unsafe relative path"));
        }
    }

    #[test]
    fn uses_relative_directory_for_svg_source_path_and_png_crc() {
        let result = generate_gui_gfx_asset(GenerateGuiGfxAssetRequest {
            output_root: None,
            asset_name: "CHI_button".to_string(),
            sprite_name: None,
            gui_name: None,
            width: 32,
            height: 16,
            style: Some("button".to_string()),
            primary_color: None,
            secondary_color: None,
            texture: None,
            shadow: Some(false),
            glow: Some(false),
            emboss: Some(false),
            write_gui: Some(false),
            approved: true,
            dry_run: true,
            relative_directory: Some("gfx/interface/custom".to_string()),
        })
        .expect("asset should render");

        assert!(result.files.iter().any(|file| {
            file.kind == "svg" && file.path == "gfx/interface/custom/source/CHI_button.svg"
        }));
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn approved_apply_writes_game_files() {
        let root = unique_test_dir("gui-gfx-asset");
        let result = generate_gui_gfx_asset(GenerateGuiGfxAssetRequest {
            output_root: Some(root.to_string_lossy().to_string()),
            asset_name: "CHI_status_panel".to_string(),
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
            root.join("gfx/interface/rhoiscribe/CHI_status_panel.png")
                .is_file()
        );
        assert!(root.join("interface/CHI_status_panel.gfx").is_file());
        let bytes = fs::read(root.join("gfx/interface/rhoiscribe/CHI_status_panel.png"))
            .expect("png should read");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));

        fs::remove_dir_all(root).expect("temp output should clean up");
    }
}
