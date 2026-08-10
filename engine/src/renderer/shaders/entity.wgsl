struct CameraUniform {
    viewport: vec4<f32>,
    world: vec4<f32>,
    hover_selected: vec4<f32>,
    options: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_position: vec2<f32>,
    @location(1) hunger: f32,
    @location(2) @interpolate(flat) activity: u32,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) entity_position: vec2<f32>,
    @location(1) hunger: f32,
    @location(2) activity: u32,
) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );
    let corner = corners[vertex_index];
    let tile_size = camera.viewport.w;
    let dpr = camera.viewport.z;
    let radius = max(tile_size * 0.38, 3.5);
    let screen_position = entity_position * tile_size - camera.world.xy + corner * radius;
    let physical_position = screen_position * dpr;
    let clip = vec2<f32>(
        physical_position.x / camera.viewport.x * 2.0 - 1.0,
        1.0 - physical_position.y / camera.viewport.y * 2.0,
    );

    var output: VertexOutput;
    output.clip_position = vec4<f32>(clip, 0.0, 1.0);
    output.local_position = corner;
    output.hunger = hunger;
    output.activity = activity;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let radius = length(input.local_position);
    if (radius > 1.0) {
        discard;
    }

    let hunger = clamp(input.hunger / 100.0, 0.0, 1.0);
    var color = mix(vec3<f32>(0.25, 0.82, 1.0), vec3<f32>(1.0, 0.28, 0.15), hunger);
    if (input.activity == 1u) {
        color = mix(color, vec3<f32>(1.0, 0.78, 0.18), 0.45);
    } else if (input.activity == 2u) {
        color = mix(color, vec3<f32>(0.4, 1.0, 0.55), 0.35);
    } else if (input.activity == 3u) {
        color = vec3<f32>(0.95, 0.08, 0.08);
    } else if (input.activity == 4u) {
        color = mix(color, vec3<f32>(0.72, 0.42, 1.0), 0.55);
    } else if (input.activity == 5u) {
        color = mix(color, vec3<f32>(0.45, 0.55, 0.72), 0.65);
    } else if (input.activity == 6u) {
        color = mix(color, vec3<f32>(1.0, 0.55, 0.85), 0.55);
    }
    let edge = smoothstep(1.0, 0.72, radius);
    return vec4<f32>(color * (0.72 + edge * 0.28), 0.98);
}
