// Outline shader: draws a texture where solid colours are outlined

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coord: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

struct ScreenUniform {
    mat0: vec4<f32>,
    mat1: vec4<f32>,
    mat2: vec4<f32>,
    mat3: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> screen_uniform: ScreenUniform;

@vertex
fn vs_main(vert: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let screen_matrix = mat4x4<f32>(
        screen_uniform.mat0,
        screen_uniform.mat1,
        screen_uniform.mat2,
        screen_uniform.mat3,
    );

    out.clip_position = screen_matrix * vec4<f32>(vert.position.xy, 0.0, 1.0);
    out.tex_coord = vert.tex_coord;
    return out;
}

@group(1) @binding(0)
var texture: texture_2d<f32>;
@group(1) @binding(1)
var texture_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // (modified) wgsl version of this shader https://godotshaders.com/shader/2d-outline-stroke/
    let thickness = 5.0;
    let size = thickness / vec2<f32>(f32(textureDimensions(texture).x), f32(textureDimensions(texture).y));
    let line_colour = vec4<f32>(0.0, 0.0, 0.0, 1.0);

    let sample_points = 16;

    var outline = 0.0;
    let angle_interval = 6.283 / f32(sample_points);

    for (var i = 0; i < sample_points; i++) {
        let angle = f32(i) * angle_interval;
        
        let offset = size * vec2<f32>(cos(angle), sin(angle));
        outline += textureSample(texture, texture_sampler, in.tex_coord + offset).a;
    }

    outline = min(2.0 * outline, 1.0);

    let sample = textureSample(texture, texture_sampler, in.tex_coord);

    return mix(sample, line_colour, outline - sample.a);
}
