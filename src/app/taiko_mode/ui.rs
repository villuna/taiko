use crate::app::RenderContext;
use crate::render::shapes::{LinearGradient, Shape, ShapeBuilder, SolidColour};
use crate::render::text::Text;
use crate::render::Renderer;
use lyon::geom::point;
use lyon::lyon_tessellation::{BuffersBuilder, StrokeOptions};
use lyon::path::Path;
use wgpu_text::glyph_brush::{HorizontalAlign, Layout, SectionBuilder, VerticalAlign};

use super::note::{TaikoModeBarline, TaikoModeNote};

// Colours
pub const HEADER_TOP_COL: [f32; 4] = [30. / 255., 67. / 255., 198. / 255., 0.94];
pub const HEADER_BOTTOM_COL: [f32; 4] = [150. / 255., 90. / 255., 225. / 255., 1.];
pub const NOTE_FIELD_COL: [f32; 4] = [45. / 255., 45. / 255., 45. / 255., 1.];
pub const CREAM: [f32; 4] = [255. / 255., 235. / 255., 206. / 255., 1.];
pub const RECEPTICLE_COL: [f32; 4] = [0.26, 0.26, 0.26, 1.0];
pub const LEFT_PANEL_TOP_COL: [f32; 4] = [1., 73. / 255., 73. / 255., 1.];
pub const LEFT_PANEL_BOTTOM_COL: [f32; 4] = [229. / 255., 41. / 255., 41. / 255., 1.];

// Positions.
// TODO: Replace this system something more sophisticated that respects resolution
pub const HEADER_HEIGHT: f32 = 315.;
pub const SPACER_WIDTH: f32 = 8.;
pub const NOTE_FIELD_Y: f32 = HEADER_HEIGHT + SPACER_WIDTH;
// The point on the screen where notes should be hit
pub const NOTE_HIT_X: f32 = 690.;
// The Y value where notes should be drawn
pub const NOTE_Y: f32 = NOTE_FIELD_Y + NOTE_FIELD_HEIGHT / 2.0;
pub const NOTE_FIELD_HEIGHT: f32 = 232.;
pub const LEFT_PANEL_WIDTH: f32 = 480.;

pub struct Header {
    background: Shape,
    title: Text,
}

impl Header {
    pub fn new(renderer: &mut Renderer, title: &str) -> anyhow::Result<Self> {
        let background = ShapeBuilder::new()
            .filled_rectangle(
                [0., 0.],
                [1920., HEADER_HEIGHT],
                LinearGradient::new(
                    HEADER_TOP_COL,
                    HEADER_BOTTOM_COL,
                    [0., 0.],
                    [0., HEADER_HEIGHT],
                )
                .ok_or(anyhow::format_err!("cant construct linear gradient"))?,
            )?
            .build(&renderer.device);

        let title = SectionBuilder::default()
            .with_screen_position((1840.0, 20.0))
            .with_layout(
                Layout::default()
                    .h_align(HorizontalAlign::Right)
                    .v_align(VerticalAlign::Top),
            )
            .with_text(vec![wgpu_text::glyph_brush::Text::new(title)
                .with_color([1.0, 1.0, 1.0, 1.0])
                .with_scale(120.0)]);

        let title = Text::new_outlined(renderer, &title)?;

        Ok(Self { background, title })
    }

    pub fn render<'pass>(&'pass mut self, ctx: &mut RenderContext<'_, 'pass>) {
        ctx.render(&self.background);
        ctx.render(&self.title);
    }
}

pub struct NoteField {
    field: Shape,
    left_panel: Shape,
}

impl NoteField {
    pub fn new(renderer: &mut Renderer) -> anyhow::Result<Self> {
        let field = ShapeBuilder::new()
            // Background
            .filled_rectangle(
                [0., NOTE_FIELD_Y],
                [1920., NOTE_FIELD_Y + NOTE_FIELD_HEIGHT],
                SolidColour::new(NOTE_FIELD_COL),
            )?
            // Top spacer
            .filled_rectangle(
                [0., HEADER_HEIGHT],
                [1920., HEADER_HEIGHT + SPACER_WIDTH],
                SolidColour::new(CREAM),
            )?
            // Bottom spacer
            .filled_rectangle(
                [0., NOTE_FIELD_Y + NOTE_FIELD_HEIGHT],
                [1920., NOTE_FIELD_Y + NOTE_FIELD_HEIGHT + SPACER_WIDTH],
                SolidColour::new(CREAM),
            )?
            // Note recepticle
            .stroke_shape(|tess, out| {
                let mut path = Path::builder();
                path.begin(point(NOTE_HIT_X, NOTE_Y - NOTE_FIELD_HEIGHT / 2.0));
                path.line_to(point(NOTE_HIT_X, NOTE_Y + NOTE_FIELD_HEIGHT / 2.0));
                path.end(false);

                let options = StrokeOptions::DEFAULT.with_line_width(4.0);
                let mut builder = BuffersBuilder::new(out, SolidColour::new(RECEPTICLE_COL));

                // A line that shows exactly where notes should be hit
                tess.tessellate_path(&path.build(), &options, &mut builder)?;

                // The outline of a small note
                tess.tessellate_circle(point(NOTE_HIT_X, NOTE_Y), 50.0, &options, &mut builder)?;

                // The outline of a large note
                tess.tessellate_circle(point(NOTE_HIT_X, NOTE_Y), 75.0, &options, &mut builder)?;

                Ok(())
            })?
            .build(&renderer.device);

        let left_panel = ShapeBuilder::new()
            .filled_rectangle(
                [0.0, NOTE_FIELD_Y],
                [LEFT_PANEL_WIDTH, NOTE_FIELD_Y + NOTE_FIELD_HEIGHT],
                LinearGradient::new(
                    LEFT_PANEL_TOP_COL,
                    LEFT_PANEL_BOTTOM_COL,
                    [0.0, NOTE_FIELD_Y],
                    [0.0, NOTE_FIELD_Y + NOTE_FIELD_HEIGHT],
                )
                .ok_or(anyhow::format_err!("couldnt construct linear gradient"))?,
            )?
            .filled_rectangle(
                [LEFT_PANEL_WIDTH, NOTE_FIELD_Y],
                [LEFT_PANEL_WIDTH + 3., NOTE_FIELD_Y + NOTE_FIELD_HEIGHT],
                SolidColour::new([0., 0., 0., 1.]),
            )?
            .build(&renderer.device);

        Ok(Self { field, left_panel })
    }

    pub fn render<'pass>(
        &'pass mut self,
        ctx: &mut RenderContext<'_, 'pass>,
        notes: impl Iterator<Item = &'pass TaikoModeNote>,
        barlines: impl Iterator<Item = &'pass TaikoModeBarline>,
    ) {
        ctx.render(&self.field);

        // Thankfully barlines are all drawn before all the notes
        // so we don't have to worry about ordering shenanigans :D
        for b in barlines {
            ctx.render(b);
        }

        for n in notes {
            ctx.render(n);
        }

        ctx.render(&self.left_panel);
    }
}
