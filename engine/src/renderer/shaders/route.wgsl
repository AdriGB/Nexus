struct CameraUniform {
    viewport: vec4<f32>,
    world: vec4<f32>,
    hover_selected: vec4<f32>,
    options: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> @builtin(position) vec4<f32> {
    let screen_css = input.position * camera.viewport.w - camera.world.xy;
    let screen_pixels = screen_css * camera.viewport.z;
    let clip = vec2<f32>(
        screen_pixels.x / camera.viewport.x * 2.0 - 1.0,
        1.0 - screen_pixels.y / camera.viewport.y * 2.0,
    );
    return vec4<f32>(clip, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.72, 0.18, 0.95);
}
