use std::{num::NonZeroU64, sync::OnceLock};

use wgpu::util::DeviceExt;

const HEALTH_BAR_LENGTH: f32 = 590.;
const HEALTH_BAR_PADDING: f32 = 5.;
const HEALTH_BAR_HEIGHT: f32 = 30.;

use crate::{
    include_shader,
    render::{
        create_render_pipeline,
        shapes::{Shape, ShapeBuilder, ShapeVertex, SolidColour},
        texture::SpriteInstance,
        Renderable, Renderer, DEPTH_FORMAT, SAMPLE_COUNT,
    },
};

/// Data used by the custom health bar shader.
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct HealthBarUniform {
    fill_amount: f32,
    length: f32,
    _padding: [f32; 2],
}

impl HealthBarUniform {
    fn new(fill_amount: f32) -> Self {
        Self {
            fill_amount,
            length: HEALTH_BAR_LENGTH,
            _padding: [0.; 2],
        }
    }
}

static HEALTH_BAR_UNIFORM_BGL: OnceLock<wgpu::BindGroupLayout> = OnceLock::new();

/// Gets the bind group layout for the [HealthBarUniform]. This is static and only created the first
/// time the function is called.
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
    /// Creates a new visual health bar. Will display as empty by default.
    pub fn new(renderer: &Renderer) -> anyhow::Result<Self> {
        let background = ShapeBuilder::new()
            .position([100., 100., 0.])
            .filled_roundrect(
                [0., 0.],
                [
                    HEALTH_BAR_LENGTH + HEALTH_BAR_PADDING * 2.,
                    HEALTH_BAR_HEIGHT + HEALTH_BAR_PADDING * 2.,
                ],
                HEALTH_BAR_HEIGHT / 2. + HEALTH_BAR_PADDING,
                SolidColour::new([0.2, 0.2, 0.2, 1.]),
            )?
            .build(&renderer.device);

        let bar = ShapeBuilder::new()
            .position([105., 105., 0.])
            .filled_roundrect(
                [0., 0.],
                [HEALTH_BAR_LENGTH, HEALTH_BAR_HEIGHT],
                HEALTH_BAR_HEIGHT / 2.,
                SolidColour::new([1.; 4]),
            )?
            .with_pipeline("health bar")
            .build(&renderer.device);

        let buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("health bar uniform buffer"),
                contents: bytemuck::cast_slice(&[HealthBarUniform::new(0.)]),
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

    /// Sets the health bar to display a certain fill amount, from 0-1 where 0 is empty and 1 is
    /// full. Values outside of this range will be clamped to fit.
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

/// Creates a custom pipeline for drawing the health bar.
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
