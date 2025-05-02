//! Constructing and drawing geometric shapes
//!
//! The main type in this module is the [Shape] struct, which can be constructed either with
//! convenience functions or with a [ShapeBuilder]. Shapes are constructed using the [lyon] crate,
//! so when constructing more complicated shapes you may need to interface with it (for example, in
//! the `ShapeBuilder`'s `filled_shape` and `stroke_shape` methods)

use crate::include_shader;
use crate::render::DEPTH_FORMAT;
use lyon::geom::vector;
use lyon::math::Angle;
use lyon::path::Winding;
use lyon::{
    geom::{point, Box2D},
    lyon_tessellation::{
        BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor,
        StrokeOptions, StrokeTessellator, StrokeVertex, StrokeVertexConstructor, TessellationError,
        VertexBuffers,
    },
    path::{builder::BorderRadii, Path},
};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    vertex_attr_array,
};

use super::{create_render_pipeline, Renderable, Renderer, SpriteInstance, SAMPLE_COUNT};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Zeroable, bytemuck::Pod)]
pub struct ShapeVertex {
    pub position: [f32; 3],
    pub colour: [f32; 4],
}

impl ShapeVertex {
    const ATTRS: &'static [wgpu::VertexAttribute] =
        &vertex_attr_array![0 => Float32x3, 1 => Float32x4];

    pub fn vertex_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ShapeVertex>() as _,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: Self::ATTRS,
        }
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

impl FillVertexConstructor<ShapeVertex> for SolidColour {
    fn new_vertex(&mut self, vertex: FillVertex) -> ShapeVertex {
        ShapeVertex {
            position: [vertex.position().x, vertex.position().y, 0.0],
            colour: self.colour,
        }
    }
}

impl StrokeVertexConstructor<ShapeVertex> for SolidColour {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> ShapeVertex {
        ShapeVertex {
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

impl FillVertexConstructor<ShapeVertex> for LinearGradient {
    fn new_vertex(&mut self, vertex: FillVertex) -> ShapeVertex {
        let position = [vertex.position().x, vertex.position().y, 0.0];

        let t = self.d
            * ((position[0] - self.from[0]) * (-self.th).cos()
                - (position[1] - self.from[1]) * (-self.th).sin());

        let colour = lerp_colour(self.colour1, self.colour2, t);

        ShapeVertex { position, colour }
    }
}

impl StrokeVertexConstructor<ShapeVertex> for LinearGradient {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> ShapeVertex {
        let position = [vertex.position().x, vertex.position().y, 0.0];

        let t = self.d
            * ((position[0] - self.from[0]) * (-self.th).cos()
                - (position[1] - self.from[1]) * (-self.th).sin());

        let colour = lerp_colour(self.colour1, self.colour2, t);

        ShapeVertex { position, colour }
    }
}

/// A shape built from coloured vertices
#[derive(Debug)]
pub struct Shape {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    instance: wgpu::Buffer,
    indices: u32,
    pipeline: &'static str,
}

/// A builder for creating complicated shapes made up of multiple primitives
pub struct ShapeBuilder {
    output: VertexBuffers<ShapeVertex, u32>,
    fill_tesselator: FillTessellator,
    stroke_tesselator: StrokeTessellator,
    pipeline: &'static str,
    position: [f32; 3],
}

impl ShapeBuilder {
    /// Constructs a new shape builder.
    ///
    /// By default, the shape will have position [0, 0, 0] and will not use depth when drawing.
    pub fn new() -> Self {
        Self {
            output: VertexBuffers::new(),
            fill_tesselator: FillTessellator::new(),
            stroke_tesselator: StrokeTessellator::new(),
            pipeline: "primitive",
            position: [0.; 3],
        }
    }

    /// Creates a [Shape] out of the constructed buffers, uploading the vertex, index and instance
    /// buffers onto the GPU. This consumes the builder.
    pub fn build(self, device: &wgpu::Device) -> Shape {
        let vertex = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("primitive vertex buffer"),
            contents: bytemuck::cast_slice(&self.output.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("primitive index buffer"),
            contents: bytemuck::cast_slice(&self.output.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let instance = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("primitive instance buffer"),
            contents: bytemuck::cast_slice(&[SpriteInstance {
                position: self.position,
            }]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Shape {
            vertex,
            index,
            instance,
            indices: self.output.indices.len() as _,
            pipeline: self.pipeline,
        }
    }

    /// Makes the shape respect depth.
    ///
    /// By default, whenever this shape is drawn its pixels will simply be drawn to the screen
    /// over whatever was there previously, without writing to the z buffer. If this is set, the
    /// shape will not be drawn over other pixels that have a lower z value.
    ///
    /// This does not work with custom render pipelines - calling this function is equivalent to
    /// calling `ShapeBuilder::with_pipeline("primitive depth")`. If you want your custom pipeline
    /// to respect depth, you must write that into the shader manually.
    pub fn has_depth(mut self) -> Self {
        self.pipeline = "primitive depth";
        self
    }

    /// Sets the shape to be drawn with a custom render pipeline.
    ///
    /// This pipeline must correspond with a pipeline in the [super::Renderer]'s shader cache, or
    /// the shape will crash at runtime when you try to render it.
    pub fn with_pipeline(mut self, pipeline: &'static str) -> Self {
        self.pipeline = pipeline;
        self
    }

    /// Sets the position of the shape.
    pub fn position(mut self, position: [f32; 3]) -> Self {
        self.position = position;
        self
    }

    /// Allows access to the [FillTessellator] and [VertexBuffers] so that you can add your own
    /// arbitrary filled shapes.
    pub fn filled_shape<F>(mut self, mut build_shapes: F) -> Result<Self, TessellationError>
    where
        F: FnMut(
            &mut FillTessellator,
            &mut VertexBuffers<ShapeVertex, u32>,
        ) -> Result<(), TessellationError>,
    {
        build_shapes(&mut self.fill_tesselator, &mut self.output)?;
        Ok(self)
    }

    /// Allows access to the [StrokeTessellator] and [VertexBuffers] so that you can add your own
    /// arbitrary stroke shapes.
    pub fn stroke_shape<F>(mut self, mut build_shapes: F) -> Result<Self, TessellationError>
    where
        F: FnMut(
            &mut StrokeTessellator,
            &mut VertexBuffers<ShapeVertex, u32>,
        ) -> Result<(), TessellationError>,
    {
        build_shapes(&mut self.stroke_tesselator, &mut self.output)?;
        Ok(self)
    }

    /// Constructs a filled rectangle, with bounds defined by min_point and max_point.
    pub fn filled_rectangle<C: FillVertexConstructor<ShapeVertex> + Clone>(
        mut self,
        min_point: [f32; 2],
        max_point: [f32; 2],
        colour: C,
    ) -> Result<Self, TessellationError> {
        let min = point(min_point[0], min_point[1]);

        let max = point(max_point[0], max_point[1]);

        self.fill_tesselator.tessellate_rectangle(
            &Box2D::new(min, max),
            &FillOptions::DEFAULT,
            &mut BuffersBuilder::new(&mut self.output, colour.clone()),
        )?;

        Ok(self)
    }

    /// Constructs a rectangle outline, with bounds defined by min_point and max_point.
    pub fn stroke_rectangle<C: StrokeVertexConstructor<ShapeVertex> + Clone>(
        mut self,
        min_point: [f32; 2],
        max_point: [f32; 2],
        colour: C,
        line_width: f32,
    ) -> Result<Self, TessellationError> {
        let min = point(min_point[0], min_point[1]);

        let max = point(max_point[0], max_point[1]);

        self.stroke_tesselator.tessellate_rectangle(
            &Box2D::new(min, max),
            &StrokeOptions::DEFAULT.with_line_width(line_width),
            &mut BuffersBuilder::new(&mut self.output, colour.clone()),
        )?;

        Ok(self)
    }

    /// Constructs a filled ellipse, with given centre point, radii and rotation.
    pub fn filled_ellipse<C: FillVertexConstructor<ShapeVertex> + Clone>(
        mut self,
        centre: [f32; 2],
        radii: [f32; 2],
        x_rotation: Angle,
        colour: C,
    ) -> Result<Self, TessellationError> {
        self.fill_tesselator.tessellate_ellipse(
            point(centre[0], centre[1]),
            vector(radii[0], radii[1]),
            x_rotation,
            Winding::Positive,
            &FillOptions::DEFAULT,
            &mut BuffersBuilder::new(&mut self.output, colour.clone()),
        )?;

        Ok(self)
    }

    /// Constructs an ellipse outline, with given centre point, radii and rotation.
    pub fn stroke_ellipse<C: StrokeVertexConstructor<ShapeVertex> + Clone>(
        mut self,
        centre: [f32; 2],
        radii: [f32; 2],
        x_rotation: Angle,
        colour: C,
        line_width: f32,
    ) -> Result<Self, TessellationError> {
        self.stroke_tesselator.tessellate_ellipse(
            point(centre[0], centre[1]),
            vector(radii[0], radii[1]),
            x_rotation,
            Winding::Positive,
            &StrokeOptions::DEFAULT.with_line_width(line_width),
            &mut BuffersBuilder::new(&mut self.output, colour.clone()),
        )?;

        Ok(self)
    }

    /// Constructs a filled circle, with given centre and radius
    pub fn filled_circle<C: FillVertexConstructor<ShapeVertex> + Clone>(
        mut self,
        centre: [f32; 2],
        radius: f32,
        colour: C,
    ) -> Result<Self, TessellationError> {
        let centre = point(centre[0], centre[1]);

        self.fill_tesselator.tessellate_circle(
            centre,
            radius,
            &FillOptions::DEFAULT,
            &mut BuffersBuilder::new(&mut self.output, colour.clone()),
        )?;

        Ok(self)
    }

    /// Constructs a circle outline, with given centre and radius
    pub fn stroke_circle<C: StrokeVertexConstructor<ShapeVertex> + Clone>(
        mut self,
        centre: [f32; 2],
        radius: f32,
        colour: C,
        line_width: f32,
    ) -> Result<Self, TessellationError> {
        let centre = point(centre[0], centre[1]);

        self.stroke_tesselator.tessellate_circle(
            centre,
            radius,
            &StrokeOptions::DEFAULT.with_line_width(line_width),
            &mut BuffersBuilder::new(&mut self.output, colour.clone()),
        )?;

        Ok(self)
    }

    /// Constructs a filled rounded rectangle, with bounds defined by min_point and max_point and
    /// corner radius defined by radius.
    pub fn filled_roundrect<C: FillVertexConstructor<ShapeVertex> + Clone>(
        mut self,
        min_point: [f32; 2],
        max_point: [f32; 2],
        radius: f32,
        colour: C,
    ) -> Result<Self, TessellationError> {
        let mut p = Path::builder();
        let min = point(min_point[0], min_point[1]);

        let max = point(max_point[0], max_point[1]);
        p.add_rounded_rectangle(
            &Box2D::new(min, max),
            &BorderRadii::new(radius),
            lyon::path::Winding::Positive,
        );

        self.fill_tesselator.tessellate_path(
            &p.build(),
            &FillOptions::DEFAULT,
            &mut BuffersBuilder::new(&mut self.output, colour.clone()),
        )?;

        Ok(self)
    }

    /// Constructs a rounded rectangle outline, with bounds defined by min_point and max_point and
    /// corner radius defined by radius.
    pub fn stroke_roundrect<C: StrokeVertexConstructor<ShapeVertex> + Clone>(
        mut self,
        min_point: [f32; 2],
        max_point: [f32; 2],
        radius: f32,
        colour: C,
        line_width: f32,
    ) -> Result<Self, TessellationError> {
        let mut p = Path::builder();
        let min = point(min_point[0], min_point[1]);

        let max = point(max_point[0], max_point[1]);
        p.add_rounded_rectangle(
            &Box2D::new(min, max),
            &BorderRadii::new(radius),
            lyon::path::Winding::Positive,
        );

        self.stroke_tesselator.tessellate_path(
            &p.build(),
            &StrokeOptions::DEFAULT.with_line_width(line_width),
            &mut BuffersBuilder::new(&mut self.output, colour.clone()),
        )?;

        Ok(self)
    }
}

impl Default for ShapeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Shape {
    /// Moves the whole shape to the given position.
    pub fn set_position(&self, position: [f32; 3], renderer: &Renderer) {
        renderer.queue.write_buffer(
            &self.instance,
            0,
            bytemuck::cast_slice(&[SpriteInstance { position }]),
        );
    }
}

impl Renderable for Shape {
    fn render<'pass>(
        &'pass self,
        renderer: &'pass Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        render_pass.set_pipeline(
            renderer.pipeline(self.pipeline).unwrap_or_else(|| {
                panic!("There is no render pipeline named \"{}\"", self.pipeline)
            }),
        );
        render_pass.set_bind_group(0, &renderer.screen_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex.slice(..));
        render_pass.set_vertex_buffer(1, self.instance.slice(..));
        render_pass.set_index_buffer(self.index.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.indices, 0, 0..1);
    }
}

/// Creates the pipelines (no depth and depth) needed to render shapes
/// This is called in render/mod.rs
pub fn create_primitive_pipelines(
    device: &wgpu::Device,
    screen_bgl: &wgpu::BindGroupLayout,
    config: &wgpu::SurfaceConfiguration,
) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
    let primitive_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Primitive pipeline layout"),
            bind_group_layouts: &[screen_bgl],
            push_constant_ranges: &[],
        });

    let primitive_shader =
        device.create_shader_module(include_shader!("shaders/primitive_shader.wgsl"));

    let primitive_pipeline = create_render_pipeline(
        &device,
        "primitive pipeline",
        &primitive_pipeline_layout,
        config.format,
        Some(DEPTH_FORMAT),
        false,
        &[
            ShapeVertex::vertex_layout(),
            SpriteInstance::vertex_layout(),
        ],
        &primitive_shader,
        SAMPLE_COUNT,
    );

    let primitive_pipeline_depth = create_render_pipeline(
        &device,
        "primitive pipeline",
        &primitive_pipeline_layout,
        config.format,
        Some(DEPTH_FORMAT),
        true,
        &[
            ShapeVertex::vertex_layout(),
            SpriteInstance::vertex_layout(),
        ],
        &primitive_shader,
        SAMPLE_COUNT,
    );

    (primitive_pipeline, primitive_pipeline_depth)
}
