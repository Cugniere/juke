//! Application state and main loop.

use crate::config::Config;
use crate::player::{Player, PlayerError};
use crate::playlist::Playlist;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io;
use std::time::Duration;

/// Main application state.
pub struct App {
    player: Player,
    playlist: Playlist,
    config: Config,
    running: bool,
    last_display_update: std::time::Instant,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl App {
    /// Creates a new application with the given playlist and config.
    pub fn new(playlist: Playlist, config: Config) -> Result<Self, PlayerError> {
        let player = Player::new()?;

        let backend = CrosstermBackend::new(io::stdout());
        let terminal = Terminal::new(backend)
            .map_err(|e| PlayerError::InitializationError(e.to_string()))?;

        Ok(Self {
            player,
            playlist,
            config,
            running: true,
            last_display_update: std::time::Instant::now(),
            terminal,
        })
    }

    /// Starts the application and loads the first track.
    pub fn start(&mut self) -> Result<(), PlayerError> {
        if let Some(track) = self.playlist.current_track() {
            self.player.load_track(&track.path)?;
            self.player.play();
            self.display_status();
        }
        Ok(())
    }

    /// Updates the application state (called from main loop).
    pub fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Check if current track finished
        if self.player.has_track() && self.player.is_finished() {
            if self.playlist.next() {
                self.load_current_track()?;
            } else {
                // End of playlist
                self.running = false;
            }
        }

        // Update display periodically (every second)
        if self.last_display_update.elapsed() >= Duration::from_secs(1) {
            self.display_status();
            self.last_display_update = std::time::Instant::now();
        }

        Ok(())
    }

    /// Handles play/pause toggle.
    pub fn toggle_play_pause(&mut self) {
        self.player.toggle_play_pause();
        self.display_status();
    }

    /// Plays the next track.
    pub fn next_track(&mut self) -> Result<(), PlayerError> {
        if self.playlist.next() {
            self.load_current_track()?;
        }
        Ok(())
    }

    /// Plays the previous track.
    pub fn previous_track(&mut self) -> Result<(), PlayerError> {
        if self.playlist.previous() {
            self.load_current_track()?;
        }
        Ok(())
    }

    /// Seeks forward.
    pub fn seek_forward(&mut self) -> Result<(), PlayerError> {
        let step = Duration::from_secs(self.config.playback.seek_step as u64);
        self.player.seek_forward(step)?;
        self.display_status();
        Ok(())
    }

    /// Seeks backward.
    pub fn seek_backward(&mut self) -> Result<(), PlayerError> {
        let step = Duration::from_secs(self.config.playback.seek_step as u64);
        self.player.seek_backward(step)?;
        self.display_status();
        Ok(())
    }

    /// Toggles shuffle mode.
    pub fn toggle_shuffle(&mut self) {
        self.playlist.toggle_shuffle();
        self.display_status();
    }

    /// Cycles repeat mode.
    pub fn cycle_repeat(&mut self) {
        self.playlist.cycle_repeat();
        self.display_status();
    }

    /// Quits the application.
    pub fn quit(&mut self) {
        self.running = false;
    }

    /// Returns whether the app is running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Loads the current track from the playlist.
    fn load_current_track(&mut self) -> Result<(), PlayerError> {
        if let Some(track) = self.playlist.current_track() {
            self.player.load_track(&track.path)?;
            self.player.play();
            self.display_status();
        }
        Ok(())
    }

    /// Displays the current status using ratatui.
    fn display_status(&mut self) {
        self.terminal.draw(|f| {
            let size = f.area();

            // Create main layout
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Header
                    Constraint::Min(0),     // Content
                    Constraint::Length(3),  // Footer
                ])
                .split(size);

            // Header
            let header = Paragraph::new("juke - minimalist music player")
                .style(Style::default().fg(Color::Cyan))
                .block(Block::default().borders(Borders::ALL))
                .alignment(Alignment::Center);
            f.render_widget(header, chunks[0]);

            // Content
            let mut content_lines = vec![];

            if let Some(track) = self.playlist.current_track() {
                content_lines.push(Line::from(vec![
                    Span::raw("  ♪  "),
                    Span::styled(track.display_name(), Style::default().fg(Color::Yellow)),
                ]));

                // Artist and album
                if let Some(artist) = &track.artist {
                    if let Some(album) = &track.album {
                        content_lines.push(Line::from(format!("     Artist: {}  •  Album: {}", artist, album)));
                    } else {
                        content_lines.push(Line::from(format!("     Artist: {}", artist)));
                    }
                } else if let Some(album) = &track.album {
                    content_lines.push(Line::from(format!("     Album: {}", album)));
                }

                content_lines.push(Line::from(""));

                // Time
                let pos = self.player.current_position();
                let dur = self.player.duration();
                content_lines.push(Line::from(format!(
                    "  ⏱  {:02}:{:02} / {:02}:{:02}",
                    pos.as_secs() / 60, pos.as_secs() % 60,
                    dur.as_secs() / 60, dur.as_secs() % 60
                )));

                content_lines.push(Line::from(""));

                // State
                let state_icon = match self.player.state() {
                    crate::player::PlaybackState::Playing => "▶ Playing",
                    crate::player::PlaybackState::Paused => "⏸ Paused",
                    crate::player::PlaybackState::Stopped => "⏹ Stopped",
                };
                content_lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(state_icon, Style::default().fg(Color::Green)),
                ]));

                content_lines.push(Line::from(""));

                // Track info
                content_lines.push(Line::from(format!(
                    "  Track {}/{}  |  Shuffle: {:?}  |  Repeat: {:?}",
                    self.playlist.current_index() + 1,
                    self.playlist.len(),
                    self.playlist.shuffle_state(),
                    self.playlist.repeat_mode()
                )));
            } else {
                content_lines.push(Line::from("  No track loaded"));
            }

            let content = Paragraph::new(content_lines)
                .block(Block::default().borders(Borders::NONE));
            f.render_widget(content, chunks[1]);

            // Footer
            let footer_text = format!(
                "Space: Play/Pause | n: Next | p: Previous | q: Quit | →/←: Seek ±{}s | s: Shuffle | r: Repeat",
                self.config.playback.seek_step
            );
            let footer = Paragraph::new(footer_text)
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL))
                .alignment(Alignment::Center);
            f.render_widget(footer, chunks[2]);
        }).unwrap();
    }
}
