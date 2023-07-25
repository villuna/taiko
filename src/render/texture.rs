//! Various types used for drawing textures

use crate::render;
use image::GenericImageView;
use std::{path::Path, rc::Rc};
use wgpu::{util::DeviceExt, vertex_attr_array};

use super::context::Renderable;

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

const TEXTURE_INDICES: &[u16] = &[0, 1, 2, 1, 3, 2];

impl TextureVertex {
    const ATTRS: &[wgpu::VertexAttribute] = &vertex_attr_array![0 => Float32x2, 1 => Float32x2];

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
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteInstance {
    pub position: [f32; 3],
}

impl SpriteInstance {
    const ATTRS: &[wgpu::VertexAttribute] = &vertex_attr_array![2 => Float32x3];

    /// Returns the vertex buffer layout describing this vertex
    pub fn vertex_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteInstance>() as _,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: Self::ATTRS,
        }
    }
}

pub struct Texture {
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    dimensions: (u32, u32),
}

impl Texture {
    pub fn from_file<P: AsRef<Path>>(path: P, renderer: &render::Renderer) -> anyhow::Result<Self> {
        let name = path.as_ref().to_str().unwrap_or_default().to_string();
        let image = image::load_from_memory(&std::fs::read(path)?)?;

        let rgba = image.to_rgba8();
        let dimensions = image.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&name),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        renderer.queue.write_texture(
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

        let sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = renderer.create_texture_bind_group(
            Some(&format!("{} bind group", name)),
            &view,
            &sampler,
        );

        let vertex_buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} vertex buffer", name)),
                contents: bytemuck::cast_slice(&texture_vertices(dimensions.0, dimensions.1)),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} index buffer", name)),
                contents: bytemuck::cast_slice(TEXTURE_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        Ok(Self {
            bind_group,
            vertex_buffer,
            index_buffer,
            dimensions,
        })
    }
}

pub struct Sprite {
    texture: Rc<Texture>,
    instance: SpriteInstance,
    instance_buffer: wgpu::Buffer,
}

impl Sprite {
    pub fn new(texture: Rc<Texture>, position: [f32; 3], renderer: &render::Renderer) -> Self {
        let instance = SpriteInstance { position };

        let instance_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None, //TODO probably give this a name aye
                    contents: bytemuck::cast_slice(&[instance]),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        Sprite {
            texture,
            instance,
            instance_buffer,
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.texture.dimensions
    }

    pub fn position(&self) -> [f32; 3] {
        self.instance.position
    }

    pub fn set_position(&mut self, position: [f32; 3], queue: &wgpu::Queue) {
        self.instance.position = position;
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&[self.instance]),
        )
    }
}

impl Renderable for Sprite {
    fn render<'a>(&'a self, ctx: &mut render::context::RenderContext<'a>) {
        ctx.render_pass.set_pipeline(ctx.pipeline("texture").expect("texture render pipeline does not exist!"));
        ctx.render_pass.set_vertex_buffer(0, self.texture.vertex_buffer.slice(..));
        ctx.render_pass.set_index_buffer(
            self.texture.index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        ctx.render_pass.set_bind_group(1, &self.texture.bind_group, &[]);
        ctx.render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        ctx.render_pass.draw_indexed(0..6 as _, 0, 0..1);
    }
}
