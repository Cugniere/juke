//! Application state and main loop.

use crate::config::Config;
use crate::player::{Player, PlayerError};
use crate::playlist::Playlist;
use std::io::{self, Write};
use std::time::Duration;

/// Main application state.
pub struct App {
    player: Player,
    playlist: Playlist,
    config: Config,
    running: bool,
    last_display_update: std::time::Instant,
}

impl App {
    /// Creates a new application with the given playlist and config.
    pub fn new(playlist: Playlist, config: Config) -> Result<Self, PlayerError> {
        let player = Player::new()?;

        Ok(Self {
            player,
            playlist,
            config,
            running: true,
            last_display_update: std::time::Instant::now(),
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

    /// Displays the current status (simple text output).
    fn display_status(&self) {
        // Clear screen (simple approach)
        print!("\x1B[2J\x1B[1;1H");
        io::stdout().flush().unwrap();

        println!("╔════════════════════════════════════════════════════════════╗");
        println!("║  juke - minimalist music player                           ║");
        println!("╚════════════════════════════════════════════════════════════╝");
        println!();

        if let Some(track) = self.playlist.current_track() {
            println!("  ♪  {}", track.display_name());
            println!();

            let pos = self.player.current_position();
            let dur = self.player.duration();
            println!("  ⏱  {:02}:{:02} / {:02}:{:02}",
                pos.as_secs() / 60, pos.as_secs() % 60,
                dur.as_secs() / 60, dur.as_secs() % 60);

            println!();
            let state_icon = match self.player.state() {
                crate::player::PlaybackState::Playing => "▶ Playing",
                crate::player::PlaybackState::Paused => "⏸ Paused",
                crate::player::PlaybackState::Stopped => "⏹ Stopped",
            };
            println!("  {}  ", state_icon);

            println!();
            println!("  Track {}/{}  |  Shuffle: {:?}  |  Repeat: {:?}",
                self.playlist.current_index() + 1,
                self.playlist.len(),
                self.playlist.shuffle_state(),
                self.playlist.repeat_mode());
        } else {
            println!("  No track loaded");
        }

        println!();
        println!("────────────────────────────────────────────────────────────");
        println!("  Space: Play/Pause  |  n: Next  |  p: Previous  |  q: Quit");
        println!("  →: Seek +{}s  |  ←: Seek -{}s  |  s: Shuffle  |  r: Repeat",
            self.config.playback.seek_step,
            self.config.playback.seek_step);
        println!("────────────────────────────────────────────────────────────");

        io::stdout().flush().unwrap();
    }
}
