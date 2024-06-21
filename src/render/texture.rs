//! Various types used for drawing textures

use anyhow::anyhow;
use image::GenericImageView;
use std::time::Instant;
use std::{path::Path, rc::Rc, sync::OnceLock};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    vertex_attr_array, RenderPass,
};

use super::{Renderable, Renderer};

static TEXTURE_BIND_GROUP_LAYOUT: OnceLock<wgpu::BindGroupLayout> = OnceLock::new();

/// A vertex of a sprite drawn to the screen
///
/// This is for use in rendering, in particular see the (texture
/// shader)[shaders/texture_shader.wgsl].
///
/// Each vertex contains a position in 2d space, and a coordinate
/// in texture space (where the vertex maps to on the texture).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextureVertex {
    pub position: [f32; 2],
    pub tex_coord: [f32; 2],
}

fn texture_vertices(width: u32, height: u32) -> [TextureVertex; 4] {
    [
        TextureVertex {
            position: [0.0, 0.0],
            tex_coord: [0.0, 0.0],
        },
        TextureVertex {
            position: [0.0, height as f32],
            tex_coord: [0.0, 1.0],
        },
        TextureVertex {
            position: [width as f32, 0.0],
            tex_coord: [1.0, 0.0],
        },
        TextureVertex {
            position: [width as f32, height as f32],
            tex_coord: [1.0, 1.0],
        },
    ]
}

// TODO: Make a single static index buffer so I don't have to have a bunch of copies of this on the GPU
const TEXTURE_INDICES: [u16; 6] = [0, 1, 2, 1, 3, 2];

impl TextureVertex {
    const ATTRS: &'static [wgpu::VertexAttribute] =
        &vertex_attr_array![0 => Float32x2, 1 => Float32x2];

    /// Returns the vertex buffer layout describing this vertex
    pub fn vertex_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextureVertex>() as _,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: Self::ATTRS,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct SpriteInstance {
    pub position: [f32; 3],
}

impl SpriteInstance {
    const ATTRS: &'static [wgpu::VertexAttribute] = &vertex_attr_array![2 => Float32x3];

    /// Returns the vertex buffer layout describing this vertex
    pub fn vertex_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteInstance>() as _,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: Self::ATTRS,
        }
    }
}

#[derive(Debug)]
pub struct Texture {
    pub bind_group: wgpu::BindGroup,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub view: wgpu::TextureView,
    pub dimensions: (u32, u32),
}

impl Texture {
    pub fn bind_group_layout(device: &wgpu::Device) -> &wgpu::BindGroupLayout {
        TEXTURE_BIND_GROUP_LAYOUT.get_or_init(|| {
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
            })
        })
    }

    pub fn create_texture_bind_group(
        device: &wgpu::Device,
        label: Option<&str>,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: Self::bind_group_layout(device),
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

    pub fn empty(
        device: &wgpu::Device,
        label: Option<&str>,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> anyhow::Result<Self> {
        let view = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = view.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = Self::create_texture_bind_group(device, label, &view, &sampler);

        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label,
            contents: bytemuck::cast_slice(&texture_vertices(size.0, size.1)),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label,
            contents: bytemuck::cast_slice(&TEXTURE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        Ok(Self {
            bind_group,
            vertex_buffer,
            index_buffer,
            view,
            dimensions: size,
        })
    }

    pub fn from_file<P: AsRef<Path>>(
        path: P,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<Self> {
        let name = path.as_ref().to_str().unwrap_or_default().to_string();
        let image = image::load_from_memory(&std::fs::read(path)?)?;

        let rgba = image.to_rgba8();
        let dimensions = image.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&name),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(dimensions.0 * 4),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = Self::create_texture_bind_group(
            device,
            Some(&format!("{} bind group", name)),
            &view,
            &sampler,
        );

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} vertex buffer", name)),
            contents: bytemuck::cast_slice(&texture_vertices(dimensions.0, dimensions.1)),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} index buffer", name)),
            contents: bytemuck::cast_slice(&TEXTURE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        Ok(Self {
            bind_group,
            vertex_buffer,
            index_buffer,
            view,
            dimensions,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    texture: Rc<Texture>,
    origin: [f32; 2],
}

impl Frame {
    pub fn new(texture: Rc<Texture>, origin: [f32; 2]) -> Self {
        Self { texture, origin }
    }
}

#[derive(Debug)]
struct SpriteInstanceController {
    position: [f32; 2],
    depth: Option<f32>,
    instance_buffer: wgpu::Buffer,
}

impl SpriteInstanceController {
    fn position_3d(&self, frame: &Frame) -> [f32; 3] {
        [
            self.position[0] - frame.origin[0],
            self.position[1] - frame.origin[1],
            self.depth.unwrap_or_default(),
        ]
    }

    fn render<'pass>(
        &'pass self,
        renderer: &'pass Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
        frame: &'pass Frame,
    ) {
        render_pass.set_pipeline(
            renderer
                .pipeline(if self.depth.is_some() {
                    "texture_depth"
                } else {
                    "texture"
                })
                .expect("texture render pipeline does not exist!"),
        );
        render_pass.set_vertex_buffer(0, frame.texture.vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            frame.texture.index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.set_bind_group(1, &frame.texture.bind_group, &[]);
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.draw_indexed(0..6 as _, 0, 0..1);
    }

    fn set_position(&mut self, position: [f32; 2], renderer: &Renderer, frame: &Frame) {
        self.position = position;
        renderer.queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&[SpriteInstance {
                position: self.position_3d(frame),
            }]),
        )
    }

    fn set_depth(&mut self, depth: Option<f32>, renderer: &Renderer, frame: &Frame) {
        self.depth = depth;
        renderer.queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&[SpriteInstance {
                position: self.position_3d(frame),
            }]),
        )
    }
}

#[derive(Debug)]
pub struct Sprite {
    frame: Frame,
    controller: SpriteInstanceController,
}

impl Sprite {
    pub fn dimensions(&self) -> (u32, u32) {
        self.frame.texture.dimensions
    }

    /// Returns the relative bounding box of the sprite.
    ///
    /// This will be two points, the top left corner and the bottom left corner of the box. This
    /// box defines the bounds of the sprite *relative to its position*. So, if a sprite is 100x100
    /// and centred, its bounding box will always be ([-50, -50], [50, 50]), regardless of its
    /// actual position on the screen.
    pub fn relative_bounding_box(&self) -> ([f32; 2], [f32; 2]) {
        let dimensions = self.dimensions();
        let (dx, dy) = (dimensions.0 as f32, dimensions.1 as f32);

        let start = [-self.frame.origin[0], -self.frame.origin[1]];
        let end = [start[0] + dx, start[1] + dy];
        (start, end)
    }

    pub fn set_position(&mut self, position: [f32; 2], renderer: &Renderer) {
        self.controller
            .set_position(position, renderer, &self.frame)
    }

    pub fn set_depth(&mut self, depth: Option<f32>, renderer: &Renderer) {
        self.controller.set_depth(depth, renderer, &self.frame)
    }
}

impl Renderable for Sprite {
    fn render<'pass>(
        &'pass self,
        renderer: &'pass Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        self.controller.render(renderer, render_pass, &self.frame);
    }
}

#[derive(Debug, Copy, Clone)]
pub enum PlaybackState {
    Stopped,
    Playing { frame_time: f32 },
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::Stopped
    }
}

#[derive(Debug)]
pub struct AnimatedSprite {
    frames: Vec<Frame>,
    index: usize,
    progress: f32,
    playback_state: PlaybackState,
    looping: bool,
    controller: SpriteInstanceController,
}

impl AnimatedSprite {
    pub fn current_frame(&self) -> &Frame {
        &self.frames[self.index]
    }

    pub fn set_position(&mut self, position: [f32; 2], renderer: &Renderer) {
        self.controller
            .set_position(position, renderer, &self.frames[self.index])
    }

    pub fn set_depth(&mut self, depth: Option<f32>, renderer: &Renderer) {
        self.controller
            .set_depth(depth, renderer, &self.frames[self.index])
    }

    pub fn set_index(&mut self, index: usize, renderer: &Renderer) {
        assert!(
            index < self.frames.len(),
            "index out of bounds (the index was {index} but there are only {} frames",
            self.frames.len()
        );
        self.index = index;
        // Reset the position as the anchor point may have changed
        self.set_position(self.controller.position, renderer)
    }

    pub fn update(&mut self, delta_time: f32, renderer: &Renderer) {
        let PlaybackState::Playing { frame_time } = self.playback_state else {
            return;
        };
        self.progress += delta_time;

        if self.progress >= frame_time {
            while self.progress >= frame_time {
                self.progress -= frame_time;
            }

            if self.looping {
                self.set_index((self.index + 1) % self.frames.len(), renderer);
            } else {
                if self.index < self.frames.len() - 1 {
                    self.set_index(self.index + 1, renderer);
                }
            }
        }
    }
}

impl Renderable for AnimatedSprite {
    fn render<'pass>(&'pass self, renderer: &'pass Renderer, render_pass: &mut RenderPass<'pass>) {
        self.controller
            .render(renderer, render_pass, self.current_frame())
    }
}

#[derive(Clone, Debug)]
pub struct SpriteBuilder {
    texture: Rc<Texture>,
    position: [f32; 2],
    depth: Option<f32>,
    origin: [f32; 2],
}

impl SpriteBuilder {
    pub fn new(texture: Rc<Texture>) -> Self {
        Self {
            texture,
            position: [0., 0.],
            depth: None,
            origin: [0., 0.],
        }
    }

    pub fn texture(mut self, texture: Rc<Texture>) -> Self {
        self.texture = texture;
        self
    }

    pub fn position(mut self, position: [f32; 2]) -> Self {
        self.position = position;
        self
    }

    pub fn depth(mut self, depth: Option<f32>) -> Self {
        self.depth = depth;
        self
    }

    /// The origin of the sprite is the point relative to the sprite that will be drawn at the
    /// sprite's position.
    ///
    /// For example, if the origin is [0, 0] (the default, which is in the top
    /// left corner), and the sprite's position is [100, 100], then the sprite will be drawn with
    /// its top left corner at [100, 100].
    ///
    /// Another example: you can set the origin to [width / 2, height / 2] to centre the sprite.
    pub fn origin(mut self, origin: [f32; 2]) -> Self {
        self.origin = origin;
        self
    }

    /// Centres the sprite, i.e. sets the sprite's origin to the centre of the sprite.
    ///
    /// See [SpriteBuilder::origin]
    pub fn centre(mut self) -> Self {
        let dimensions = self.texture.dimensions;
        let centre = [dimensions.0 as f32 / 2., dimensions.1 as f32 / 2.];
        self.origin = centre;
        self
    }

    pub fn build(self, renderer: &Renderer) -> Sprite {
        let instance = SpriteInstance {
            position: [
                self.position[0] - self.origin[0],
                self.position[1] - self.origin[1],
                self.depth.unwrap_or_default(),
            ],
        };

        let instance_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("sprite instance buffer"),
                    contents: bytemuck::cast_slice(&[instance]),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        Sprite {
            frame: Frame {
                texture: self.texture,
                origin: self.origin,
            },
            controller: SpriteInstanceController {
                position: self.position,
                depth: self.depth,
                instance_buffer,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct AnimatedSpriteBuilder {
    frames: Vec<Frame>,
    index: usize,
    looping: bool,
    playback_state: PlaybackState,
    position: [f32; 2],
    depth: Option<f32>,
}

impl AnimatedSpriteBuilder {
    pub fn new(frames: Vec<Frame>) -> Self {
        assert!(
            !frames.is_empty(),
            "Animated sprite must have at least one frame"
        );

        Self {
            frames,
            index: 0,
            looping: false,
            playback_state: PlaybackState::Stopped,
            position: [0.; 2],
            depth: None,
        }
    }

    pub fn position(mut self, position: [f32; 2]) -> Self {
        self.position = position;
        self
    }

    pub fn depth(mut self, depth: Option<f32>) -> Self {
        self.depth = depth;
        self
    }

    pub fn index(mut self, index: usize) -> Self {
        self.index = index;
        self
    }

    pub fn playback_state(mut self, state: PlaybackState) -> Self {
        self.playback_state = state;
        self
    }

    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    pub fn build(self, renderer: &Renderer) -> AnimatedSprite {
        let instance = SpriteInstance {
            position: [
                self.position[0] - self.frames[self.index].origin[0],
                self.position[1] - self.frames[self.index].origin[1],
                self.depth.unwrap_or_default(),
            ],
        };

        let instance_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("sprite instance buffer"),
                    contents: bytemuck::cast_slice(&[instance]),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        AnimatedSprite {
            frames: self.frames,
            index: self.index,
            looping: self.looping,
            progress: 0.0,
            playback_state: self.playback_state,
            controller: SpriteInstanceController {
                position: self.position,
                depth: self.depth,
                instance_buffer,
            },
        }
    }
}
