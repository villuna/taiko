//! Utilities for handling game settings
//!
//! The settings for lunataiko are stored in a toml file (by default `taiko_settings.toml`). Use
//! the function [read_settings] to read this config from file.
use std::ops::Deref;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use winit::keyboard::{KeyCode, PhysicalKey};

/// The path to the settings file
pub const SETTINGS_PATH: &str = "taiko_settings.toml";

pub static SETTINGS: RwLock<Settings> = RwLock::new(Settings {
    visual: VisualSettings {
        resolution: ResolutionState::BorderlessFullscreen,
    },
    game: GameSettings {
        global_note_offset: 0.0,
        key_mappings: KeyMap::default_mapping(),
    },
});

/// Convenience function that returns an immutable reference to [settings::SETTINGS].
/// Panics if the settings haven't been initialised yet.
pub fn settings() -> impl Deref<Target = Settings> {
    SETTINGS.read().unwrap()
}

/// All the settings for the game
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Settings {
    pub visual: VisualSettings,
    pub game: GameSettings,
}

impl Settings {
    pub fn key_is_don(&self, key: PhysicalKey) -> bool {
        key == self.game.key_mappings.left_don || key == self.game.key_mappings.right_don
    }

    pub fn key_is_kat(&self, key: PhysicalKey) -> bool {
        key == self.game.key_mappings.left_kat || key == self.game.key_mappings.right_kat
    }

    pub fn key_is_don_or_kat(&self, key: PhysicalKey) -> bool {
        self.key_is_don(key) || self.key_is_kat(key)
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(tag = "mode", content = "resolution")]
pub enum ResolutionState {
    #[default]
    BorderlessFullscreen,
    Windowed(u32, u32),
    Fullscreen(u32, u32),
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct VisualSettings {
    pub resolution: ResolutionState,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct GameSettings {
    pub global_note_offset: f32,
    pub key_mappings: KeyMap,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct KeyMap {
    pub left_don: PhysicalKey,
    pub right_don: PhysicalKey,
    pub left_kat: PhysicalKey,
    pub right_kat: PhysicalKey,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            global_note_offset: 0.0,
            key_mappings: KeyMap::default(),
        }
    }
}

impl KeyMap {
    const fn default_mapping() -> Self {
        Self {
            left_don: PhysicalKey::Code(KeyCode::KeyF),
            right_don: PhysicalKey::Code(KeyCode::KeyJ),
            left_kat: PhysicalKey::Code(KeyCode::KeyD),
            right_kat: PhysicalKey::Code(KeyCode::KeyK),
        }
    }
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::default_mapping()
    }
}

/// Try to ead and deserialize settings from the settings path.
///
/// If the file does not exist, it will create it with default settings. If it does exist but its
/// contents are in error, it will also return the default settings. Panics if it encounters any
/// other errors.
pub fn read_settings() {
    let settings = try_read_settings().unwrap_or_else(|e| match e {
        SettingsError::InvalidSettings => {
            eprintln!(
                "Couldn't read settings file due to invalid contents. \
                          Please fix the settings file at \"{}\". \
                          Continuing with default settings...",
                SETTINGS_PATH
            );

            Settings::default()
        }

        SettingsError::FileError(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                eprintln!(
                    "Settings file not found. Creating it at \"{}\"",
                    SETTINGS_PATH
                );

                let settings = Settings::default();

                std::fs::write(SETTINGS_PATH, toml::to_string(&settings).unwrap())
                    .unwrap_or_else(|_| panic!("couldnt write to file \"{}\"", SETTINGS_PATH));
                settings
            } else {
                panic!("unexpected error reading settings!: {e}");
            }
        }
    });

    *SETTINGS.write().unwrap() = settings;
}

/// Tries to read and deserialize config from the settings path.
///
/// Will return an error if the file does not exist, so the file must be created in this case.
fn try_read_settings() -> Result<Settings, SettingsError> {
    let str = std::fs::read_to_string(SETTINGS_PATH)?;

    Ok(toml::from_str(&str)?)
}

// Errors
#[derive(Debug)]
enum SettingsError {
    FileError(std::io::Error),
    InvalidSettings,
}

impl From<std::io::Error> for SettingsError {
    fn from(value: std::io::Error) -> Self {
        Self::FileError(value)
    }
}

impl From<toml::de::Error> for SettingsError {
    fn from(_: toml::de::Error) -> Self {
        Self::InvalidSettings
    }
}
