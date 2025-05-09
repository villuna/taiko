// Health bar shader - shader for the health bar ui element

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) colour: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // The fill amount that this vertex corresponds to
    // so if a pixel is directly in the middle of the bar, this will be 0.5
    @location(0) fill_amount: f32,
    @location(1) height: f32,
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

struct HealthBarUniform {
    length: f32,
    fill: f32,
    target_fill: f32,
    time: f32,
};

@group(0) @binding(0)
var<uniform> screen_uniform: ScreenUniform;

@group(1) @binding(0)
var<uniform> health_bar_uniform: HealthBarUniform;


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

    out.fill_amount = in.position.x / health_bar_uniform.length;
    out.height = in.position.y / 30.;

    return out;
}

fn add_colours(fg: vec4<f32>, bg: vec4<f32>) -> vec4<f32> {
    var res: vec4<f32>;
    // from https://stackoverflow.com/a/727339
    res.a = 1.0 - (1.0 - fg.a) * (1.0 - bg.a);
    res.r = fg.r * fg.a / res.a + bg.r * bg.a * (1 - fg.a) / res.a;
    res.g = fg.g * fg.a / res.a + bg.g * bg.a * (1 - fg.a) / res.a;
    res.b = fg.b * fg.a / res.a + bg.b * bg.a * (1 - fg.a) / res.a;
    return res;
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> vec3<f32> {
    let c = (1.0 - abs(2.0 * l - 1.0)) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - abs(hp % 2.0 - 1.0));

    var rgb: vec3<f32>;
    if 0.0 <= hp && hp < 1.0 {
        rgb = vec3<f32>(c, x, 0.0);
    } else if 1.0 <= hp && hp < 2.0 {
        rgb = vec3<f32>(x, c, 0.0);
    } else if 2.0 <= hp && hp < 3.0 {
        rgb = vec3<f32>(0.0, c, x);
    } else if 3.0 <= hp && hp < 4.0 {
        rgb = vec3<f32>(0.0, x, c);
    } else if 4.0 <= hp && hp < 5.0 {
        rgb = vec3<f32>(x, 0.0, c);
    } else {
        rgb = vec3<f32>(c, 0.0, x);
    }

    let m = l - c / 2.0;
    return rgb + m;
}

fn rainbow(fill_amount: f32, height: f32, time: f32) -> vec4<f32> {
    let height_scaled = 0.03 * height;
    let spread = 2.0 * 360.0;
    let time_scale = 0.15;
    let t = fract(time * time_scale);
    let h = (fill_amount + height_scaled + 1.0 - t) * spread % 360.0;
    let quantizing = 25.0;

    let h2 = h - h % quantizing;
    return vec4<f32>(hsl_to_rgb(h2, 1.0, 0.6), 1.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // TODO srgb
    // TODO also other colour effects such as rainbow for full
    var colour: vec4<f32>;
    let empty_colour = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    let fill_colour = vec4<f32>(1.0, 1.0, 1.0, 1.0);

    // Currently this doesn't do anything but it will do when we change the empty colour
    // and make the green and red overlay more transparent
    let green = add_colours(vec4<f32>(0.0, 1.0, 0.0, 1.0), empty_colour);
    let red = add_colours(vec4<f32>(1.0, 0.0, 0.0, 1.0), empty_colour);

    if health_bar_uniform.target_fill == 1.0 {
        return rainbow(in.fill_amount, in.height, health_bar_uniform.time);
    }

    if in.fill_amount > health_bar_uniform.fill {
        if in.fill_amount < health_bar_uniform.target_fill {
            colour = green;
        } else {
            colour = empty_colour;
        }
    } else {
        if in.fill_amount > health_bar_uniform.target_fill {
            colour = red;
        } else {
            colour = fill_colour;
        }
    }

    return colour;
}
