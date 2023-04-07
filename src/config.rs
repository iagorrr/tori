use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error::Error, io, path::PathBuf};

use crate::{
    command::Command,
    shortcuts::{InputStr, Shortcuts},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub playlists_dir: String,
    pub normal: Shortcuts,
}

static INSTANCE: OnceCell<Config> = OnceCell::new();

impl Config {
    pub fn global() -> &'static Self {
        INSTANCE.get().expect("Config instance not loaded!")
    }

    pub fn set_global(instance: Self) {
        INSTANCE.set(instance).unwrap();
    }

    pub fn playlist_path(playlist_name: &str) -> PathBuf {
        PathBuf::from(&Config::global().playlists_dir).join(format!("{}.m3u8", playlist_name))
    }

    pub fn merge(mut self, other: OptionalConfig) -> Self {
        if let Some(playlists_dir) = other.playlists_dir {
            self.playlists_dir = playlists_dir;
        }

        if let Some(normal) = other.normal {
            for (k, v) in normal.0 {
                self.normal.0.insert(k, v);
            }
        }

        self
    }
}

impl Default for Config {
    fn default() -> Self {
        let playlists_dir = dirs::audio_dir()
            .map(|p| p.join("tori"))
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or("playlists".into());

        let i = |s: &str, c: Command| (InputStr(s.into()), c);

        let normal = Shortcuts::new(HashMap::from([
            i("C-c", Command::Quit),
            i("C-d", Command::Quit),
            i("h", Command::SelectLeft),
            i("j", Command::SelectNext),
            i("k", Command::SelectPrev),
            i("l", Command::SelectRight),
            i(">", Command::NextSong),
            i("<", Command::PrevSong),
            i("q", Command::QueueSong),
            i("A-enter", Command::QueueShown),
            i("S-right", Command::SeekForward),
            i("S-left", Command::SeekBackward),
            i("o", Command::OpenInBrowser),
            i("y", Command::CopyUrl),
            i("t", Command::CopyTitle),
            i(" ", Command::TogglePause),
            i("A-up", Command::VolumeUp),
            i("A-down", Command::VolumeDown),
            i("m", Command::Mute),
            i("p", Command::PlayFromModal),
            i("a", Command::Add),
            i("R", Command::Rename),
            i("X", Command::Delete),
            i("J", Command::SwapSongDown),
            i("K", Command::SwapSongUp),
            i(",", Command::Shuffle),
            i("v", Command::ToggleVisualizer),
        ]));

        Self {
            playlists_dir,
            normal,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OptionalConfig {
    pub playlists_dir: Option<String>,
    pub normal: Option<Shortcuts>,
}

impl OptionalConfig {
    /// Loads the shortcuts from some path
    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        match std::fs::File::open(path) {
            Ok(file) => serde_yaml::from_reader(file)
                .map_err(|e| format!("Couldn't parse your config.yaml. Reason: {}", e).into()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
}
