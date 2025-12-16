//! Playlist management, track metadata, M3U parsing, and directory scanning.

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A single track in the playlist.
#[derive(Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,
}

impl Track {
    /// Creates a new track from a path with no metadata.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            title: None,
            artist: None,
            album: None,
            duration: None,
        }
    }

    /// Returns a display name for the track (title or filename).
    pub fn display_name(&self) -> String {
        self.title
            .clone()
            .unwrap_or_else(|| self.path.file_name().unwrap_or_default().to_string_lossy().to_string())
    }
}

/// Shuffle state for the playlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShuffleState {
    Off,
    On,
}

impl ShuffleState {
    /// Toggles shuffle state.
    pub fn toggle(&mut self) {
        *self = match self {
            ShuffleState::Off => ShuffleState::On,
            ShuffleState::On => ShuffleState::Off,
        };
    }
}

/// Repeat mode for the playlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    Single,
}

impl RepeatMode {
    /// Cycles to the next repeat mode: Off → All → Single → Off.
    pub fn cycle(&mut self) {
        *self = match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::Single,
            RepeatMode::Single => RepeatMode::Off,
        };
    }
}

/// A playlist containing tracks with shuffle and repeat support.
pub struct Playlist {
    tracks: Vec<Track>,
    current_index: usize,
    shuffle: ShuffleState,
    shuffle_indices: Vec<usize>,
    repeat: RepeatMode,
}

impl Playlist {
    /// Creates an empty playlist.
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            current_index: 0,
            shuffle: ShuffleState::Off,
            shuffle_indices: Vec::new(),
            repeat: RepeatMode::Off,
        }
    }

    /// Creates a playlist from a directory by scanning for audio files.
    pub fn from_directory<P: AsRef<Path>>(path: P) -> Result<Self, PlaylistError> {
        let tracks = scan_directory(path)?;
        if tracks.is_empty() {
            return Err(PlaylistError::EmptyPlaylist);
        }

        Ok(Self {
            tracks,
            current_index: 0,
            shuffle: ShuffleState::Off,
            shuffle_indices: Vec::new(),
            repeat: RepeatMode::Off,
        })
    }

    /// Loads a playlist from an M3U file.
    pub fn from_m3u<P: AsRef<Path>>(path: P) -> Result<Self, PlaylistError> {
        let tracks = parse_m3u(path)?;
        if tracks.is_empty() {
            return Err(PlaylistError::EmptyPlaylist);
        }

        Ok(Self {
            tracks,
            current_index: 0,
            shuffle: ShuffleState::Off,
            shuffle_indices: Vec::new(),
            repeat: RepeatMode::Off,
        })
    }

    /// Saves the playlist to an M3U file.
    pub fn save_m3u<P: AsRef<Path>>(&self, path: P) -> Result<(), PlaylistError> {
        write_m3u(&self.tracks, path)
    }

    /// Adds a track to the playlist.
    pub fn add_track(&mut self, track: Track) {
        self.tracks.push(track);
        if self.shuffle == ShuffleState::On {
            self.regenerate_shuffle();
        }
    }

    /// Returns the current track, if any.
    pub fn current_track(&self) -> Option<&Track> {
        if self.tracks.is_empty() {
            return None;
        }

        let index = self.get_actual_index(self.current_index);
        self.tracks.get(index)
    }

    /// Returns the current track index.
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Moves to the next track, respecting repeat mode.
    /// Returns true if successful, false if at end with no repeat.
    pub fn next(&mut self) -> bool {
        if self.tracks.is_empty() {
            return false;
        }

        match self.repeat {
            RepeatMode::Single => true, // Stay on current track
            RepeatMode::All => {
                self.current_index = (self.current_index + 1) % self.len();
                true
            }
            RepeatMode::Off => {
                if self.current_index + 1 < self.len() {
                    self.current_index += 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Moves to the previous track.
    /// Returns true if successful, false if at beginning.
    pub fn previous(&mut self) -> bool {
        if self.tracks.is_empty() {
            return false;
        }

        if self.current_index > 0 {
            self.current_index -= 1;
            true
        } else if self.repeat == RepeatMode::All {
            self.current_index = self.len() - 1;
            true
        } else {
            false
        }
    }

    /// Jumps to a specific track index.
    pub fn goto(&mut self, index: usize) -> bool {
        if index < self.len() {
            self.current_index = index;
            true
        } else {
            false
        }
    }

    /// Toggles shuffle mode.
    pub fn toggle_shuffle(&mut self) {
        self.shuffle.toggle();
        if self.shuffle == ShuffleState::On {
            self.regenerate_shuffle();
        }
    }

    /// Cycles the repeat mode.
    pub fn cycle_repeat(&mut self) {
        self.repeat.cycle();
    }

    /// Returns the current shuffle state.
    pub fn shuffle_state(&self) -> ShuffleState {
        self.shuffle
    }

    /// Returns the current repeat mode.
    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat
    }

    /// Returns the number of tracks in the playlist.
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    /// Returns whether the playlist is empty.
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    /// Returns all tracks in the playlist.
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Regenerates shuffle indices using Fisher-Yates algorithm.
    fn regenerate_shuffle(&mut self) {
        use rand::seq::SliceRandom;
        use rand::thread_rng;

        let mut indices: Vec<usize> = (0..self.tracks.len()).collect();
        indices.shuffle(&mut thread_rng());
        self.shuffle_indices = indices;

        // Ensure current track stays current after shuffle
        if let Some(pos) = self.shuffle_indices.iter().position(|&i| i == self.current_index) {
            self.shuffle_indices.swap(0, pos);
        }
        self.current_index = 0;
    }

    /// Gets the actual track index, accounting for shuffle.
    fn get_actual_index(&self, index: usize) -> usize {
        if self.shuffle == ShuffleState::On && index < self.shuffle_indices.len() {
            self.shuffle_indices[index]
        } else {
            index
        }
    }
}

impl Default for Playlist {
    fn default() -> Self {
        Self::new()
    }
}

/// Scans a directory recursively for audio files.
fn scan_directory<P: AsRef<Path>>(path: P) -> Result<Vec<Track>, PlaylistError> {
    let path = path.as_ref();
    let mut tracks = Vec::new();

    fn scan_recursive(dir: &Path, tracks: &mut Vec<Track>) -> std::io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    scan_recursive(&path, tracks)?;
                } else if is_audio_file(&path) {
                    tracks.push(extract_metadata(&path));
                }
            }
        }
        Ok(())
    }

    scan_recursive(path, &mut tracks)
        .map_err(|e| PlaylistError::IoError(e.to_string()))?;

    // Sort alphabetically by path
    tracks.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(tracks)
}

/// Checks if a file is an audio file based on extension.
fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_lowercase().as_str(), "mp3" | "flac" | "ogg"))
        .unwrap_or(false)
}

/// Extracts metadata from an audio file using lofty.
fn extract_metadata(path: &Path) -> Track {
    let mut track = Track::new(path.to_path_buf());

    // Try to read metadata, but don't fail if we can't
    if let Ok(tagged_file) = lofty::read_from_path(path) {
        let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

        if let Some(tag) = tag {
            // Extract title
            track.title = tag.title().map(|s| s.to_string());

            // Extract artist
            track.artist = tag.artist().map(|s| s.to_string());

            // Extract album
            track.album = tag.album().map(|s| s.to_string());
        }

        // Extract duration from properties
        let duration = tagged_file.properties().duration();
        if !duration.is_zero() {
            track.duration = Some(duration);
        }
    }

    track
}

/// Parses an M3U playlist file.
fn parse_m3u<P: AsRef<Path>>(path: P) -> Result<Vec<Track>, PlaylistError> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|e| PlaylistError::IoError(e.to_string()))?;
    let reader = BufReader::new(file);

    let mut tracks = Vec::new();
    let mut current_extinf: Option<(Duration, String)> = None;
    let playlist_dir = path.parent().unwrap_or_else(|| Path::new("."));

    for line in reader.lines() {
        let line = line.map_err(|e| PlaylistError::IoError(e.to_string()))?;
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if line.starts_with("#EXTINF:") {
            // Parse #EXTINF:duration,title
            if let Some(content) = line.strip_prefix("#EXTINF:") {
                if let Some((duration_str, title)) = content.split_once(',') {
                    let duration = duration_str
                        .parse::<i64>()
                        .ok()
                        .map(|secs| Duration::from_secs(secs.max(0) as u64));

                    current_extinf = duration.map(|d| (d, title.to_string()));
                }
            }
        } else if !line.starts_with('#') {
            // This is a file path
            let track_path = if Path::new(line).is_absolute() {
                PathBuf::from(line)
            } else {
                playlist_dir.join(line)
            };

            // Extract metadata from the file
            let mut track = extract_metadata(&track_path);

            // Apply or override with EXTINF metadata if present
            if let Some((duration, title)) = current_extinf.take() {
                track.duration = Some(duration);
                track.title = Some(title);
            }

            tracks.push(track);
        }
    }

    Ok(tracks)
}

/// Writes tracks to an M3U playlist file.
fn write_m3u<P: AsRef<Path>>(tracks: &[Track], path: P) -> Result<(), PlaylistError> {
    let mut file = File::create(path).map_err(|e| PlaylistError::IoError(e.to_string()))?;

    writeln!(file, "#EXTM3U").map_err(|e| PlaylistError::IoError(e.to_string()))?;

    for track in tracks {
        let duration_secs = track.duration.map(|d| d.as_secs() as i64).unwrap_or(-1);
        let title = track.title.as_deref().unwrap_or_else(|| {
            track
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
        });

        writeln!(file, "#EXTINF:{},{}", duration_secs, title)
            .map_err(|e| PlaylistError::IoError(e.to_string()))?;

        writeln!(file, "{}", track.path.display())
            .map_err(|e| PlaylistError::IoError(e.to_string()))?;
    }

    Ok(())
}

/// Errors that can occur during playlist operations.
#[derive(Debug)]
pub enum PlaylistError {
    IoError(String),
    EmptyPlaylist,
}

impl std::fmt::Display for PlaylistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaylistError::IoError(msg) => write!(f, "IO error: {}", msg),
            PlaylistError::EmptyPlaylist => write!(f, "Playlist is empty"),
        }
    }
}

impl std::error::Error for PlaylistError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_display_name() {
        let mut track = Track::new(PathBuf::from("/music/song.mp3"));
        assert_eq!(track.display_name(), "song.mp3");

        track.title = Some("My Song".to_string());
        assert_eq!(track.display_name(), "My Song");
    }

    #[test]
    fn test_shuffle_state_toggle() {
        let mut state = ShuffleState::Off;
        state.toggle();
        assert_eq!(state, ShuffleState::On);
        state.toggle();
        assert_eq!(state, ShuffleState::Off);
    }

    #[test]
    fn test_repeat_mode_cycle() {
        let mut mode = RepeatMode::Off;
        mode.cycle();
        assert_eq!(mode, RepeatMode::All);
        mode.cycle();
        assert_eq!(mode, RepeatMode::Single);
        mode.cycle();
        assert_eq!(mode, RepeatMode::Off);
    }

    #[test]
    fn test_empty_playlist() {
        let playlist = Playlist::new();
        assert!(playlist.is_empty());
        assert_eq!(playlist.len(), 0);
        assert!(playlist.current_track().is_none());
    }

    #[test]
    fn test_playlist_navigation() {
        let mut playlist = Playlist::new();
        playlist.add_track(Track::new(PathBuf::from("track1.mp3")));
        playlist.add_track(Track::new(PathBuf::from("track2.mp3")));
        playlist.add_track(Track::new(PathBuf::from("track3.mp3")));

        assert_eq!(playlist.current_index(), 0);

        // Next without repeat
        assert!(playlist.next());
        assert_eq!(playlist.current_index(), 1);

        assert!(playlist.next());
        assert_eq!(playlist.current_index(), 2);

        // At end, should not advance
        assert!(!playlist.next());
        assert_eq!(playlist.current_index(), 2);

        // Previous
        assert!(playlist.previous());
        assert_eq!(playlist.current_index(), 1);
    }

    #[test]
    fn test_playlist_repeat_all() {
        let mut playlist = Playlist::new();
        playlist.add_track(Track::new(PathBuf::from("track1.mp3")));
        playlist.add_track(Track::new(PathBuf::from("track2.mp3")));

        playlist.cycle_repeat();
        assert_eq!(playlist.repeat_mode(), RepeatMode::All);

        // Navigate to end
        playlist.goto(1);
        assert_eq!(playlist.current_index(), 1);

        // Should wrap to beginning
        assert!(playlist.next());
        assert_eq!(playlist.current_index(), 0);
    }

    #[test]
    fn test_is_audio_file() {
        assert!(is_audio_file(Path::new("song.mp3")));
        assert!(is_audio_file(Path::new("song.MP3")));
        assert!(is_audio_file(Path::new("song.flac")));
        assert!(is_audio_file(Path::new("song.ogg")));
        assert!(!is_audio_file(Path::new("song.txt")));
        assert!(!is_audio_file(Path::new("song.wav")));
    }

    #[test]
    fn test_metadata_extraction() {
        // Test with a non-existent file - should not panic
        let track = extract_metadata(Path::new("nonexistent.mp3"));
        assert!(track.title.is_none());
        assert!(track.artist.is_none());
        assert!(track.album.is_none());
    }

    #[test]
    fn test_load_music_directory() {
        // Test loading the actual music directory if it exists
        let music_dir = Path::new("music");
        if music_dir.exists() && music_dir.is_dir() {
            let result = Playlist::from_directory(music_dir);
            match result {
                Ok(playlist) => {
                    println!("\nLoaded {} tracks from music directory", playlist.len());
                    for track in playlist.tracks() {
                        println!("  - {}", track.display_name());
                        if let Some(artist) = &track.artist {
                            println!("    Artist: {}", artist);
                        }
                        if let Some(album) = &track.album {
                            println!("    Album: {}", album);
                        }
                        if let Some(duration) = &track.duration {
                            println!("    Duration: {}:{:02}",
                                duration.as_secs() / 60,
                                duration.as_secs() % 60);
                        }
                    }
                }
                Err(e) => {
                    println!("Note: Could not load music directory: {}", e);
                }
            }
        } else {
            println!("Skipping music directory test - directory not found");
        }
    }
}
