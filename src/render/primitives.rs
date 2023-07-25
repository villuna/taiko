//! Primitives - functions for constructing and drawing primitives

use lyon::lyon_tessellation::{
    FillTessellator, FillVertex, FillVertexConstructor, StrokeTessellator, StrokeVertex,
    StrokeVertexConstructor, VertexBuffers,
};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    vertex_attr_array,
};

use super::context::Renderable;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Zeroable, bytemuck::Pod)]
pub struct PrimitiveVertex {
    pub position: [f32; 2],
    pub colour: [f32; 4],
}

pub struct VertexBuilder {
    pub colour: [f32; 4],
}

impl FillVertexConstructor<PrimitiveVertex> for VertexBuilder {
    fn new_vertex(&mut self, vertex: FillVertex) -> PrimitiveVertex {
        PrimitiveVertex {
            position: [vertex.position().x, vertex.position().y],
            colour: self.colour,
        }
    }
}

impl StrokeVertexConstructor<PrimitiveVertex> for VertexBuilder {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> PrimitiveVertex {
        PrimitiveVertex {
            position: [vertex.position().x, vertex.position().y],
            colour: self.colour,
        }
    }
}

impl PrimitiveVertex {
    const ATTRS: &[wgpu::VertexAttribute] = &vertex_attr_array![0 => Float32x2, 1 => Float32x4];

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PrimitiveVertex>() as _,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: Self::ATTRS,
        }
    }
}

#[derive(Debug)]
pub struct Primitive {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    indices: u32,
}

impl Primitive {
    pub fn filled_shape<F>(device: &wgpu::Device, mut build_shapes: F) -> anyhow::Result<Self>
    where
        F: FnMut(
            &mut FillTessellator,
            &mut VertexBuffers<PrimitiveVertex, u32>,
        ) -> anyhow::Result<()>,
    {
        let mut output: VertexBuffers<PrimitiveVertex, u32> = VertexBuffers::new();
        let mut tesselator = FillTessellator::new();

        build_shapes(&mut tesselator, &mut output)?;

        let vertex = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("primitive vertex buffer"),
            contents: bytemuck::cast_slice(&output.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("primitive index buffer"),
            contents: bytemuck::cast_slice(&output.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Ok(Primitive {
            vertex,
            index,
            indices: output.indices.len() as _,
        })
    }

    pub fn stroke_shape<F>(device: &wgpu::Device, mut build_shapes: F) -> anyhow::Result<Self>
    where
        F: FnMut(
            &mut StrokeTessellator,
            &mut VertexBuffers<PrimitiveVertex, u32>,
        ) -> anyhow::Result<()>,
    {
        let mut output: VertexBuffers<PrimitiveVertex, u32> = VertexBuffers::new();
        let mut tesselator = StrokeTessellator::new();

        build_shapes(&mut tesselator, &mut output)?;

        let vertex = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("primitive vertex buffer"),
            contents: bytemuck::cast_slice(&output.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("primitive index buffer"),
            contents: bytemuck::cast_slice(&output.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Ok(Primitive {
            vertex,
            index,
            indices: output.indices.len() as _,
        })
    }
}

impl Renderable for Primitive {
    fn render<'a>(&'a self, ctx: &mut super::RenderContext<'a>) {
        ctx.render_pass.set_pipeline(
            ctx.pipeline("primitive")
                .expect("primitive render pipeline doesn't exist!"),
        );
        ctx.render_pass.set_vertex_buffer(0, self.vertex.slice(..));
        ctx.render_pass
            .set_index_buffer(self.index.slice(..), wgpu::IndexFormat::Uint32);
        ctx.render_pass.draw_indexed(0..self.indices, 0, 0..1);
    }
}
