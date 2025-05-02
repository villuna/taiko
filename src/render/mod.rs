use anyhow::anyhow;
use egui_wgpu::ScreenDescriptor;
use kaku::{ab_glyph::FontVec, FontId, FontSize, SdfSettings, TextRendererBuilder};

use wgpu::util::{BufferInitDescriptor, DeviceExt};
use winit::{dpi::PhysicalSize, window::Window};

use crate::game::{create_health_bar_pipeline, Game};
use shapes::create_primitive_pipelines;
use texture::create_texture_pipelines;

use self::texture::SpriteInstance;

macro_rules! rgba {
    ($r:expr, $g:expr, $b:expr, $a:expr) => {
        [
            { $r } as f32 / 255.,
            { $g } as f32 / 255.,
            { $b } as f32 / 255.,
            { $a } as f32 / 255.,
        ]
    };
}

macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        [
            { $r } as f32 / 255.,
            { $g } as f32 / 255.,
            { $b } as f32 / 255.,
            1.,
        ]
    };
}

pub(crate) use rgb;
pub(crate) use rgba;

pub const SAMPLE_COUNT: u32 = 4;
const CLEAR_COLOUR: wgpu::Color = wgpu::Color::BLACK;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

mod egui;
pub mod shapes;
pub mod text;
pub mod texture;

/// A trait that allows objects to render themselves to the screen in any given render pass. If a
/// type implements Renderable, then it is able to be rendered by the [RenderPassContext]'s render
/// function.
pub trait Renderable {
    fn render<'pass>(
        &'pass self,
        renderer: &'pass Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
    );
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ScreenUniform {
    matrix: [[f32; 4]; 4],
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub window: &'static Window,
    size: PhysicalSize<u32>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    msaa_view: Option<wgpu::TextureView>,
    depth_view: wgpu::TextureView,
    screen_uniform: wgpu::Buffer,
    screen_bind_group: wgpu::BindGroup,
    pipeline_cache: Vec<(&'static str, wgpu::RenderPipeline)>,
    font_cache: Vec<(&'static str, FontId)>,

    pub text_renderer: kaku::TextRenderer,
    egui_handler: egui::Egui,
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
    size: (u32, u32),
    format: wgpu::TextureFormat,
    samples: u32,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("msaa texture"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            mip_level_count: 1,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

pub fn create_render_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    colour_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
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
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: colour_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
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
        depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: use_depth,
            depth_compare: if use_depth {
                wgpu::CompareFunction::LessEqual
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

// An extension of the include_wgsl macro that only includes the shaders at compile time if
// building for release version
#[macro_export]
macro_rules! include_shader {
    ($($token:tt)*) => {{
        #[cfg(debug_assertions)]
        {
            let path = { $($token)* };
            let full_path = format!("{}/src/render/{}", env!("CARGO_MANIFEST_DIR"), path);

            ::wgpu::ShaderModuleDescriptor {
                label: Some(path),
                source: ::wgpu::ShaderSource::Wgsl(::std::fs::read_to_string(full_path).unwrap().into())
            }
        }

        #[cfg(not(debug_assertions))]
        {
            ::wgpu::include_wgsl!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/", $($token)*))
        }
    }}
}

impl Renderer {
    pub fn new(window: &'static Window) -> anyhow::Result<Self> {
        pollster::block_on(Self::new_async(window))
    }

    async fn new_async(window: &'static Window) -> anyhow::Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window)?;

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
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                /*trace_path: */ None,
            )
            .await?;

        let surface_capabilities = surface.get_capabilities(&adapter);

        let format = surface_capabilities
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(surface_capabilities.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        let msaa_view = if SAMPLE_COUNT > 1 {
            Some(create_msaa_texture(
                &device,
                (size.width, size.height),
                config.format,
                SAMPLE_COUNT,
            ))
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

        let (primitive_pipeline, primitive_pipeline_depth) =
            create_primitive_pipelines(&device, &screen_bind_group_layout, &config);

        let (texture_pipeline, texture_pipeline_depth) =
            create_texture_pipelines(&device, &screen_bind_group_layout, &config);

        let health_bar_pipeline =
            create_health_bar_pipeline(&device, &screen_bind_group_layout, &config);

        let depth_view = create_depth_texture(&device, &size);
        let egui_handler = egui::Egui::new(&device, &config, window.scale_factor());

        let mut font_cache = Vec::new();
        let mut text_renderer =
            TextRendererBuilder::new(config.format, (config.width, config.height))
                .with_msaa_sample_count(SAMPLE_COUNT)
                .with_depth(DEPTH_FORMAT)
                .build(&device);

        for (font, filename, size) in [
            ("mplus bold", "MPLUSRounded1c-Bold.ttf", 50.),
            ("mplus regular", "MPLUSRounded1c-Regular.ttf", 50.),
            ("mochiy pop one", "MochiyPopOne-Regular.ttf", 80.),
        ] {
            let font_data =
                FontVec::try_from_vec(std::fs::read(format!("assets/fonts/{filename}"))?)?;
            let id = text_renderer.load_font_with_sdf(
                font_data,
                FontSize::Px(size),
                SdfSettings { radius: 20. },
            );
            font_cache.push((font.to_string().leak() as &'static str, id));
            text_renderer.generate_char_textures('0'..'9', id, &device, &queue);
        }

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
            pipeline_cache: vec![
                ("texture", texture_pipeline),
                ("texture depth", texture_pipeline_depth),
                ("primitive", primitive_pipeline),
                ("primitive depth", primitive_pipeline_depth),
                ("health bar", health_bar_pipeline),
            ],
            font_cache,
            text_renderer,
            egui_handler,
        })
    }

    pub fn render(&mut self, app: &mut Game) -> Result<(), wgpu::SurfaceError> {
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

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Rendering goes here...
        app.render(self, &mut render_pass);

        self.egui_handler
            .render(&mut render_pass, &paint_jobs, &screen_descriptor);

        drop(render_pass);

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

            self.depth_view = create_depth_texture(&self.device, &size);

            if SAMPLE_COUNT > 1 {
                self.msaa_view = Some(create_msaa_texture(
                    &self.device,
                    (size.width, size.height),
                    self.config.format,
                    SAMPLE_COUNT,
                ));
            }

            // Resize the screen space transformation matrirx
            let screen_uniform = create_screen_uniform(&size);
            self.queue.write_buffer(
                &self.screen_uniform,
                0,
                bytemuck::cast_slice(&[screen_uniform]),
            );

            self.text_renderer
                .resize((size.width, size.height), &self.queue);
        }
    }

    /// Handles a [winit] window event.
    ///
    /// Returns a bool indicating whether the event was 'captured' by the renderer.
    /// That is, if this returns true, the event should not be processed further.
    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        self.egui_handler.handle_event(event)
    }

    pub fn size(&self) -> &PhysicalSize<u32> {
        &self.size
    }

    pub fn pipeline(&self, name: &str) -> Option<&wgpu::RenderPipeline> {
        self.pipeline_cache.iter().find_map(
            |(n, pipeline)| {
                if name == *n {
                    Some(pipeline)
                } else {
                    None
                }
            },
        )
    }

    pub fn font(&self, name: &str) -> FontId {
        self.font_cache
            .iter()
            .find(|(n, _)| *n == name)
            .expect("Font does not exist")
            .1
    }
}
