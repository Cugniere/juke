//! Application state and main loop.

use crate::config::Config;
use crate::player::{Player, PlayerError};
use crate::playlist::Playlist;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Terminal,
};
use std::io;
use std::time::Duration;

/// UI display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UIMode {
    Normal,
    TrackList,
    Help,
}

/// Main application state.
pub struct App {
    player: Player,
    playlist: Playlist,
    config: Config,
    running: bool,
    last_display_update: std::time::Instant,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    ui_mode: UIMode,
    search_query: String,
    waveform_history: Vec<f32>, // Rolling buffer of amplitude values for visualization
    track_list_selected: usize, // Selected index in filtered track list view
    filtered_indices: Vec<usize>, // Indices of tracks matching search filter
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
            ui_mode: UIMode::Normal,
            search_query: String::new(),
            waveform_history: vec![0.0; 12], // 12 fixed bars for visualization
            track_list_selected: 0,
            filtered_indices: Vec::new(),
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

        // Update waveform visualization
        if self.player.has_track() {
            self.update_waveform();
        }

        // Update display periodically for smooth waveform animation
        // Update every 30ms when playing, every second when paused
        let update_interval = if self.player.state() == crate::player::PlaybackState::Playing {
            Duration::from_millis(30)
        } else {
            Duration::from_secs(1)
        };

        if self.last_display_update.elapsed() >= update_interval {
            self.display_status();
            self.last_display_update = std::time::Instant::now();
        }

        Ok(())
    }

    /// Updates the waveform visualization data.
    fn update_waveform(&mut self) {
        // Update all bars independently (simulated based on playback state)
        if self.player.state() == crate::player::PlaybackState::Playing {
            // Generate bar heights based on time
            // In a real implementation, this would use FFT on actual audio data
            // Use modulo to keep time in a reasonable range for sine calculations
            let time = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_millis() % 60000) as f32 / 1000.0; // 0-60 seconds range

            // Update each bar with its own frequency to simulate different frequency bands
            for (i, bar) in self.waveform_history.iter_mut().enumerate() {
                // Each bar has a different base frequency (simulating bass to treble)
                let freq = 1.0 + (i as f32 * 0.5); // Frequencies from 1 Hz to 6.5 Hz
                let amplitude = (time * freq * std::f32::consts::PI).sin().abs();
                // Add some variation to make it more interesting
                let variation = (time * freq * 2.0).sin() * 0.3;
                *bar = (amplitude * 0.7 + variation.abs() * 0.3).min(1.0);
            }
        } else {
            // When paused, reset all bars to zero
            for bar in self.waveform_history.iter_mut() {
                *bar = 0.0;
            }
        }
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
        self.player.stop();
        self.running = false;
    }

    /// Stops playback before shutdown.
    pub fn stop_playback(&mut self) {
        self.player.stop();
    }

    /// Returns whether the app is running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Returns the current UI mode.
    pub fn ui_mode(&self) -> UIMode {
        self.ui_mode
    }

    /// Sets the UI mode.
    pub fn set_ui_mode(&mut self, mode: UIMode) {
        self.ui_mode = mode;
        if mode == UIMode::TrackList {
            // Initialize selection and update filtered indices
            self.update_filtered_indices();
            // Find the current track in filtered list
            let current_idx = self.playlist.current_index();
            self.track_list_selected = self.filtered_indices
                .iter()
                .position(|&idx| idx == current_idx)
                .unwrap_or(0);
        } else {
            self.search_query.clear();
        }
        self.display_status();
    }

    /// Updates the filtered track indices based on search query.
    fn update_filtered_indices(&mut self) {
        let search_lower = self.search_query.to_lowercase();
        self.filtered_indices.clear();

        for (idx, track) in self.playlist.tracks().iter().enumerate() {
            if search_lower.is_empty() {
                // No filter - include all tracks
                self.filtered_indices.push(idx);
            } else {
                // Check if track matches search
                let display_name = track.display_name().to_lowercase();
                let artist_name = track.artist.as_deref().unwrap_or("").to_lowercase();
                let album_name = track.album.as_deref().unwrap_or("").to_lowercase();

                if display_name.contains(&search_lower)
                    || artist_name.contains(&search_lower)
                    || album_name.contains(&search_lower)
                {
                    self.filtered_indices.push(idx);
                }
            }
        }

        // Reset selection to first filtered track if current selection is out of bounds
        if self.track_list_selected >= self.filtered_indices.len() {
            self.track_list_selected = 0;
        }
    }

    /// Adds a character to the search query.
    pub fn search_input(&mut self, c: char) {
        if self.ui_mode == UIMode::TrackList {
            self.search_query.push(c);
            self.update_filtered_indices();
            self.display_status();
        }
    }

    /// Removes the last character from the search query.
    pub fn search_backspace(&mut self) {
        if self.ui_mode == UIMode::TrackList {
            self.search_query.pop();
            self.update_filtered_indices();
            self.display_status();
        }
    }

    /// Clears the search query.
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.display_status();
    }

    /// Moves selection up in track list.
    pub fn track_list_up(&mut self) {
        if self.ui_mode == UIMode::TrackList && self.track_list_selected > 0 {
            self.track_list_selected -= 1;
            self.display_status();
        }
    }

    /// Moves selection down in track list.
    pub fn track_list_down(&mut self) {
        if self.ui_mode == UIMode::TrackList {
            let max_index = self.filtered_indices.len().saturating_sub(1);
            if self.track_list_selected < max_index {
                self.track_list_selected += 1;
                self.display_status();
            }
        }
    }

    /// Plays the selected track from track list.
    pub fn track_list_play_selected(&mut self) -> Result<(), PlayerError> {
        if self.ui_mode == UIMode::TrackList && self.track_list_selected < self.filtered_indices.len() {
            // Map filtered index to actual playlist index
            let actual_index = self.filtered_indices[self.track_list_selected];
            if self.playlist.goto(actual_index) {
                self.load_current_track()?;
                self.set_ui_mode(UIMode::Normal);
            }
        }
        Ok(())
    }

    /// Loads the current track from the playlist.
    fn load_current_track(&mut self) -> Result<(), PlayerError> {
        if let Some(track) = self.playlist.current_track() {
            match self.player.load_track(&track.path) {
                Ok(_) => {
                    self.player.play();
                    self.display_status();
                }
                Err(PlayerError::DecodeError(msg)) => {
                    // Log error to stderr (silent skip in UI)
                    eprintln!(
                        "Warning: Skipping unplayable track: {} ({})",
                        track.display_name(),
                        msg
                    );
                    // Skip to next track
                    if self.playlist.next() {
                        return self.load_current_track(); // Recursive retry
                    } else {
                        return Err(PlayerError::DecodeError(
                            "No playable tracks remaining".to_string(),
                        ));
                    }
                }
                Err(e) => return Err(e), // Propagate other errors
            }
        }
        Ok(())
    }

    /// Displays the current status using ratatui.
    fn display_status(&mut self) {
        let ui_mode = self.ui_mode;
        let search_query = self.search_query.clone();
        let current_index = self.playlist.current_index();
        let playlist_len = self.playlist.len();
        let shuffle_state = self.playlist.shuffle_state();
        let repeat_mode = self.playlist.repeat_mode();
        let seek_step = self.config.playback.seek_step;
        let track_list_selected = self.track_list_selected;

        let current_track = self.playlist.current_track().cloned();
        let pos = self.player.current_position();
        let dur = self.player.duration();
        let state = self.player.state();

        let tracks: Vec<_> = self.playlist.tracks().to_vec();
        let waveform_data = self.waveform_history.clone();
        let filtered_indices = self.filtered_indices.clone();

        if let Err(e) = self.terminal.draw(move |f| {
            let size = f.area();

            // Check minimum terminal size
            const MIN_WIDTH: u16 = 40;
            const MIN_HEIGHT: u16 = 10;

            if size.width < MIN_WIDTH || size.height < MIN_HEIGHT {
                render_size_warning(f, size, MIN_WIDTH, MIN_HEIGHT);
                return;
            }

            match ui_mode {
                UIMode::Normal => render_normal_view(
                    f, size, current_track.as_ref(), pos, dur, state,
                    current_index, playlist_len, shuffle_state, repeat_mode, seek_step,
                    &waveform_data
                ),
                UIMode::TrackList => render_track_list_view(
                    f, size, &tracks, current_index, track_list_selected, &search_query, &filtered_indices
                ),
                UIMode::Help => render_help_view(f, size, seek_step),
            }
        }) {
            eprintln!("Fatal: Failed to draw terminal: {}", e);
            self.running = false;
        }
    }
}

/// Truncates text to max width, adding ellipsis if needed.
fn truncate_text(text: &str, max_width: usize) -> String {
    if text.len() <= max_width {
        text.to_string()
    } else {
        format!("{}...", &text[..max_width.saturating_sub(3)])
    }
}

/// Truncates text based on available terminal width.
fn truncate_for_display(text: &str, area_width: u16, reserved: u16) -> String {
    let max_width = (area_width.saturating_sub(reserved)) as usize;
    truncate_text(text, max_width.max(10)) // Minimum 10 chars
}

/// Renders the normal playback view.
fn render_normal_view(
    f: &mut ratatui::Frame,
    size: ratatui::layout::Rect,
    current_track: Option<&crate::playlist::Track>,
    pos: Duration,
    dur: Duration,
    state: crate::player::PlaybackState,
    _current_index: usize,
    _playlist_len: usize,
    shuffle_state: crate::playlist::ShuffleState,
    repeat_mode: crate::playlist::RepeatMode,
    _seek_step: u32,
    waveform_data: &[f32],
) {
            // Single full-screen content area
            let mut content_lines = vec![];

            if let Some(track) = current_track {
                // Artist - Album (on one line)
                let artist_album = if let (Some(artist), Some(album)) = (&track.artist, &track.album) {
                    let artist_truncated = truncate_text(artist, 40);
                    let album_truncated = truncate_text(album, 40);
                    format!("  {} - {}", artist_truncated, album_truncated)
                } else if let Some(artist) = &track.artist {
                    format!("  {}", truncate_text(artist, 40))
                } else if let Some(album) = &track.album {
                    format!("  {}", truncate_text(album, 40))
                } else {
                    "".to_string()
                };
                if !artist_album.is_empty() {
                    content_lines.push(Line::from(artist_album));
                }

                // Track Title
                let display_name = truncate_for_display(&track.display_name(), size.width, 4);
                content_lines.push(Line::from(format!("  {}", display_name)));

                // Empty line
                content_lines.push(Line::from(""));

                // Waveform and Time
                let waveform_str = render_waveform(waveform_data);
                let time_str = format!(
                    "{:02}:{:02} / {:02}:{:02}",
                    pos.as_secs() / 60,
                    pos.as_secs() % 60,
                    dur.as_secs() / 60,
                    dur.as_secs() % 60
                );
                content_lines.push(Line::from(vec![
                    Span::styled(format!("  {}   ", waveform_str), Style::default().fg(Color::Cyan)),
                    Span::raw(time_str),
                ]));

                // Empty line
                content_lines.push(Line::from(""));

                // Progress bar using braille characters
                let progress_bar = render_progress_bar(pos, dur, size.width.saturating_sub(5) as usize);
                content_lines.push(Line::from(format!("  {} ", progress_bar)));

                // Empty line
                content_lines.push(Line::from(""));

                // Status line: [▶ Playing]  [⤮ Shuffle]  [↻ Repeat]  ? Help
                let state_text = match state {
                    crate::player::PlaybackState::Playing => "▶ Playing",
                    crate::player::PlaybackState::Paused => "⏸ Paused",
                    crate::player::PlaybackState::Stopped => "⏹ Stopped",
                };

                let shuffle_text = match shuffle_state {
                    crate::playlist::ShuffleState::Off => "Shuffle: Off",
                    crate::playlist::ShuffleState::On => "⤮ Shuffle",
                };

                let repeat_text = match repeat_mode {
                    crate::playlist::RepeatMode::Off => "Repeat: Off",
                    crate::playlist::RepeatMode::All => "↻ All",
                    crate::playlist::RepeatMode::Single => "↻ Single",
                };

                content_lines.push(Line::from(vec![
                    Span::raw("  ["),
                    Span::styled(state_text, Style::default().fg(Color::Green)),
                    Span::raw("]  ["),
                    Span::styled(shuffle_text, Style::default().fg(Color::Yellow)),
                    Span::raw("]  ["),
                    Span::styled(repeat_text, Style::default().fg(Color::Magenta)),
                    Span::raw("]  "),
                    Span::styled("? Help", Style::default().fg(Color::Cyan)),
                ]));
            } else {
                content_lines.push(Line::from("  No track loaded"));
            }

            let content = Paragraph::new(content_lines)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(content, size);
}

/// Renders the track list overlay view.
fn render_track_list_view(
    f: &mut ratatui::Frame,
    size: ratatui::layout::Rect,
    tracks: &[crate::playlist::Track],
    current_index: usize,
    selected_index: usize,
    search_query: &str,
    filtered_indices: &[usize],
) {
        // Create layout for track list
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header with search
                Constraint::Min(0),     // Track list
                Constraint::Length(2),  // Footer
            ])
            .split(size);

        // Header with search bar
        let search_text = if search_query.is_empty() {
            "Track List - Start typing to search...".to_string()
        } else {
            format!("Search: {}_", search_query)
        };
        let header = Paragraph::new(search_text)
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Left);
        f.render_widget(header, chunks[0]);

        // Track list
        let mut track_lines = vec![];

        for (filtered_idx, &actual_idx) in filtered_indices.iter().enumerate() {
            if actual_idx >= tracks.len() {
                continue;
            }

            let track = &tracks[actual_idx];
            let prefix = if actual_idx == current_index { "▶ " } else { "  " };
            let track_num = format!("{:3}. ", actual_idx + 1);

            let mut line_spans = vec![Span::raw(prefix), Span::raw(track_num)];

            // Truncate track name based on available width (reserve 25 chars for prefix, number, duration)
            let display_name = truncate_for_display(&track.display_name(), size.width, 25);

            // Determine styling based on whether this is the selected or currently playing track
            let style = if filtered_idx == selected_index {
                // Selected track - highlighted with reverse colors
                Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)
            } else if actual_idx == current_index {
                // Currently playing track - yellow and bold
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                // Regular track
                Style::default()
            };

            line_spans.push(Span::styled(display_name, style));

            if let Some(duration) = &track.duration {
                let duration_str = format!(
                    "  [{:02}:{:02}]",
                    duration.as_secs() / 60,
                    duration.as_secs() % 60
                );
                line_spans.push(Span::styled(
                    duration_str,
                    Style::default().fg(Color::DarkGray),
                ));
            }

            track_lines.push(Line::from(line_spans));
        }

        if track_lines.is_empty() {
            track_lines.push(Line::from("  No tracks match your search"));
        }

        let track_list = Paragraph::new(track_lines)
            .block(Block::default().borders(Borders::ALL).title("Tracks"));
        f.render_widget(track_list, chunks[1]);

        // Footer
        let footer = Paragraph::new("Esc: Back | Enter: Play selected | Type to search")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::NONE))
            .alignment(Alignment::Center);
        f.render_widget(footer, chunks[2]);
}

/// Renders the help overlay view.
fn render_help_view(f: &mut ratatui::Frame, size: ratatui::layout::Rect, seek_step: u32) {
        // Create centered help box
        let help_area = {
            let vertical = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(10),
                    Constraint::Percentage(80),
                    Constraint::Percentage(10),
                ])
                .split(size);

            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(20),
                    Constraint::Percentage(60),
                    Constraint::Percentage(20),
                ])
                .split(vertical[1])[1]
        };

        let help_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "juke - Keybindings",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Space      ", Style::default().fg(Color::Yellow)),
                Span::raw("Play / Pause"),
            ]),
            Line::from(vec![
                Span::styled("  n / →      ", Style::default().fg(Color::Yellow)),
                Span::raw("Next track"),
            ]),
            Line::from(vec![
                Span::styled("  p / ←      ", Style::default().fg(Color::Yellow)),
                Span::raw("Previous track"),
            ]),
            Line::from(vec![
                Span::styled("  Shift+→    ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("Seek forward {}s", seek_step)),
            ]),
            Line::from(vec![
                Span::styled("  Shift+←    ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("Seek backward {}s", seek_step)),
            ]),
            Line::from(vec![
                Span::styled("  s          ", Style::default().fg(Color::Yellow)),
                Span::raw("Toggle shuffle"),
            ]),
            Line::from(vec![
                Span::styled("  r          ", Style::default().fg(Color::Yellow)),
                Span::raw("Cycle repeat mode"),
            ]),
            Line::from(vec![
                Span::styled("  t          ", Style::default().fg(Color::Yellow)),
                Span::raw("Toggle track list"),
            ]),
            Line::from(vec![
                Span::styled("  ?          ", Style::default().fg(Color::Yellow)),
                Span::raw("Toggle help (this screen)"),
            ]),
            Line::from(vec![
                Span::styled("  Esc / q    ", Style::default().fg(Color::Yellow)),
                Span::raw("Quit"),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Press any key to close",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let help = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .alignment(Alignment::Left);
        f.render_widget(help, help_area);
}

/// Renders a warning when terminal is too small.
fn render_size_warning(
    f: &mut ratatui::Frame,
    size: ratatui::layout::Rect,
    min_width: u16,
    min_height: u16,
) {
    use ratatui::widgets::Wrap;

    let message = format!(
        "Terminal too small!\n\nMinimum: {}x{}\nCurrent: {}x{}\n\nPlease resize terminal to continue.",
        min_width, min_height, size.width, size.height
    );

    let paragraph = Paragraph::new(message)
        .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center);

    // Create centered block
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(7),
            Constraint::Percentage(40),
        ])
        .split(size);

    f.render_widget(paragraph, vertical[1]);
}

/// Renders a progress bar using braille characters.
fn render_progress_bar(pos: Duration, dur: Duration, width: usize) -> String {
    if width == 0 || dur.as_secs() == 0 {
        return String::new();
    }

    let progress = (pos.as_secs_f64() / dur.as_secs_f64()).min(1.0);
    let filled_width = (progress * width as f64).round() as usize;

    let filled = "⣿".repeat(filled_width);
    let empty = "⣀".repeat(width.saturating_sub(filled_width));

    format!("{}{}", filled, empty)
}

/// Renders bar visualizer data as a string of block characters.
fn render_waveform(data: &[f32]) -> String {
    // Use block characters to represent amplitude levels
    // Characters from lowest to highest: ▁▂▃▄▅▆▇█
    const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    // Each bar is rendered with a block character based on its amplitude
    data.iter()
        .map(|&amplitude| {
            // Map amplitude (0.0-1.0) to character index (0-7)
            let index = (amplitude * 7.0).round() as usize;
            LEVELS[index.min(7)]
        })
        .collect()
}
