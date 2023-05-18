use kira::{
    manager::{backend::DefaultBackend, AudioManager},
    sound::{
        streaming::{StreamingSoundData, StreamingSoundHandle, StreamingSoundSettings},
        FromFileError,
    },
    tween::Tween,
};
use lazy_static::lazy_static;
use std::{io, path::Path};

use crate::{parser::parse_tja_file, track::Song};

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

const SONGS_DIR: &str = "songs";

pub enum StateTransition {
    Continue,
    Push(Box<dyn GameState>),
    Pop,
}

pub trait GameState {
    // TODO: Make a context struct instead of passing in the raw audio manager
    fn update(&mut self, _delta: f32, _audio: &mut AudioManager) -> StateTransition {
        StateTransition::Continue
    }
    fn debug_ui(&mut self, _ctx: egui::Context, _audio: &mut AudioManager) {}
}

pub struct SongSelect {
    test_tracks: Vec<Song>,
    selected: Option<usize>,
    song_handle: Option<SongHandle>,
}

pub struct App {
    audio_manager: AudioManager,
    state: Box<dyn GameState>,
}

fn read_song_list_dir<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<Song>> {
    let dir = std::fs::read_dir(path)?;
    let mut res = Vec::new();

    for file in dir {
        if let Ok(file) = file {
            if file.file_type().map(|ty| ty.is_dir()).unwrap_or(false) {
                let subdir_path = file.path();
                res.push(read_song_dir(subdir_path)?)
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
        .join(&format!("{}.tja", dir_name.to_string_lossy()));
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
    pub fn new() -> anyhow::Result<Self> {
        let test_tracks = read_song_list_dir(SONGS_DIR)?;

        Ok(SongSelect {
            test_tracks,
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
    fn debug_ui(&mut self, ctx: egui::Context, audio: &mut AudioManager) {
        egui::Window::new("Taiko Test!").show(&ctx, |ui| {
            ui.label("It works!");

            let old_song = self.selected;

            egui::ComboBox::from_label("Song select")
                .selected_text(
                    self.selected
                        .map(|id| self.test_tracks[id].title.as_str())
                        .unwrap_or("None"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.selected, None, "none");

                    for (id, song) in self.test_tracks.iter().enumerate() {
                        ui.selectable_value(&mut self.selected, Some(id), &song.title);
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

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let audio_manager = AudioManager::<DefaultBackend>::new(Default::default())?;
        let state = Box::new(SongSelect::new()?);

        Ok(App {
            audio_manager,
            state,
        })
    }

    pub fn update(&mut self, delta: f32) {
        self.state.update(delta, &mut self.audio_manager);
    }

    pub fn debug_ui(&mut self, ctx: egui::Context) {
        self.state.debug_ui(ctx, &mut self.audio_manager);
    }
}
