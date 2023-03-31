use std::{path::PathBuf, thread, io};

use crate::{app::App, config::Config, events::Event, m3u};

/// Adds a song to an existing playlist
pub fn add_song(app: &mut App, playlist: &str, song_path: String) {
    app.notify_info(format!("Adding {}...", song_path));
    let sender = app.channel.sender.clone();
    let playlist = playlist.to_string();
    thread::spawn(move || {
        add_song_recursively(&song_path, &playlist);

        // Extract last part (separated by '/') of the song_path
        let mut rsplit = song_path.trim_end_matches('/').rsplit('/');
        let song = rsplit.next().unwrap_or(&song_path).to_string();

        let event = Event::SongAdded { playlist, song };
        sender.send(event).expect("Failed to send internal event");
    });
}

/// Adds songs from some path. If the path points to a directory, it'll traverse the directory
/// recursively, adding all songs inside it. If the path points to a file, it'll add that file.
/// If it points to a URL, it adds the url.
/// We do not traverse symlinks, to avoid infinite loops.
fn add_song_recursively(path: &str, playlist_name: &str) {
    let file = std::path::Path::new(&path);
    if file.is_dir() && !file.is_symlink() {
        let mut entries: Vec<_> = std::fs::read_dir(path)
            .expect("Failed to read dir")
            .map(|entry| entry.expect("Failed to read entry").path())
            .collect();

        entries.sort();

        for path in entries {
            let path = path.to_str().expect("Failed to convert path to str");
            add_song_recursively(path, playlist_name);
        }
    } else {
        let song = m3u::Song::from_path(path).expect("Failed to parse song");
        song.add_to_playlist(playlist_name)
            .unwrap_or_else(|e| panic!("Failed to add '{}' to playlist. Error: {}", path, e));
    }
}

#[derive(Debug)]
pub enum CreatePlaylistError {
    PlaylistAlreadyExists,
    IOError(io::Error),
}

impl From<io::Error> for CreatePlaylistError {
    fn from(value: io::Error) -> Self {
        Self::IOError(value)
    }
}

/// Creates the corresponding .m3u8 file for a new playlist
pub fn create_playlist(playlist_name: &str) -> Result<(), CreatePlaylistError> {
    let path =
        PathBuf::from(&Config::global().playlists_dir).join(format!("{}.m3u8", playlist_name));

    // TODO: when it's stabilized, use std::fs::File::create_new
    if path.try_exists()? {
        Err(CreatePlaylistError::PlaylistAlreadyExists)
    } else {
        std::fs::File::create(path)?;
        Ok(())
    }
}

pub fn delete_song(playlist_name: &str, index: usize) {
    unimplemented!()
}

pub fn rename_song(playlist_name: &str, index: usize, new_name: &str) {
    unimplemented!()
}

/// Swaps `index`-th song with the `index+1`-th (0-indexed)
pub fn swap_song(playlist_name: &str, index: usize) {
    unimplemented!()
}

