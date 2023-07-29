use std::time::Instant;

use egui_wgpu::renderer::ScreenDescriptor;
use winit::window::Window;

pub struct Egui {
    platform: egui_winit_platform::Platform,
    renderer: egui_wgpu::Renderer,
    start_time: Instant,
}

impl Egui {
    /// Creates a new egui handler.
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, scale_factor: f64) -> Self {
        let platform =
            egui_winit_platform::Platform::new(egui_winit_platform::PlatformDescriptor {
                physical_width: config.width,
                physical_height: config.height,
                scale_factor,
                ..Default::default()
            });

        let renderer =
            egui_wgpu::Renderer::new(device, config.format, Some(super::DEPTH_FORMAT), super::SAMPLE_COUNT);

        Self {
            platform,
            renderer,
            start_time: Instant::now(),
        }
    }

    /// Passes a winit event to egui for processing.
    ///
    /// Returns true if the event is "captured", which means it should not be handled by anything
    /// else (for example, clicking on an egui element should not also click behind it).
    pub fn handle_event<T>(&mut self, event: &winit::event::Event<'_, T>) -> bool {
        self.platform.handle_event(event);
        self.platform.captures_event(event)
    }

    pub fn begin_render(&mut self) {
        self.platform
            .update_time(self.start_time.elapsed().as_secs_f64());
        self.platform.begin_frame();
    }

    pub fn context(&self) -> egui::Context {
        self.platform.context()
    }

    pub fn end_render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        screen_descriptor: &ScreenDescriptor,
        window: &Window,
    ) -> Vec<egui::ClippedPrimitive> {
        let full_output = self.platform.end_frame(Some(window));
        let paint_jobs = self.platform.context().tessellate(full_output.shapes);
        let textures_delta = full_output.textures_delta;

        for texture in textures_delta.free.iter() {
            self.renderer.free_texture(texture);
        }

        for (id, image_delta) in textures_delta.set {
            self.renderer
                .update_texture(device, queue, id, &image_delta);
        }

        self.renderer
            .update_buffers(device, queue, encoder, &paint_jobs, screen_descriptor);

        paint_jobs
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        paint_jobs: Vec<egui::ClippedPrimitive>,
        screen_descriptor: &ScreenDescriptor,
    ) {
        self.renderer
            .render(render_pass, &paint_jobs, screen_descriptor);
    }
}
