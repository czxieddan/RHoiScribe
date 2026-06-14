use super::Color;

pub(super) struct RenderOptions<'a> {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) primary: Color,
    pub(super) secondary: Color,
    pub(super) style: &'a str,
    pub(super) texture: &'a str,
    pub(super) shadow: bool,
    pub(super) glow: bool,
    pub(super) emboss: bool,
}

pub(super) fn render_asset(options: RenderOptions<'_>) -> Vec<u8> {
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
    if options.shadow && is_near_shadow(&options, shifted_x, shifted_y) {
        return Color {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 75,
        };
    }
    if options.glow && is_near_glow(&options) {
        return Color {
            red: options.secondary.red,
            green: options.secondary.green,
            blue: options.secondary.blue,
            alpha: 55,
        };
    }
    TRANSPARENT
}

fn is_near_shadow(options: &OuterEffectOptions, shifted_x: f32, shifted_y: f32) -> bool {
    rounded_rect_contains(
        shifted_x,
        shifted_y,
        options.width,
        options.height,
        options.radius + 2.0,
    )
}

fn is_near_glow(options: &OuterEffectOptions) -> bool {
    rounded_rect_contains(
        options.x,
        options.y,
        options.width,
        options.height,
        options.radius + 4.0,
    )
}

fn apply_texture(color: &mut Color, texture: &str, x: u32, y: u32) {
    let noise = pseudo_noise(x, y) as i16 - 128;
    let delta = match texture {
        "grid" if x.is_multiple_of(8) || y.is_multiple_of(8) => 18,
        "brushed" => ((x as i16 % 9) - 4) * 3,
        "none" => 0,
        _ => noise / 12,
    };
    adjust_rgb(color, delta);
}

fn apply_emboss(color: &mut Color, fx: f32, fy: f32) {
    let delta = if fx + fy < 0.45 {
        24
    } else if fx + fy > 1.55 {
        -24
    } else {
        0
    };
    adjust_rgb(color, delta);
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
        adjust_rgb(color, -38);
    }
}

fn adjust_rgb(color: &mut Color, delta: i16) {
    color.red = add_channel(color.red, delta);
    color.green = add_channel(color.green, delta);
    color.blue = add_channel(color.blue, delta);
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

pub(super) fn encode_png_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    if rgba.len() != (width as usize * height as usize * 4) {
        return Err("rgba buffer length does not match dimensions".to_string());
    }

    let raw = png_scanlines(width, height, rgba);
    let mut png = png_signature();
    write_png_chunk(&mut png, b"IHDR", &png_ihdr(width, height));
    write_png_chunk(&mut png, b"IDAT", &zlib_store(&raw));
    write_png_chunk(&mut png, b"IEND", &[]);
    Ok(png)
}

fn png_scanlines(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let stride = width as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for row in rgba.chunks(stride) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    raw
}

fn png_signature() -> Vec<u8> {
    Vec::from([137, 80, 78, 71, 13, 10, 26, 10])
}

fn png_ihdr(width: u32, height: u32) -> Vec<u8> {
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    ihdr
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut output = vec![0x78, 0x01];
    let mut offset = 0usize;
    while offset < data.len() {
        offset += write_zlib_block(&mut output, data, offset);
    }
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

fn write_zlib_block(output: &mut Vec<u8>, data: &[u8], offset: usize) -> usize {
    let remaining = data.len() - offset;
    let block_len = remaining.min(65_535);
    let final_block = offset + block_len >= data.len();
    output.push(if final_block { 0x01 } else { 0x00 });
    let len = block_len as u16;
    output.extend_from_slice(&len.to_le_bytes());
    output.extend_from_slice(&(!len).to_le_bytes());
    output.extend_from_slice(&data[offset..offset + block_len]);
    block_len
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

pub(super) fn crc32(data: &[u8]) -> u32 {
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

pub(super) fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        append_base64_chunk(&mut output, TABLE, chunk);
    }
    output
}

fn append_base64_chunk(output: &mut String, table: &[u8; 64], chunk: &[u8]) {
    let first = chunk[0];
    let second = *chunk.get(1).unwrap_or(&0);
    let third = *chunk.get(2).unwrap_or(&0);
    let triple = (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third);
    output.push(table[((triple >> 18) & 0x3f) as usize] as char);
    output.push(table[((triple >> 12) & 0x3f) as usize] as char);
    output.push(if chunk.len() > 1 {
        table[((triple >> 6) & 0x3f) as usize] as char
    } else {
        '='
    });
    output.push(if chunk.len() > 2 {
        table[(triple & 0x3f) as usize] as char
    } else {
        '='
    });
}

pub(super) fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
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
