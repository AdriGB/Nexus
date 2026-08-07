struct CameraUniform {
    viewport: vec4<f32>,
    world: vec4<f32>,
    hover_selected: vec4<f32>,
    options: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var world_texture: texture_2d<u32>;

@group(0) @binding(2)
var resource_texture: texture_2d<u32>;

const TERRAIN_H = array<f32, 13>(
    215.0, 205.0, 42.0, 85.0, 95.0, 110.0, 120.0,
    50.0, 30.0, 210.0, 35.0, 78.0, 180.0,
);
const TERRAIN_S = array<f32, 13>(
    55.0, 45.0, 50.0, 35.0, 42.0, 45.0, 50.0,
    22.0, 8.0, 12.0, 58.0, 30.0, 10.0,
);
const TERRAIN_L = array<f32, 13>(
    14.0, 24.0, 58.0, 38.0, 32.0, 24.0, 16.0,
    38.0, 44.0, 88.0, 52.0, 22.0, 56.0,
);

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[vertex_index], 0.0, 1.0);
}

fn hue_to_rgb(p: f32, q: f32, source_t: f32) -> f32 {
    var t = source_t;
    if (t < 0.0) { t += 1.0; }
    if (t > 1.0) { t -= 1.0; }
    if (t < 1.0 / 6.0) { return p + (q - p) * 6.0 * t; }
    if (t < 0.5) { return q; }
    if (t < 2.0 / 3.0) { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
    return p;
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> vec3<f32> {
    let h = hue / 360.0;
    let s = saturation / 100.0;
    let l = lightness / 100.0;
    if (s == 0.0) {
        return vec3<f32>(l);
    }
    let q = select(l + s - l * s, l * (1.0 + s), l < 0.5);
    let p = 2.0 * l - q;
    return vec3<f32>(
        hue_to_rgb(p, q, h + 1.0 / 3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0 / 3.0),
    );
}

fn tile_jitter(tile: vec2<i32>) -> f32 {
    var hash = tile.x * 374761393 + tile.y * 668265263;
    hash = (hash ^ (hash >> 13u)) * 1274126177;
    return f32((hash ^ (hash >> 16u)) & 255) / 255.0;
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let dpr = camera.viewport.z;
    let tile_size = camera.viewport.w;
    let screen_position = position.xy / dpr;
    let world_position = (screen_position + camera.world.xy) / tile_size;
    let tile = vec2<i32>(floor(world_position));
    let world_size = vec2<i32>(camera.world.zw);

    if (any(tile < vec2<i32>(0)) || any(tile >= world_size)) {
        return vec4<f32>(7.0 / 255.0, 8.0 / 255.0, 12.0 / 255.0, 1.0);
    }

    let encoded = textureLoad(world_texture, tile, 0);
    let terrain = encoded.r;
    if (terrain >= 13u) {
        return vec4<f32>(7.0 / 255.0, 8.0 / 255.0, 12.0 / 255.0, 1.0);
    }

    let altitude = f32(encoded.g) / 255.0;
    let altitude_factor = 0.72 + altitude * 0.5;
    let jitter_range = select(5.0, 3.0, terrain <= 1u);
    var lightness = TERRAIN_L[terrain] * altitude_factor;
    lightness += (tile_jitter(tile) - 0.5) * jitter_range;
    lightness = clamp(lightness, 0.0, 100.0);
    var color = hsl_to_rgb(TERRAIN_H[terrain], TERRAIN_S[terrain], lightness);

    if (camera.options.z > 0.5) {
        let resource = textureLoad(resource_texture, tile, 0);
        let kind = resource.r;
        let amount = resource.g | (resource.b << 8u);
        let abundance = clamp(f32(amount) / 900.0, 0.0, 1.0);
        color = mix(vec3<f32>(0.035, 0.04, 0.05), color, 0.18);

        if (kind > 0u) {
            var resource_color = vec3<f32>(0.3, 0.9, 0.35);
            if (kind == 2u) {
                resource_color = vec3<f32>(0.12, 0.72, 0.42);
            } else if (kind == 3u) {
                resource_color = vec3<f32>(0.68, 0.72, 0.78);
            } else if (kind == 4u) {
                resource_color = vec3<f32>(0.92, 0.42, 0.18);
            }
            color = mix(color, resource_color, 0.55 + abundance * 0.4);
        }
    }

    let local = fract(world_position);
    let edge_distance = min(min(local.x, 1.0 - local.x), min(local.y, 1.0 - local.y)) * tile_size;

    if (camera.options.x > 0.5 && camera.options.y > 2.5 && edge_distance < 0.5) {
        color = mix(color, vec3<f32>(1.0), 0.06);
    }

    let hover = vec2<i32>(camera.hover_selected.xy);
    let selected = vec2<i32>(camera.hover_selected.zw);
    if (all(tile == selected) && edge_distance < 2.0) {
        color = mix(color, vec3<f32>(201.0 / 255.0, 168.0 / 255.0, 76.0 / 255.0), 0.9);
    } else if (all(tile == hover) && edge_distance < 2.0) {
        color = mix(color, vec3<f32>(201.0 / 255.0, 168.0 / 255.0, 76.0 / 255.0), 0.6);
    }

    return vec4<f32>(color, 1.0);
}
