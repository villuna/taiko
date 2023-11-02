use std::rc::Rc;

use wgpu_text::glyph_brush::Section;

use crate::render::{context::Renderable, texture, Renderer, RenderPassContext};

#[derive(Debug)]
pub struct Text {
    pub sprite: texture::Sprite,
}

impl Text {
    pub fn new(renderer: &mut Renderer, section: &Section) -> anyhow::Result<Text> {
        let texture = Self::create_texture(renderer, section, false)?;

        let sprite = texture::Sprite::new(Rc::new(texture), [0.0; 3], &renderer.device, false);

        Ok(Self { sprite })
    }

    pub fn new_outlined(renderer: &mut Renderer, section: &Section) -> anyhow::Result<Text> {
        let texture = Self::create_texture(renderer, section, true)?;

        let sprite = texture::Sprite::new(Rc::new(texture), [0.0; 3], &renderer.device, false);

        Ok(Self { sprite })
    }

    fn create_texture(
        renderer: &mut Renderer,
        section: &Section,
        outline: bool,
    ) -> anyhow::Result<texture::Texture> {
        let mut texture = texture::Texture::empty(
            &renderer.device,
            Some("text texture"),
            renderer.config.format,
            (renderer.config.width, renderer.config.height),
        )?;

        renderer
            .text_brush
            .queue(&renderer.device, &renderer.queue, vec![section])?;

        let mut encoder = renderer
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("text command encoder"),
            });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Text render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        renderer.text_brush.draw(&mut render_pass);

        drop(render_pass);

        if outline {
            let outlined_texture = texture::Texture::empty(
                &renderer.device,
                Some("text texture"),
                renderer.config.format,
                (renderer.config.width, renderer.config.height),
            )?;

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Outline render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &outlined_texture.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(renderer.pipeline("outline").unwrap());
            render_pass.set_bind_group(0, &renderer.screen_bind_group, &[]);
            render_pass.set_bind_group(1, &texture.bind_group, &[]);
            render_pass.set_vertex_buffer(0, texture.vertex_buffer.slice(..));
            render_pass.set_index_buffer(texture.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..1);

            drop(render_pass);

            texture = outlined_texture;
        }

        renderer.queue.submit([encoder.finish()]);

        Ok(texture)
    }
}

impl Renderable for Text {
    fn render<'a>(&'a self, ctx: &mut RenderPassContext<'a>) {
        ctx.render(&self.sprite);
    }
}
