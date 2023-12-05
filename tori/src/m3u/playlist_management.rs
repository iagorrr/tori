use std::{
    fs,
    io::{self, Write},
    path, rc,
    result::Result as StdResult,
    thread,
};

use crate::{
    app::App,
    config::{Config, OptionalConfig},
    error::Result,
    events::Event,
    m3u,
};

/// Adds a song to an existing playlist
pub fn add_song(app: &mut App, playlist: &str, song_path: String) {
    app.notify_info(format!("Adding {}...", song_path));

    if surely_invalid_path(&song_path) {
        app.notify_err(format!("Failed to add song path '{}'. Doesn't look like a URL and is not a valid path in your filesystem.", song_path));
        return;
    }

    let sender = app.channel.sender.clone();
    let playlist = playlist.to_string();
    // thread::spawn(move || {
    add_song_recursively(&song_path, &playlist);

    // Extract last part (separated by '/') of the song_path
    let mut rsplit = song_path.trim_end_matches('/').rsplit('/');
    let song = rsplit.next().unwrap_or(&song_path).to_string();

    let event = Event::SongAdded { playlist, song };
    sender.send(event).expect("Failed to send internal event");
    // });
}

/// Adds songs from some path. If the path points to a directory, it'll traverse the directory
/// recursively, adding all songs inside it. If the path points to a file, it'll add that file.
/// If it points to a URL, it adds the url.
/// We do not traverse symlinks, to avoid infinite loops.
fn add_song_recursively(path: &str, playlist_name: &str) {
    let file = std::path::Path::new(&path);
    if file.is_dir() && !file.is_symlink() {
        let mut entries: Vec<_> = fs::read_dir(path)
            .unwrap_or_else(|e| panic!("Failed to read directory '{}'. Error: {}", path, e))
            .map(|entry| entry.expect("Failed to read entry").path())
            .collect();

        entries.sort();

        for path in entries {
            let path = path.to_str().unwrap_or_else(|| {
                panic!(
                    "Failed to add '{}' to playlist. Path is not valid UTF-8",
                    path.display()
                )
            });
            add_song_recursively(path, playlist_name);
        }
    } else if !image_file(file) {
        let song = m3u::Song::from_path(path)
            .unwrap_or_else(|e| panic!("Failed to add '{}' to playlist. Error: {}", path, e));
        song.add_to_playlist(playlist_name)
            .unwrap_or_else(|e| panic!("Failed to add '{}' to playlist. Error: {}", path, e));
    }
}

fn surely_invalid_path(path: &str) -> bool {
    let file = std::path::Path::new(&path);
    !file.is_dir() // not a directory...
        && !file.exists() // ...or a valid filepath...
        && !path.starts_with("http://") // ...or a URL...
        && !path.starts_with("https://")
        && !path.starts_with("ytdl://")
}

fn image_file(file: &std::path::Path) -> bool {
    matches!(
        file.extension().and_then(|s| s.to_str()),
        Some("png") | Some("jpg") | Some("jpeg") | Some("webp") | Some("svg")
    )
}

#[derive(Debug)]
pub enum CreatePlaylistError {
    PlaylistAlreadyExists,
    InvalidChar(char),
    IOError(io::Error),
}

#[derive(Debug)]
pub enum RenamePlaylistError {
    PlaylistAlreadyExists,
    EmptyPlaylistName,
    InvalidChar(char),
    IOError(io::Error),
}

impl From<io::Error> for CreatePlaylistError {
    fn from(value: io::Error) -> Self {
        Self::IOError(value)
    }
}

/// Creates the corresponding .m3u8 file for a new playlist
pub fn create_playlist(playlist_name: &str) -> StdResult<(), CreatePlaylistError> {
    if playlist_name.contains('/') {
        return Err(CreatePlaylistError::InvalidChar('/'));
    }
    if playlist_name.contains('\\') {
        return Err(CreatePlaylistError::InvalidChar('\\'));
    }

    let path = Config::playlist_path(playlist_name);

    // TODO: when it's stabilized, use std::fs::File::create_new
    if path.try_exists()? {
        Err(CreatePlaylistError::PlaylistAlreadyExists)
    } else {
        fs::File::create(path)?;
        Ok(())
    }
}

pub fn delete_song(playlist_name: &str, index: usize) -> Result<()> {
    let path = Config::playlist_path(playlist_name);
    let content = fs::read_to_string(&path)?;
    let mut parser = m3u::Parser::from_string(&content);

    parser.next_header()?;
    for _ in 0..index {
        parser.next_song()?;
    }

    let start_pos = parser.cursor();
    let _song = parser.next_song()?;
    let end_pos = parser.cursor();

    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)?;
    file.write_all(content[..start_pos].as_bytes())?;
    file.write_all(content[end_pos..].as_bytes())?;

    Ok(())
}

pub fn rename_song(playlist_name: &str, index: usize, new_name: &str) -> Result<()> {
    let path = Config::playlist_path(playlist_name);
    let content = fs::read_to_string(&path)?;
    let mut parser = m3u::Parser::from_string(&content);

    parser.next_header()?;
    for _ in 0..index {
        parser.next_song()?;
    }

    let start_pos = parser.cursor();
    let song = parser.next_song()?;
    let end_pos = parser.cursor();

    if let Some(mut song) = song {
        song.title = new_name.to_string();
        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&path)?;
        file.write_all(content[..start_pos].as_bytes())?;
        file.write_all(song.serialize().as_bytes())?;
        file.write_all(content[end_pos..].as_bytes())?;
    }

    Ok(())
}

/// Swaps `index`-th song with the `index+1`-th (0-indexed)
pub fn swap_song(playlist_name: &str, index: usize) -> Result<()> {
    let path = Config::playlist_path(playlist_name);
    let content = fs::read_to_string(&path)?;
    let mut parser = m3u::Parser::from_string(&content);

    parser.next_header()?;
    for _ in 0..index {
        parser.next_song()?;
    }

    let start_pos = parser.cursor();
    let song1 = parser.next_song()?;
    let song2 = parser.next_song()?;
    let end_pos = parser.cursor();

    if let (Some(song1), Some(song2)) = (song1, song2) {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&path)?;
        file.write_all(content[..start_pos].as_bytes())?;
        file.write_all(song2.serialize().as_bytes())?;
        file.write_all(song1.serialize().as_bytes())?;
        file.write_all(content[end_pos..].as_bytes())?;
    }

    Ok(())
}

pub fn rename_playlist(playlist_name: &str, new_name: &str) -> StdResult<(), RenamePlaylistError> {
    if new_name.is_empty() {
        return Err(RenamePlaylistError::EmptyPlaylistName);
    }

    if new_name.contains('/') {
        return Err(RenamePlaylistError::InvalidChar('/'));
    }

    if new_name.contains('\\') {
        return Err(RenamePlaylistError::InvalidChar('\\'));
    }

    let old_path: path::PathBuf = Config::playlist_path(playlist_name);
    let new_path: path::PathBuf = Config::playlist_path(new_name);

    if let Ok(metadata) = fs::metadata(new_path.clone()) {
        if metadata.is_file() || metadata.is_dir() {
            return Err(RenamePlaylistError::PlaylistAlreadyExists);
        }
    }

    match fs::rename(&old_path, &new_path) {
        Err(e) => Err(RenamePlaylistError::IOError(e)),
        Ok(_) => Ok(()),
    }
}

pub fn delete_playlist(playlist_name: &str) -> Result<()> {
    let path = Config::playlist_path(playlist_name);
    fs::remove_file(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m3u::parser;
    use serial_test::serial;
    use std::sync::Once;
    use std::vec;

    static INIT: Once = Once::new();

    fn setup() {
        // Set a new folder to tori
        let playlist_dir: String = dirs::cache_dir()
            .expect("Playlist dir failed")
            .join("test_tori")
            .as_os_str()
            .to_str()
            .expect("Playlist dir failed")
            .to_string();

        let optconf = OptionalConfig {
            playlists_dir: Some(playlist_dir.clone()),
            visualizer_gradient: None,
            keybindings: None,
            mpv_ao: None,
        };

        INIT.call_once(|| {
            Config::set_global(Config::default().merge(optconf));
        });

        std::fs::remove_dir_all(&playlist_dir).expect("Remove dir all failed !");
        std::fs::create_dir_all(&playlist_dir).expect("Create a dir failed !");
    }

    #[test]
    #[serial]
    fn test__rename_playlist_files_being_changed() {
        setup();

        let mut app = App::new().expect("Failed to load app");
        let name: &str = "Original Playlist";
        let new_name: &str = "Renamed Playlist";
        create_playlist(name).expect("Failed to create playlist while testing");

        match rename_playlist(name, new_name) {
            Ok(()) => {
                assert!(!Config::playlist_path(name)
                    .try_exists()
                    .expect("Failed to check if path exists"));
                assert!(Config::playlist_path(new_name)
                    .try_exists()
                    .expect("Failed to check if path exists"));
            }
            _ => {}
        }
    }

    #[test]
    #[serial]
    fn test_rename_playlist_musics_are_being_kept() {
        setup();
        let mut app = App::new().expect("Failed to load app");

        let original_playlist: &str = "Pagans";
        let original_songs: Vec<String> = vec![
            "https://www.youtube.com/watch?v=KIvb6YchUsM".to_string(),
            "https://www.youtube.com/watch?v=Yyphq7C62jE".to_string(),
            "https://www.youtube.com/watch?v=EVnvh76S24s".to_string(),
        ];
        let original_songs_name: Vec<String> = vec![
            "Pagan".to_string(),
            "Pagan, Pt. 2".to_string(),
            "VITALISM | PAGAN III | GUITAR PLAYTHROUGH".to_string(),
        ];

        create_playlist(original_playlist);
        original_songs
            .iter()
            .for_each(|song| add_song(&mut app, original_playlist, song.to_string()));

        rename_playlist("Pagans", "Religious");
        let path = dirs::cache_dir()
            .unwrap()
            .join("test_tori")
            .join("Religious.m3u8");

        let mut parser = parser::Parser::from_path(path).unwrap();
        let songs_names: Vec<String> = parser
            .all_songs()
            .unwrap()
            .iter()
            .map(|song_name| song_name.title.clone())
            .collect();

        original_songs_name.iter().for_each(|song_name| {
            assert!(songs_names.iter().any(|title| title == song_name));
        });
    }

    #[test]
    #[serial]
    fn test_rename_playlist_new_name_empty() {
        setup();
        let mut app = App::new().expect("Failed to load app while testing");

        let original_playlist: &str = "Test Playlist";
        create_playlist(original_playlist);

        let result = rename_playlist(original_playlist, "");

        assert!(matches!(
            result,
            StdResult::Err(RenamePlaylistError::EmptyPlaylistName)
        ));
    }

    #[test]
    #[serial]
    fn test_rename_playlist_new_name_already_exists() {
        setup();
        let mut app = App::new().expect("Failed to load app while testing");

        let playlist1: &str = "Playlist 1";
        let playlist2: &str = "Playlist 2";
        create_playlist(playlist1);
        create_playlist(playlist2);

        let result = rename_playlist(playlist1, playlist2);

        assert!(matches!(
            result,
            StdResult::Err(RenamePlaylistError::PlaylistAlreadyExists)
        ));
    }

    #[test]
    #[serial]
    fn test_rename_playlist_contains_invalid_chars() {
        setup();
        let mut app = App::new().expect("Failed to load app while testing");
        let invalid_chars: Vec<char> = vec!['\\', '/'];

        let playlist_name: &str = "Playlist 1";
        create_playlist(playlist_name);

        invalid_chars.iter().for_each(|invalid_char| {
            assert!(matches!(
                rename_playlist(playlist_name, invalid_char.to_string().as_str()),
                Err(RenamePlaylistError::InvalidChar(invalid_char))
            ));
        });
    }
}
