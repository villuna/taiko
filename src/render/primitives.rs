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

/// A vertex builder that sets every vertex to a single colour.
///
/// "Solid" in this case doesn't mean "not transparent", it just means that there is no gradient. I
/// can't think of a better name. Sorry.
pub struct SolidColour {
    pub colour: [f32; 4],
}

impl SolidColour {
    pub fn new(colour: [f32; 4]) -> Self {
        Self { colour }
    }
}

impl FillVertexConstructor<PrimitiveVertex> for SolidColour {
    fn new_vertex(&mut self, vertex: FillVertex) -> PrimitiveVertex {
        PrimitiveVertex {
            position: [vertex.position().x, vertex.position().y],
            colour: self.colour,
        }
    }
}

impl StrokeVertexConstructor<PrimitiveVertex> for SolidColour {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> PrimitiveVertex {
        PrimitiveVertex {
            position: [vertex.position().x, vertex.position().y],
            colour: self.colour,
        }
    }
}

/// A vertex builder that colours vertices according to a linear gradient.
///
/// You should ideally make sure that all vertices constructed by this are within the gradient,
/// because due to the limitations of this approach, we cannot construct for instance, a gradient
/// that only spans a small portion of a rectangle.
pub struct LinearGradient {
    pub colour1: [f32; 4],
    pub colour2: [f32; 4],
    from: [f32; 2],
    d: f32,
    th: f32,
}

impl LinearGradient {
    /// Construct a new linear gradient vertex builder.
    ///
    /// `from` will be coloured in the first colour, and `to` in the second colour. The angle between
    /// the two points will define the gradient. Thus, these points cannot be the same, and this
    /// function returns None if they are.
    pub fn new(colour1: [f32; 4], colour2: [f32; 4], from: [f32; 2], to: [f32; 2]) -> Option<Self> {
        if from == to {
            None
        } else {
            let th = (to[1] - from[1]).atan2(to[0] - from[0]);
            let d = 1.0 / ((to[0] - from[0]).powi(2) + (to[1] - from[1]).powi(2)).sqrt();

            dbg!(d);
            dbg!(th);

            Some(Self {
                colour1,
                colour2,
                from,
                d,
                th,
            })
        }
    }
}

fn lerp_colour(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);

    [
        a[0] * (1.0 - t) + b[0] * t,
        a[1] * (1.0 - t) + b[1] * t,
        a[2] * (1.0 - t) + b[2] * t,
        a[3] * (1.0 - t) + b[3] * t,
    ]
}

impl FillVertexConstructor<PrimitiveVertex> for LinearGradient {
    fn new_vertex(&mut self, vertex: FillVertex) -> PrimitiveVertex {
        let position = [vertex.position().x, vertex.position().y];

        let t = self.d
            * ((position[0] - self.from[0]) * (-self.th).cos()
                - (position[1] - self.from[1]) * (-self.th).sin());

        let colour = lerp_colour(self.colour1, self.colour2, t);

        println!("position: {position:?}, colour: {colour:?}");

        PrimitiveVertex { position, colour }
    }
}

impl StrokeVertexConstructor<PrimitiveVertex> for LinearGradient {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> PrimitiveVertex {
        let position = [vertex.position().x, vertex.position().y];

        let t = self.d
            * ((position[0] - self.from[0]) * (-self.th).cos()
                - (position[1] - self.from[1]) * (-self.th).sin());

        let colour = lerp_colour(self.colour1, self.colour2, t);

        PrimitiveVertex { position, colour }
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
    /// Constructs a Primitive out of filled shapes.
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

    /// Constructs a Primitive out of the outlines of shapes.
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
