use std::{io, path::Path, rc::Rc};

use crate::{
    game::credits::CreditsScreen,
    notechart_parser::{parse_tja_file, Song},
    render::texture::SpriteBuilder,
};

use crate::render::{texture::Sprite, Renderer};

use egui::RichText;
use kira::{
    manager::AudioManager,
    sound::{
        static_sound::{StaticSoundData, StaticSoundSettings},
        streaming::{StreamingSoundData, StreamingSoundHandle, StreamingSoundSettings},
        FromFileError,
    },
    tween::Tween,
};
use lazy_static::lazy_static;

use crate::game::{
    taiko_mode::TaikoMode, Context, GameState, RenderContext, StateTransition, TextureCache,
};

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

// Potentially this could go in config but i'm not sure that's necessary
const SONGS_DIR: &str = "songs";

pub struct SongSelect {
    songs: Vec<Song>,
    selected: Option<usize>,
    difficulty: usize,
    song_preview_handle: Option<SongHandle>,
    bg_sprite: Rc<Sprite>,
    go_to_credits: bool,
    exit: bool,
    go_to_song: Option<(usize, usize)>,
}

fn read_song_list_dir<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<Song>> {
    let dir = std::fs::read_dir(path)?;
    let mut res = Vec::new();

    for file in dir.flatten() {
        if file.file_type().map(|ty| ty.is_dir()).unwrap_or(false) {
            let subdir_path = file.path();

            match read_song_dir(&subdir_path) {
                Ok(song) => res.push(song),
                Err(e) => log::error!(
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
    pub fn new(textures: &mut TextureCache, renderer: &Renderer) -> anyhow::Result<Self> {
        let test_tracks = read_song_list_dir(SONGS_DIR)?;
        let bg_sprite = SpriteBuilder::new(textures.get(
            &renderer.device,
            &renderer.queue,
            "song_select_bg.jpg",
        )?)
        .build(renderer);

        Ok(SongSelect {
            songs: test_tracks,
            bg_sprite: Rc::new(bg_sprite),
            selected: None,
            difficulty: 0,
            song_preview_handle: None,
            go_to_credits: false,
            exit: false,
            go_to_song: None,
        })
    }

    fn play_preview(
        &mut self,
        audio: &mut AudioManager,
        selected: usize,
    ) -> anyhow::Result<StreamingSoundHandle<FromFileError>> {
        let selected = &self.songs[selected];

        let settings = StreamingSoundSettings::default()
            .playback_region(selected.demostart as f64..)
            .fade_in_tween(Some(*IN_TWEEN))
            .loop_region(selected.demostart as f64..);

        let song = StreamingSoundData::from_file(&selected.audio_filename, settings)?;

        Ok(audio.play(song)?)
    }
}

impl GameState for SongSelect {
    fn update(&mut self, ctx: &mut Context, _dt: f32) -> StateTransition {
        if self.go_to_credits {
            if let Some(handle) = self.song_preview_handle.as_mut() {
                handle.stop(*OUT_TWEEN).unwrap();
            }

            self.go_to_credits = false;
            StateTransition::Push(Box::new(CreditsScreen::new()))
        } else if let Some((song_id, difficulty)) = self.go_to_song {
            let sound_data = StaticSoundData::from_file(
                &self.songs[song_id].audio_filename,
                StaticSoundSettings::default(),
            )
            .unwrap();

            self.go_to_song = None;

            if let Some(handle) = self.song_preview_handle.as_mut() {
                handle.stop(Default::default()).unwrap();
            }

            StateTransition::Push(Box::new(
                TaikoMode::new(
                    &self.songs[song_id],
                    sound_data,
                    ctx.audio,
                    difficulty,
                    ctx.renderer,
                    ctx.textures,
                )
                .expect("error creating taiko mode scene"),
            ))
        } else if self.exit {
            StateTransition::Pop
        } else {
            StateTransition::Continue
        }
    }
    fn render<'pass>(&'pass mut self, ctx: &mut RenderContext<'_, 'pass>) {
        ctx.render(self.bg_sprite.as_ref())
    }

    fn debug_ui(&mut self, ctx: egui::Context, audio: &mut AudioManager) {
        egui::SidePanel::left("main menu")
            .resizable(false)
            .show(&ctx, |ui| {
                ui.label(
                    RichText::new("Taiko Clone Demo!")
                        .text_style(egui::TextStyle::Heading)
                        .size(40.0)
                        .color(egui::Color32::from_rgb(255, 84, 54))
                        .strong(),
                );

                ui.add_space(50.0);

                let old_song = self.selected;

                egui::ComboBox::from_label("Song select")
                    .selected_text(
                        RichText::new(
                            self.selected
                                .map(|id| self.songs[id].title.as_str())
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

                        for (id, song) in self.songs.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.selected,
                                Some(id),
                                RichText::new(&song.title).size(15.0),
                            );
                        }
                    });

                if self.selected != old_song {
                    if let Some(handle) = self.song_preview_handle.as_mut() {
                        handle.stop(*OUT_TWEEN).unwrap();
                    }

                    self.song_preview_handle = self
                        .selected
                        .map(|id| self.play_preview(audio, id).unwrap());
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.add_space(10.0);

                    if ui.button(RichText::new("return").size(20.0)).clicked() {
                        self.exit = true;
                    }

                    ui.add_space(10.0);

                    if ui.button(RichText::new("credits").size(20.0)).clicked() {
                        self.go_to_credits = true;
                    }
                });
            });

        if let Some(song_index) = self.selected {
            egui::Window::new("difficulty select").show(&ctx, |ui| {
                const DIFFICULTY_NAMES: [&str; 5] = ["Easy", "Normal", "Hard", "Oni", "Ura"];

                egui::TopBottomPanel::top("difficulty select panel").show_inside(ui, |ui| {
                    for (i, difficulty) in self.songs[song_index]
                        .difficulties
                        .iter()
                        .enumerate()
                        .filter_map(|(i, d)| d.as_ref().map(|dinner| (i, dinner)))
                    {
                        egui::SidePanel::left(format!("{} difficulty block", DIFFICULTY_NAMES[i]))
                            .show_inside(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.difficulty,
                                    i,
                                    RichText::new(format!(
                                        "{}\n{}★",
                                        DIFFICULTY_NAMES[i], difficulty.star_level
                                    ))
                                    .size(20.0),
                                );
                            });
                    }
                });

                if ui.button(RichText::new("Play!").size(17.0)).clicked() {
                    self.go_to_song = Some((song_index, self.difficulty));
                }
            });
        }
    }
}
