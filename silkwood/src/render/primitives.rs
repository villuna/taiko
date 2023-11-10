//! Primitives - functions for constructing and drawing primitives

use lyon::{lyon_tessellation::{
    FillTessellator, FillVertex, FillVertexConstructor, StrokeTessellator, StrokeVertex,
    StrokeVertexConstructor, VertexBuffers, FillOptions, BuffersBuilder, StrokeOptions,
}, path::{Path, builder::BorderRadii}, geom::{Box2D, point}};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    vertex_attr_array,
};

use super::{context::Renderable, SpriteInstance};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Zeroable, bytemuck::Pod)]
pub struct PrimitiveVertex {
    pub position: [f32; 3],
    pub colour: [f32; 4],
}

impl PrimitiveVertex {
    const ATTRS: &[wgpu::VertexAttribute] = &vertex_attr_array![0 => Float32x3, 1 => Float32x4];

    pub fn vertex_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PrimitiveVertex>() as _,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: Self::ATTRS,
        }
    }
}

/// A vetex builder that wraps around another builder and sets each vertex's z value to the one it
/// contains.
///
/// Should not be used directly. Instead just call one of the Primitive 'depth' constructor
/// functions.
#[derive(Copy, Clone, Debug)]
pub struct WithDepth<T> {
    inner: T,
    z: f32,
}

impl<T> WithDepth<T> {
    pub fn new(inner: T, z: f32) -> Self {
        Self { inner, z }
    }
}

impl<T> FillVertexConstructor<PrimitiveVertex> for WithDepth<T>
where
    T: FillVertexConstructor<PrimitiveVertex>,
{
    fn new_vertex(&mut self, vertex: FillVertex) -> PrimitiveVertex {
        let mut v = self.inner.new_vertex(vertex);
        v.position[2] = self.z;
        v
    }
}

impl<T> StrokeVertexConstructor<PrimitiveVertex> for WithDepth<T>
where
    T: StrokeVertexConstructor<PrimitiveVertex>,
{
    fn new_vertex(&mut self, vertex: StrokeVertex) -> PrimitiveVertex {
        let mut v = self.inner.new_vertex(vertex);
        v.position[2] = self.z;
        v
    }
}

/// A vertex builder that sets every vertex to a single colour.
///
/// "Solid" in this case doesn't mean "not transparent", it just means that there is no gradient. I
/// can't think of a better name. Sorry.
#[derive(Copy, Clone, Debug)]
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
            position: [vertex.position().x, vertex.position().y, 0.0],
            colour: self.colour,
        }
    }
}

impl StrokeVertexConstructor<PrimitiveVertex> for SolidColour {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> PrimitiveVertex {
        PrimitiveVertex {
            position: [vertex.position().x, vertex.position().y, 0.0],
            colour: self.colour,
        }
    }
}

/// A vertex builder that colours vertices according to a linear gradient.
///
/// You should ideally make sure that all vertices constructed by this are within the gradient,
/// because due to the limitations of this approach, we cannot construct for instance, a gradient
/// that only spans a small portion of a rectangle.
#[derive(Copy, Clone, Debug)]
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
        let position = [vertex.position().x, vertex.position().y, 0.0];

        let t = self.d
            * ((position[0] - self.from[0]) * (-self.th).cos()
                - (position[1] - self.from[1]) * (-self.th).sin());

        let colour = lerp_colour(self.colour1, self.colour2, t);

        PrimitiveVertex { position, colour }
    }
}

impl StrokeVertexConstructor<PrimitiveVertex> for LinearGradient {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> PrimitiveVertex {
        let position = [vertex.position().x, vertex.position().y, 0.0];

        let t = self.d
            * ((position[0] - self.from[0]) * (-self.th).cos()
                - (position[1] - self.from[1]) * (-self.th).sin());

        let colour = lerp_colour(self.colour1, self.colour2, t);

        PrimitiveVertex { position, colour }
    }
}

#[derive(Debug)]
pub struct Primitive {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    instance: wgpu::Buffer,
    indices: u32,
    has_depth: bool,
}

impl Primitive {
    pub fn filled_roundrect<C: FillVertexConstructor<PrimitiveVertex> + Clone>(
        device: &wgpu::Device,
        position: [f32; 3],
        dimensions: [f32; 2],
        radius: f32,
        has_depth: bool,
        colour: C,
    ) -> anyhow::Result<Self> {
        Self::filled_shape(device, position, has_depth, |tess, out| {
            let mut p = Path::builder();
            let min = point(0., 0.);
            let max = point(
                dimensions[0],
                dimensions[1],
            );
            p.add_rounded_rectangle(&Box2D::new(min, max), &BorderRadii::new(radius), lyon::path::Winding::Positive);
            
            tess.tessellate_path(
                &p.build(),
                &FillOptions::DEFAULT,
                &mut BuffersBuilder::new(
                    out,
                    colour.clone(),
                ),
            )?;

            Ok(())
        })
    }

    pub fn stroke_roundrect<C: StrokeVertexConstructor<PrimitiveVertex> + Clone>(
        device: &wgpu::Device,
        position: [f32; 3],
        dimensions: [f32; 2],
        radius: f32,
        has_depth: bool,
        colour: C,
        line_width: f32,
    ) -> anyhow::Result<Self> {
        Self::stroke_shape(device, position, has_depth, |tess, out| {
            let mut p = Path::builder();
            let min = point(0., 0.);

            let max = point(
                dimensions[0],
                dimensions[1],
            );
            p.add_rounded_rectangle(&Box2D::new(min, max), &BorderRadii::new(radius), lyon::path::Winding::Positive);
            
            tess.tessellate_path(
                &p.build(),
                &StrokeOptions::DEFAULT.with_line_width(line_width),
                &mut BuffersBuilder::new(
                    out,
                    colour.clone(),
                ),
            )?;

            Ok(())
        })
    }

    /// Constructs a Primitive out of filled shapes.
    pub fn filled_shape<F>(
        device: &wgpu::Device,
        position: [f32; 3],
        has_depth: bool,
        mut build_shapes: F,
    ) -> anyhow::Result<Self>
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

        let instance = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("primitive instance buffer"),
            contents: bytemuck::cast_slice(&[SpriteInstance { position }]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Ok(Primitive {
            vertex,
            index,
            instance,
            indices: output.indices.len() as _,
            has_depth,
        })
    }

    /// Constructs a Primitive out of the outlines of shapes.
    pub fn stroke_shape<F>(
        device: &wgpu::Device,
        position: [f32; 3],
        has_depth: bool,
        mut build_shapes: F,
    ) -> anyhow::Result<Self>
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

        let instance = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("primitive instance buffer"),
            contents: bytemuck::cast_slice(&[SpriteInstance { position }]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Ok(Primitive {
            vertex,
            index,
            instance,
            indices: output.indices.len() as _,
            has_depth,
        })
    }

    pub fn set_position(&self, position: [f32; 3], queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.instance,
            0,
            bytemuck::cast_slice(&[SpriteInstance { position }]),
        );
    }
}

impl Renderable for Primitive {
    fn render<'a>(&'a self, ctx: &mut super::RenderPassContext<'a>) {
        let pipeline = if self.has_depth {
            "primitive_depth"
        } else {
            "primitive"
        };

        ctx.render_pass.set_pipeline(
            ctx.pipeline(pipeline)
                .unwrap_or_else(|| panic!("{pipeline} render pipeline doesn't exist!")),
        );
        ctx.render_pass.set_vertex_buffer(0, self.vertex.slice(..));
        ctx.render_pass
            .set_vertex_buffer(1, self.instance.slice(..));
        ctx.render_pass
            .set_index_buffer(self.index.slice(..), wgpu::IndexFormat::Uint32);
        ctx.render_pass.draw_indexed(0..self.indices, 0, 0..1);
    }
}
