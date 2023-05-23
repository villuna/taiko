use image::GenericImageView;
use std::{io, path::Path};

use crate::{parser::parse_tja_file, renderer, texture::TextureVertex, track::Song, HEIGHT, WIDTH};
use egui::RichText;
use kira::{
    manager::AudioManager,
    sound::{
        streaming::{StreamingSoundData, StreamingSoundHandle, StreamingSoundSettings},
        FromFileError,
    },
    tween::Tween,
};
use lazy_static::lazy_static;
use wgpu::util::DeviceExt;

use super::GameState;

type SongHandle = StreamingSoundHandle<FromFileError>;

lazy_static! {
    static ref IN_TWEEN: Tween = Tween {
        start_time: kira::StartTime::Immediate,
        duration: std::time::Duration::from_secs_f32(0.2),
        easing: kira::tween::Easing::OutPowi(2),
    };
    static ref OUT_TWEEN: Tween = Tween {
        start_time: kira::StartTime::Immediate,
        duration: std::time::Duration::from_secs_f32(0.2),
        easing: kira::tween::Easing::InPowi(2),
    };
}

const TEXTURE_VERTICES: &[TextureVertex] = &[
    TextureVertex {
        position: [0.0, 0.0],
        tex_coord: [0.0, 0.0],
    },
    TextureVertex {
        position: [0.0, HEIGHT as f32],
        tex_coord: [0.0, 1.0],
    },
    TextureVertex {
        position: [WIDTH as f32, 0.0],
        tex_coord: [1.0, 0.0],
    },
    TextureVertex {
        position: [WIDTH as f32, HEIGHT as f32],
        tex_coord: [1.0, 1.0],
    },
];

const TEXTURE_INDICES: &[u16] = &[0, 1, 2, 1, 3, 2];

const SONGS_DIR: &str = "songs";

pub struct SongSelect {
    test_tracks: Vec<Song>,
    selected: Option<usize>,
    song_handle: Option<SongHandle>,
    bg_bind_group: wgpu::BindGroup,
    bg_vertex_buffer: wgpu::Buffer,
    bg_index_buffer: wgpu::Buffer,
}

fn read_song_list_dir<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<Song>> {
    let dir = std::fs::read_dir(path)?;
    let mut res = Vec::new();

    for file in dir.flatten() {
        if file.file_type().map(|ty| ty.is_dir()).unwrap_or(false) {
            let subdir_path = file.path();

            match read_song_dir(&subdir_path) {
                Ok(song) => res.push(song),
                Err(e) => eprintln!(
                    "error encountered while trying to read song at directory {}: {e}",
                    subdir_path.to_string_lossy()
                ),
            }
        }
    }

    Ok(res)
}

fn read_song_dir<P: AsRef<Path>>(path: P) -> anyhow::Result<Song> {
    let dir_name = path.as_ref().file_name().ok_or(io::Error::new(
        io::ErrorKind::InvalidData,
        "couldn't read directory name",
    ))?;

    let tja_file_path = path
        .as_ref()
        .join(format!("{}.tja", dir_name.to_string_lossy()));
    let tja_file_contents = std::fs::read_to_string(tja_file_path)?;

    let mut song = parse_tja_file(&tja_file_contents)?;
    let audio_filename = path
        .as_ref()
        .join(&song.audio_filename)
        .to_string_lossy()
        .into_owned();

    song.audio_filename = audio_filename;
    Ok(song)
}

impl SongSelect {
    pub fn new(renderer: &renderer::Renderer) -> anyhow::Result<Self> {
        let test_tracks = read_song_list_dir(SONGS_DIR)?;

        let bg_image = image::load_from_memory(&std::fs::read("assets/song_select_bg.jpg")?)?;

        let rgba = bg_image.to_rgba8();
        let dimensions = bg_image.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let bg_texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("song select background"),
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
                texture: &bg_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: std::num::NonZeroU32::new(dimensions.0 * 4),
                rows_per_image: std::num::NonZeroU32::new(dimensions.1),
            },
            size,
        );

        let bg_view = bg_texture.create_view(&Default::default());

        let bg_sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bg_bind_group =
            renderer.create_texture_bind_group(Some("bg bind group"), &bg_view, &bg_sampler);

        let bg_vertex_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Bg vertex buffer"),
                    contents: bytemuck::cast_slice(TEXTURE_VERTICES),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let bg_index_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("bg index buffer"),
                    contents: bytemuck::cast_slice(TEXTURE_INDICES),
                    usage: wgpu::BufferUsages::INDEX,
                });

        Ok(SongSelect {
            test_tracks,
            bg_bind_group,
            bg_vertex_buffer,
            bg_index_buffer,
            selected: None,
            song_handle: None,
        })
    }

    fn play_preview(
        &mut self,
        audio: &mut AudioManager,
        selected: usize,
    ) -> anyhow::Result<StreamingSoundHandle<FromFileError>> {
        let selected = &self.test_tracks[selected];

        let settings = StreamingSoundSettings::default()
            .start_position(selected.demostart as _)
            .fade_in_tween(Some(*IN_TWEEN))
            .loop_behavior(Some(kira::LoopBehavior {
                start_position: selected.demostart as _,
            }));

        let song = StreamingSoundData::from_file(&selected.audio_filename, settings)?;

        Ok(audio.play(song)?)
    }
}

impl GameState for SongSelect {
    fn render<'a>(
        &'a mut self,
        renderer: &'a renderer::Renderer,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        render_pass.set_pipeline(renderer.texture_pipeline());
        render_pass.set_vertex_buffer(0, self.bg_vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.bg_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.set_bind_group(1, &self.bg_bind_group, &[]);
        render_pass.draw_indexed(0..TEXTURE_INDICES.len() as _, 0, 0..1);
    }

    fn debug_ui(&mut self, ctx: egui::Context, audio: &mut AudioManager) {
        egui::SidePanel::left("main menu")
            .resizable(false)
            .show(&ctx, |ui| {
                ui.label(
                    RichText::new("LunaTaiko Demo!")
                        .text_style(egui::TextStyle::Heading)
                        .size(40.0)
                        .color(egui::Color32::from_rgb(255, 84, 54))
                        .strong(),
                );

                ui.label(RichText::new("\"That's a working title!\"").italics());

                ui.add_space(50.0);

                let old_song = self.selected;

                egui::ComboBox::from_label("Song select")
                    .selected_text(
                        RichText::new(
                            self.selected
                                .map(|id| self.test_tracks[id].title.as_str())
                                .unwrap_or("None"),
                        )
                        .size(20.0),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.selected,
                            None,
                            RichText::new("none").size(15.0),
                        );

                        for (id, song) in self.test_tracks.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.selected,
                                Some(id),
                                RichText::new(&song.title).size(15.0),
                            );
                        }
                    });

                if self.selected != old_song {
                    if let Some(handle) = self.song_handle.as_mut() {
                        handle.stop(*OUT_TWEEN).unwrap();
                    }

                    self.song_handle = self
                        .selected
                        .map(|id| self.play_preview(audio, id).unwrap());
                }
            });
    }
}
