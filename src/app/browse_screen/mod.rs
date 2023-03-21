use crate::app::MyBackend;
use crate::app::Screen;
use crate::App;

use crate::events::Event;
use crossterm::event::KeyCode;
use std::error::Error;

use tui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

mod playlists;
use playlists::PlaylistsPane;

mod songs;
use songs::SongsPane;

mod add;
use add::AddPane;

mod now_playing;
use now_playing::NowPlaying;

use super::Mode;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i8)]
enum BrowsePane {
    #[default]
    Playlists,
    Songs,
    Add,
}

#[derive(Debug, Default)]
pub struct BrowseScreen<'a> {
    playlists: PlaylistsPane<'a>,
    songs: SongsPane<'a>,
    add: AddPane,
    now_playing: NowPlaying,
    selected_pane: BrowsePane,
}

impl<'a> BrowseScreen<'a> {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let playlists = PlaylistsPane::from_dir("playlists")?;
        let songs = SongsPane::from_playlist_pane(&playlists);
        Ok(Self {
            playlists,
            songs,
            ..Default::default()
        })
    }

    pub fn reload_songs(&mut self) {
        self.songs = SongsPane::from_playlist_pane(&self.playlists);
    }

    /// Passes the event down to the currently selected pane.
    fn pass_event_down(&mut self, app: &mut App, event: Event) -> Result<(), Box<dyn Error>> {
        use BrowsePane::*;
        match self.selected_pane {
            Playlists => self.playlists.handle_event(app, event),
            Songs => self.songs.handle_event(app, event),
            Add => self.add.handle_event(
                app,
                self.playlists.selected_item().map(|s| s.as_str()),
                event,
            ),
        }
    }
}

impl Screen for BrowseScreen<'_> {
    fn render(&mut self, frame: &mut Frame<'_, MyBackend>) {
        let size = frame.size();

        let vchunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(20), Constraint::Length(2)].as_ref())
            .split(size);

        let hchunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(15), Constraint::Percentage(85)].as_ref())
            .split(vchunks[0]);

        self.playlists.render(
            self.selected_pane == BrowsePane::Playlists,
            frame,
            hchunks[0],
        );
        self.songs
            .render(self.selected_pane == BrowsePane::Songs, frame, hchunks[1]);

        self.now_playing.render(frame, vchunks[1]);

        if self.selected_pane == BrowsePane::Add {
            self.add.render(frame);
        }
    }

    fn handle_event(&mut self, app: &mut App, event: Event) -> Result<(), Box<dyn Error>> {
        use crate::command::Command::*;
        use BrowsePane::*;
        use Event::*;
        use KeyCode::*;

        match event {
            Command(cmd) => match cmd {
                Quit => {
                    app.change_state(None);
                }
                _ => self.pass_event_down(app, Command(cmd))?,
            },
            SongAdded { playlist, song } => {
                // Reload songs pane
                self.songs = SongsPane::from_playlist_pane(&self.playlists);
                app.notify_ok(format!("\"{}\" was added to {}", song, playlist));
            }
            SecondTick => {
                self.now_playing.update(&app.mpv);
            }
            ChangedPlaylist => self.reload_songs(),
            Terminal(event) => match event {
                crossterm::event::Event::Key(event) => match event.code {
                    Esc | Enter if self.selected_pane == Add => {
                        self.pass_event_down(app, Terminal(crossterm::event::Event::Key(event)))?;
                        self.selected_pane = Songs;
                    }
                    Right | Left => {
                        self.selected_pane = match self.selected_pane {
                            Playlists => Songs,
                            Songs => Playlists,
                            Add => Add,
                        };
                    }
                    // play/pause
                    // TODO: I really need a better event system
                    Char(' ') if self.mode() == Mode::Normal => {
                        app.mpv.command("cycle", &["pause"])?;
                        self.now_playing.update(&app.mpv);
                    }
                    // 'a'dd
                    Char('a') if self.mode() == Mode::Normal => {
                        self.selected_pane = Add;
                        self.add = AddPane::new();
                    }
                    // 'c'hange
                    KeyCode::Char('c') if self.mode() == Mode::Normal => {
                        self.playlists.open_editor_for_selected()?;
                    }
                    _ => {
                        self.pass_event_down(app, Terminal(crossterm::event::Event::Key(event)))?
                    }
                },
                _ => {
                    self.pass_event_down(app, Terminal(event))?;
                }
            },
        }
        Ok(())
    }

    fn mode(&self) -> Mode {
        use BrowsePane::*;
        match self.selected_pane {
            Playlists => self.playlists.mode(),
            Songs => self.songs.mode(),
            Add => self.add.mode(),
        }
    }
}
