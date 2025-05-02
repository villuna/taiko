use std::{num::NonZeroU64, sync::OnceLock};

use wgpu::util::DeviceExt;

const HEALTH_BAR_LENGTH: f32 = 290.;
const HEALTH_BAR_PADDING: f32 = 5.;

use crate::{
    include_shader,
    render::{
        create_render_pipeline,
        shapes::{Shape, ShapeBuilder, ShapeVertex, SolidColour},
        texture::SpriteInstance,
        Renderable, Renderer, DEPTH_FORMAT, SAMPLE_COUNT,
    },
};

#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct HealthBarUniform {
    empty_colour: [f32; 4],
    full_colour: [f32; 4],
    fill_amount: f32,
    length: f32,
    _padding: [f32; 2],
}

impl HealthBarUniform {
    fn new(fill_amount: f32) -> Self {
        Self {
            // TODO: better colours
            empty_colour: [0., 0., 0., 1.],
            full_colour: [1.; 4],
            fill_amount,
            length: HEALTH_BAR_LENGTH,
            _padding: [0.; 2],
        }
    }
}

static HEALTH_BAR_UNIFORM_BGL: OnceLock<wgpu::BindGroupLayout> = OnceLock::new();

fn health_bar_bgl(device: &wgpu::Device) -> &'static wgpu::BindGroupLayout {
    HEALTH_BAR_UNIFORM_BGL.get_or_init(|| {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("health bar uniform bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(std::mem::size_of::<HealthBarUniform>() as _),
                },
                count: None,
            }],
        })
    })
}

/// The health bar/soul gague. A horizontal rounded rectangle with a bar inside that fills up from the right
/// to the left.
pub struct HealthBar {
    background: Shape,
    bar: Shape,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl HealthBar {
    pub fn new(renderer: &Renderer) -> anyhow::Result<Self> {
        let background = ShapeBuilder::new()
            .position([100., 100., 0.])
            .filled_roundrect(
                [0., 0.],
                [HEALTH_BAR_LENGTH + HEALTH_BAR_PADDING * 2., 30.],
                15.,
                SolidColour::new([0.2, 0.2, 0.2, 1.]),
            )?
            .build(&renderer.device);

        let bar = ShapeBuilder::new()
            .position([105., 105., 0.])
            .filled_roundrect(
                [0., 0.],
                [HEALTH_BAR_LENGTH, 20.],
                10.,
                SolidColour::new([1.; 4]),
            )?
            .with_pipeline("health bar")
            .build(&renderer.device);

        let buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("health bar uniform buffer"),
                contents: bytemuck::cast_slice(&[HealthBarUniform::new(0.5)]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group = renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("health bar uniform bind group"),
                layout: health_bar_bgl(&renderer.device),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });

        Ok(Self {
            background,
            bar,
            uniform: buffer,
            bind_group,
        })
    }

    pub fn set_fill_amount(&self, amount: f32, renderer: &Renderer) {
        renderer.queue.write_buffer(
            &self.uniform,
            0 as _,
            bytemuck::cast_slice(&[HealthBarUniform::new(amount.clamp(0., 1.))]),
        );
    }
}

impl Renderable for HealthBar {
    fn render<'pass>(
        &'pass self,
        renderer: &'pass crate::render::Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        self.background.render(renderer, render_pass);
        render_pass.set_bind_group(1, &self.bind_group, &[]);
        self.bar.render(renderer, render_pass);
    }
}

pub fn create_health_bar_pipeline(
    device: &wgpu::Device,
    screen_bgl: &wgpu::BindGroupLayout,
    config: &wgpu::SurfaceConfiguration,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(include_shader!("shaders/health_bar_shader.wgsl"));

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("health bar pipeline layout"),
        bind_group_layouts: &[&screen_bgl, health_bar_bgl(device)],
        push_constant_ranges: &[],
    });

    create_render_pipeline(
        device,
        "health bar pipeline",
        &pipeline_layout,
        config.format,
        Some(DEPTH_FORMAT),
        false,
        &[
            ShapeVertex::vertex_layout(),
            SpriteInstance::vertex_layout(),
        ],
        &shader,
        SAMPLE_COUNT,
    )
}
