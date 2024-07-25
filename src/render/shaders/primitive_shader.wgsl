// Primitive filled shader: Basic coloured triangles

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) colour: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) colour: vec4<f32>,
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
fn vs_main(in: VertexInput, instance: Instance) -> VertexOutput {
    var out: VertexOutput;

    let screen_matrix = mat4x4<f32>(
        screen_uniform.mat0,
        screen_uniform.mat1,
        screen_uniform.mat2,
        screen_uniform.mat3,
    );

    out.clip_position = screen_matrix * vec4<f32>(in.position + instance.world_position, 1.0);
    out.clip_position.z = quick_sigmoid(out.clip_position.z);
    // For non-srgb:
    out.colour = in.colour;
    // // For srgb:
    // out.colour = vec4<f32>(pow(in.colour.xyz, vec3<f32>(2.2)), in.colour.w);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.colour;
}
