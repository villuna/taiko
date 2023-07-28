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
    let thickness = 4.0;
    let size = thickness / vec2<f32>(f32(textureDimensions(texture).x), f32(textureDimensions(texture).y));
    let line_colour = vec4<f32>(0.0, 0.0, 0.0, 1.0);

    // wgsl version of this shader https://godotshaders.com/shader/2d-outline-stroke/
    var outline = textureSample(texture, texture_sampler, in.tex_coord + vec2<f32>(-size.x, 0.0)).a;
    outline += textureSample(texture, texture_sampler, in.tex_coord + vec2<f32>(size.x, 0.0)).a;
    outline += textureSample(texture, texture_sampler, in.tex_coord + vec2<f32>(0.0, -size.y)).a;
    outline += textureSample(texture, texture_sampler, in.tex_coord + vec2<f32>(0.0, size.y)).a;
    outline += textureSample(texture, texture_sampler, in.tex_coord + vec2<f32>(size.x, size.y)).a;
    outline += textureSample(texture, texture_sampler, in.tex_coord + vec2<f32>(-size.x, size.y)).a;
    outline += textureSample(texture, texture_sampler, in.tex_coord + vec2<f32>(size.x, -size.y)).a;
    outline += textureSample(texture, texture_sampler, in.tex_coord + vec2<f32>(-size.x, -size.y)).a;
    outline = min(outline, 1.0);

    let sample = textureSample(texture, texture_sampler, in.tex_coord);

    return mix(sample, line_colour, outline - sample.a);
}
