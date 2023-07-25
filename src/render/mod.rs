use std::time::Instant;

use ab_glyph::FontArc;
use anyhow::anyhow;
use egui_wgpu::renderer::ScreenDescriptor;
use wgpu::{
    include_wgsl,
    util::{BufferInitDescriptor, DeviceExt},
};
use wgpu_text::{glyph_brush::Section, BrushBuilder};
use winit::{dpi::PhysicalSize, window::Window};

use crate::app::App;
use primitives::PrimitiveVertex;
use texture::TextureVertex;

use self::texture::SpriteInstance;

const SAMPLE_COUNT: u32 = 4;
const CLEAR_COLOUR: wgpu::Color = wgpu::Color::BLACK;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub mod context;
pub mod primitives;
pub mod text;
pub mod texture;

pub use context::RenderContext;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ScreenUniform {
    matrix: [[f32; 4]; 4],
}

struct Egui {
    platform: egui_winit_platform::Platform,
    renderer: egui_wgpu::Renderer,
    start_time: Instant,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub window: Window,
    size: PhysicalSize<u32>,
    surface: wgpu::Surface,
    config: wgpu::SurfaceConfiguration,
    msaa_view: Option<wgpu::TextureView>,
    depth_view: wgpu::TextureView,
    screen_uniform: wgpu::Buffer,
    screen_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    pipeline_cache: Vec<(&'static str, wgpu::RenderPipeline)>,

    text_brush: wgpu_text::TextBrush,
    egui_handler: Egui,
}

// A matrix that turns pixel coordinates into wgpu screen coordinates.
fn create_screen_uniform(size: &PhysicalSize<u32>) -> ScreenUniform {
    let width = size.width as f32;
    let height = size.height as f32;
    let sx = 2.0 / width;
    let sy = -2.0 / height;

    // Note that wgsl constructs matrices by *row*, not by column
    // which means this is the transpose of what it should be
    // i found that out the hard way
    ScreenUniform {
        matrix: [
            [sx, 0.0, 0.0, 0.0],
            [0.0, sy, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0, 1.0],
        ],
    }
}

// This is here just in case i need it again
// but for a 2d application, z sorting is better as it preserves transparency
// and every object has a constant fixed z value (flat)
//
// Creates a z buffer for depth-based pixel culling
fn create_depth_texture(device: &wgpu::Device, size: &PhysicalSize<u32>) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth texture"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

// Creates a texture to be used as a target for multisampling.
fn create_msaa_texture(
    device: &wgpu::Device,
    size: &PhysicalSize<u32>,
    config: &wgpu::SurfaceConfiguration,
    samples: u32,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("msaa texture"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            mip_level_count: 1,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn create_render_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    colour_format: wgpu::TextureFormat,
    use_depth: bool,
    vertex_layouts: &[wgpu::VertexBufferLayout],
    shader: &wgpu::ShaderModule,
    samples: u32,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "vs_main",
            buffers: vertex_layouts,
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: colour_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: use_depth,
            depth_compare: if use_depth {
                wgpu::CompareFunction::Less
            } else {
                wgpu::CompareFunction::Always
            },
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
        multiview: None,
    })
}

impl Egui {
    /// Creates a new egui handler.
    fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, scale_factor: f64) -> Self {
        let platform =
            egui_winit_platform::Platform::new(egui_winit_platform::PlatformDescriptor {
                physical_width: config.width,
                physical_height: config.height,
                scale_factor,
                ..Default::default()
            });

        let renderer =
            egui_wgpu::Renderer::new(device, config.format, Some(DEPTH_FORMAT), SAMPLE_COUNT);

        Self {
            platform,
            renderer,
            start_time: Instant::now(),
        }
    }

    /// Passes a winit event to egui for processing.
    ///
    /// Returns true if the event is "captured", which means it should not be handled by anything
    /// else (for example, clicking on an egui element should not also click behind it).
    fn handle_event<T>(&mut self, event: &winit::event::Event<'_, T>) -> bool {
        self.platform.handle_event(event);
        self.platform.captures_event(event)
    }

    fn begin_render(&mut self) {
        self.platform
            .update_time(self.start_time.elapsed().as_secs_f64());
        self.platform.begin_frame();
    }

    fn context(&self) -> egui::Context {
        self.platform.context()
    }

    fn end_render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        screen_descriptor: &ScreenDescriptor,
        window: &Window,
    ) -> Vec<egui::ClippedPrimitive> {
        let full_output = self.platform.end_frame(Some(window));
        let paint_jobs = self.platform.context().tessellate(full_output.shapes);
        let textures_delta = full_output.textures_delta;

        for texture in textures_delta.free.iter() {
            self.renderer.free_texture(texture);
        }

        for (id, image_delta) in textures_delta.set {
            self.renderer
                .update_texture(device, queue, id, &image_delta);
        }

        self.renderer
            .update_buffers(device, queue, encoder, &paint_jobs, screen_descriptor);

        paint_jobs
    }

    fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        paint_jobs: Vec<egui::ClippedPrimitive>,
        screen_descriptor: &ScreenDescriptor,
    ) {
        self.renderer
            .render(render_pass, &paint_jobs, screen_descriptor);
    }
}

impl Renderer {
    pub async fn new(window: Window) -> anyhow::Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let surface = unsafe { instance.create_surface(&window) }?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: Default::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or(anyhow!("Error requesting wgpu adapter."))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                /*trace_path: */ None,
            )
            .await?;

        let surface_capabilities = surface.get_capabilities(&adapter);

        let format = surface_capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_capabilities.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        let msaa_view = if SAMPLE_COUNT > 1 {
            Some(create_msaa_texture(&device, &size, &config, SAMPLE_COUNT))
        } else {
            None
        };

        let screen_uniform = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Screen uniform buffer"),
            contents: bytemuck::cast_slice(&[create_screen_uniform(&size)]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let screen_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Screen uniform bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let screen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Screen uniform bind group"),
            layout: &screen_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_uniform.as_entire_binding(),
            }],
        });

        let primitive_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Primitive pipeline layout"),
                bind_group_layouts: &[&screen_bind_group_layout],
                push_constant_ranges: &[],
            });

        let primitive_shader =
            device.create_shader_module(include_wgsl!("shaders/primitive_shader.wgsl"));

        let primitive_pipeline = create_render_pipeline(
            &device,
            "primitive pipeline",
            &primitive_pipeline_layout,
            config.format,
            false,
            &[PrimitiveVertex::desc()],
            &primitive_shader,
            SAMPLE_COUNT,
        );

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let texture_shader =
            device.create_shader_module(include_wgsl!("shaders/texture_shader.wgsl"));

        let texture_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("texture pipeline layout"),
                bind_group_layouts: &[&screen_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let texture_pipeline = create_render_pipeline(
            &device,
            "texture pipeline",
            &texture_pipeline_layout,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            true,
            &[
                TextureVertex::vertex_layout(),
                SpriteInstance::vertex_layout(),
            ],
            &texture_shader,
            SAMPLE_COUNT,
        );

        let depth_view = create_depth_texture(&device, &size);
        let egui_handler = Egui::new(&device, &config, window.scale_factor());

        let text_brush = BrushBuilder::using_font(FontArc::try_from_vec(std::fs::read(
            "assets/fonts/MochiyPopOne-Regular.ttf",
        )?)?)
        .with_depth_stencil(Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Always,
            stencil: Default::default(),
            bias: Default::default(),
        }))
        .with_multisample(wgpu::MultisampleState {
            count: SAMPLE_COUNT,
            ..Default::default()
        })
        .build(&device, config.width, config.height, format);

        Ok(Self {
            size,
            surface,
            config,
            device,
            queue,
            window,
            msaa_view,
            depth_view,
            screen_uniform,
            screen_bind_group,
            texture_bind_group_layout,
            pipeline_cache: vec![
                ("texture", texture_pipeline),
                ("primitive", primitive_pipeline),
            ],
            text_brush,
            egui_handler,
        })
    }

    pub fn render(&mut self, app: &mut App) -> Result<(), wgpu::SurfaceError> {
        let texture = self.surface.get_current_texture()?;
        let view = texture.texture.create_view(&Default::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render pass encoder"),
            });

        self.egui_handler.begin_render();

        app.debug_ui(self.egui_handler.context());

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: self.window.scale_factor() as _,
        };

        let paint_jobs = self.egui_handler.end_render(
            &self.device,
            &self.queue,
            &mut encoder,
            &screen_descriptor,
            &self.window,
        );

        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: if SAMPLE_COUNT == 1 {
                    &view
                } else {
                    self.msaa_view.as_ref().unwrap()
                },
                resolve_target: if SAMPLE_COUNT == 1 { None } else { Some(&view) },
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CLEAR_COLOUR),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        self.text_brush
            .queue(&self.device, &self.queue, Vec::<Section>::new())
            .unwrap();

        let mut ctx = RenderContext {
            render_pass,
            device: &self.device,
            queue: &self.queue,
            pipeline_cache: &self.pipeline_cache,
            text_brush: Some(&mut self.text_brush),
        };

        ctx.render_pass
            .set_bind_group(0, &self.screen_bind_group, &[]);

        // Rendering goes here...
        app.render(&mut ctx);

        ctx.text_brush.take().unwrap().draw(&mut ctx.render_pass);

        // Last step will be to render the debug gui
        self.egui_handler
            .render(&mut ctx.render_pass, paint_jobs, &screen_descriptor);

        drop(ctx);

        self.queue.submit([encoder.finish()]);
        texture.present();

        Ok(())
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.size = size;
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);

            // Need to create a new msaa target texture
            if SAMPLE_COUNT > 1 {
                self.msaa_view = Some(create_msaa_texture(
                    &self.device,
                    &size,
                    &self.config,
                    SAMPLE_COUNT,
                ));
            }

            // Resize the screen space transformation matrirx
            let screen_uniform = create_screen_uniform(&size);
            self.queue.write_buffer(
                &self.screen_uniform,
                0,
                bytemuck::cast_slice(&[screen_uniform]),
            )
        }
    }

    /// Handles a [winit] window event.
    ///
    /// Returns a bool indicating whether the event was 'captured' by the renderer.
    /// That is, if this returns true, the event should not be processed further.
    pub fn handle_event<T>(&mut self, event: &winit::event::Event<'_, T>) -> bool {
        self.egui_handler.handle_event(event)
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn size(&self) -> &PhysicalSize<u32> {
        &self.size
    }

    pub fn create_texture_bind_group(
        &self,
        label: Option<&str>,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}
