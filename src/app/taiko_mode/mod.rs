mod note;

use lyon::{
    geom::point,
    path::Path,
    tessellation::{BuffersBuilder, StrokeOptions},
};
use wgpu_text::glyph_brush::{HorizontalAlign, Layout, SectionBuilder, VerticalAlign};
use winit::event::VirtualKeyCode;

use super::{GameState, StateTransition, TextureCache};
use crate::{beatmap_parser::Song, render::{
    shapes::{LinearGradient, Shape, ShapeBuilder, SolidColour}, text::Text, texture::Sprite, Renderer
}};

// Colours
const HEADER_TOP_COL: [f32; 4] = [30. / 255., 67. / 255., 198. / 255., 0.94];
const HEADER_BOTTOM_COL: [f32; 4] = [150. / 255., 90. / 255., 225. / 255., 1.];
const NOTE_FIELD_COL: [f32; 4] = [45. / 255., 45. / 255., 45. / 255., 1.];
const CREAM: [f32; 4] = [255. / 255., 235. / 255., 206. / 255., 1.];
const RECEPTICLE_COL: [f32; 4] = [0.26, 0.26, 0.26, 1.0];
const LEFT_PANEL_TOP_COL: [f32; 4] = [1., 73. / 255., 73. / 255., 1.];
const LEFT_PANEL_BOTTOM_COL: [f32; 4] = [229./255., 41. / 255., 41. / 255., 1.];

const HEADER_HEIGHT: f32 = 315.;
const SPACER_WIDTH: f32 = 8.;
const NOTE_FIELD_Y: f32 = HEADER_HEIGHT + SPACER_WIDTH;
// The point on the screen where notes should be hit
const NOTE_HIT_X: f32 = 690.;
// The Y value where notes should be drawn
const NOTE_Y: f32 = NOTE_FIELD_Y + NOTE_FIELD_HEIGHT / 2.0;
const NOTE_FIELD_HEIGHT: f32 = 232.;
const LEFT_PANEL_WIDTH: f32 = 480.;

pub struct TaikoMode {
    // UI Stuff
    background: Sprite,
    // TOOD: Give sprites a colour tint
    background_dim: Shape,
    header: Header,
    note_field: NoteField,
}

impl TaikoMode {
    pub fn new(song: &Song, diff: usize, renderer: &mut Renderer, textures: &mut TextureCache) -> anyhow::Result<Self> {
        let background = Sprite::new(
            textures.get(&renderer.device, &renderer.queue, "song_select_bg.jpg")?,
            [0.0; 3],
            &renderer.device,
            false,
        );

        let background_dim = ShapeBuilder::new()
            .filled_rectangle(
                [0., 0.],
                [1920., 1080.],
                SolidColour::new([0., 0., 0., 0.6]),
            )?
            .build(&renderer.device);

        Ok(Self {
            background,
            background_dim,
            header: Header::new(renderer, &song.title)?,
            note_field: NoteField::new(renderer)?,
        })
    }
}

impl GameState for TaikoMode {
    fn render<'pass>(&'pass mut self, ctx: &mut super::RenderContext<'_, 'pass>) {
        ctx.render(&self.background);
        ctx.render(&self.background_dim);
        self.header.render(ctx);
        self.note_field.render(ctx);
    }

    fn update(&mut self, ctx: &mut super::Context, _delta_time: f32) -> super::StateTransition {
        if ctx.keyboard.is_pressed(VirtualKeyCode::Escape) {
            StateTransition::Pop
        } else {
            StateTransition::Continue
        }
    }
}

struct Header {
    background: Shape,
    title: Text,
}

impl Header {
    fn new(renderer: &mut Renderer, title: &str) -> anyhow::Result<Self> {
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
            .with_layout(Layout::default().h_align(HorizontalAlign::Right).v_align(VerticalAlign::Top))
            .with_text(vec![wgpu_text::glyph_brush::Text::new(title)
                .with_color([1.0, 1.0, 1.0, 1.0])
                .with_scale(120.0)]);

        let title = Text::new_outlined(renderer, &title)?;

        Ok(Self { background, title })
    }

    fn render<'pass>(&'pass mut self, ctx: &mut super::RenderContext<'_, 'pass>) {
        ctx.render(&self.background);
        ctx.render(&self.title);
    }
}

struct NoteField {
    field: Shape,
    left_panel: Shape,
}

impl NoteField {
    fn new(renderer: &mut Renderer) -> anyhow::Result<Self> {
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
                ).ok_or(anyhow::format_err!("couldnt construct linear gradient"))?,
            )?
            .filled_rectangle(
                [LEFT_PANEL_WIDTH, NOTE_FIELD_Y],
                [LEFT_PANEL_WIDTH + 3., NOTE_FIELD_Y + NOTE_FIELD_HEIGHT],
                SolidColour::new([0., 0., 0., 1.]),
            )?
            .build(&renderer.device);

        Ok(Self { field, left_panel })
    }

    fn render<'pass>(&'pass mut self, ctx: &mut super::RenderContext<'_, 'pass>) {
        ctx.render(&self.field);
        // Render notes here
        ctx.render(&self.left_panel);
    }
}
