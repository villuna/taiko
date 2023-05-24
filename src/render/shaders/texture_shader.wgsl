// Texture shader: Draws 2d textures

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coord: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

struct Instance {
    @location(2) world_position: vec3<f32>,
};

struct ScreenUniform {
    mat0: vec4<f32>,
    mat1: vec4<f32>,
    mat2: vec4<f32>,
    mat3: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> screen_uniform: ScreenUniform;

fn quick_sigmoid(z: f32) -> f32 {
    return 0.5 * ((z / (1.0 + abs(z))) + 1.0);
}

@vertex
fn vs_main(vert: VertexInput, inst: Instance) -> VertexOutput {
    var out: VertexOutput;

    let screen_matrix = mat4x4<f32>(
        screen_uniform.mat0,
        screen_uniform.mat1,
        screen_uniform.mat2,
        screen_uniform.mat3,
    );

    out.clip_position = screen_matrix * vec4<f32>(vert.position.xy + inst.world_position.xy, inst.world_position.z, 1.0);
    out.clip_position.z = quick_sigmoid(out.clip_position.z);
    out.tex_coord = vert.tex_coord;
    return out;
}

@group(1) @binding(0)
var texture: texture_2d<f32>;
@group(1) @binding(1)
var texture_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(texture, texture_sampler, in.tex_coord);
}
