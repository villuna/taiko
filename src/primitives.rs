//! Primitives - functions for constructing and drawing primitives

use lyon::lyon_tessellation::{
    math::point, BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor,
    StrokeOptions, StrokeTessellator, StrokeVertex, StrokeVertexConstructor, VertexBuffers,
};
use wgpu::vertex_attr_array;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Zeroable, bytemuck::Pod)]
pub struct PrimitiveVertex {
    pub position: [f32; 2],
    pub colour: [f32; 4],
}

struct VertexBuilder {
    colour: [f32; 4],
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

pub fn circle_filled(
    centre: [f32; 2],
    radius: f32,
    colour: [f32; 4],
) -> anyhow::Result<(Vec<PrimitiveVertex>, Vec<u32>)> {
    let mut output: VertexBuffers<PrimitiveVertex, u32> = VertexBuffers::new();
    let mut tesselator = FillTessellator::new();

    tesselator.tessellate_circle(
        point(centre[0], centre[1]),
        radius,
        &FillOptions::DEFAULT,
        &mut BuffersBuilder::new(&mut output, VertexBuilder { colour }),
    )?;

    Ok((output.vertices, output.indices))
}

pub fn circle(
    centre: [f32; 2],
    radius: f32,
    colour: [f32; 4],
    stroke_width: f32,
) -> anyhow::Result<(Vec<PrimitiveVertex>, Vec<u32>)> {
    let mut output: VertexBuffers<PrimitiveVertex, u32> = VertexBuffers::new();
    let mut tesselator = StrokeTessellator::new();

    tesselator.tessellate_circle(
        point(centre[0], centre[1]),
        radius,
        &StrokeOptions::DEFAULT
            .with_line_width(stroke_width),
        &mut BuffersBuilder::new(&mut output, VertexBuilder { colour }),
    )?;

    Ok((output.vertices, output.indices))
}
