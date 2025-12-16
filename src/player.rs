//! Audio playback engine.

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Current playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
}

/// Audio player with playback control.
pub struct Player {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    sink: Option<Sink>,
    state: PlaybackState,
    current_path: Option<String>,
    current_duration: Duration,
    // Track elapsed time manually since rodio doesn't provide easy seeking
    playback_start: Option<std::time::Instant>,
    elapsed_before_pause: Duration,
    amplitude: Arc<Mutex<f32>>,
}

impl Player {
    /// Creates a new player with initialized audio output.
    pub fn new() -> Result<Self, PlayerError> {
        let (stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| PlayerError::InitializationError(e.to_string()))?;

        Ok(Self {
            _stream: stream,
            stream_handle,
            sink: None,
            state: PlaybackState::Stopped,
            current_path: None,
            current_duration: Duration::ZERO,
            playback_start: None,
            elapsed_before_pause: Duration::ZERO,
            amplitude: Arc::new(Mutex::new(0.0)),
        })
    }

    /// Loads and plays a track from the given path.
    pub fn load_track<P: AsRef<Path>>(&mut self, path: P) -> Result<(), PlayerError> {
        let path = path.as_ref();

        // Open the file
        let file = File::open(path)
            .map_err(|e| PlayerError::FileError(format!("Failed to open file: {}", e)))?;

        let buf_reader = BufReader::new(file);

        // Decode the audio file
        let source = Decoder::new(buf_reader)
            .map_err(|e| PlayerError::DecodeError(format!("Failed to decode audio: {}", e)))?;

        // Get duration if available
        let duration = source.total_duration().unwrap_or(Duration::ZERO);

        // Create a new sink
        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| PlayerError::InitializationError(e.to_string()))?;

        // Append the source to the sink
        sink.append(source);

        // Start paused - user must explicitly play
        sink.pause();

        // Update state
        self.sink = Some(sink);
        self.current_path = Some(path.to_string_lossy().to_string());
        self.current_duration = duration;
        self.state = PlaybackState::Paused;
        self.playback_start = None;
        self.elapsed_before_pause = Duration::ZERO;

        Ok(())
    }

    /// Toggles between play and pause.
    pub fn toggle_play_pause(&mut self) {
        match self.state {
            PlaybackState::Playing => self.pause(),
            PlaybackState::Paused | PlaybackState::Stopped => self.play(),
        }
    }

    /// Starts or resumes playback.
    pub fn play(&mut self) {
        if let Some(sink) = &self.sink {
            if sink.is_paused() {
                sink.play();
                self.state = PlaybackState::Playing;
                self.playback_start = Some(std::time::Instant::now());
            }
        }
    }

    /// Pauses playback.
    pub fn pause(&mut self) {
        if let Some(sink) = &self.sink {
            if !sink.is_paused() {
                sink.pause();
                self.state = PlaybackState::Paused;

                // Update elapsed time
                if let Some(start) = self.playback_start {
                    self.elapsed_before_pause += start.elapsed();
                    self.playback_start = None;
                }
            }
        }
    }

    /// Stops playback and resets position.
    pub fn stop(&mut self) {
        self.sink = None;
        self.state = PlaybackState::Stopped;
        self.current_path = None;
        self.current_duration = Duration::ZERO;
        self.playback_start = None;
        self.elapsed_before_pause = Duration::ZERO;
    }

    /// Seeks forward by the specified duration.
    ///
    /// Note: rodio's Sink doesn't support true seeking, so this is implemented
    /// by reloading the track and skipping ahead. This is a limitation of the
    /// current implementation.
    pub fn seek_forward(&mut self, step: Duration) -> Result<(), PlayerError> {
        let current_pos = self.current_position();
        let new_pos = current_pos + step;

        if new_pos >= self.current_duration {
            // Can't seek beyond the end
            return Ok(());
        }

        self.seek_to(new_pos)
    }

    /// Seeks backward by the specified duration.
    pub fn seek_backward(&mut self, step: Duration) -> Result<(), PlayerError> {
        let current_pos = self.current_position();
        let new_pos = current_pos.saturating_sub(step);
        self.seek_to(new_pos)
    }

    /// Seeks to a specific position in the track.
    ///
    /// Note: This reloads the track and uses skip_duration, which is not perfect
    /// but works for basic seeking functionality.
    fn seek_to(&mut self, position: Duration) -> Result<(), PlayerError> {
        let path = match &self.current_path {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        let was_playing = self.state == PlaybackState::Playing;

        // Reload the track
        let file = File::open(&path)
            .map_err(|e| PlayerError::FileError(format!("Failed to open file: {}", e)))?;

        let buf_reader = BufReader::new(file);
        let source = Decoder::new(buf_reader)
            .map_err(|e| PlayerError::DecodeError(format!("Failed to decode audio: {}", e)))?;

        // Skip to the desired position
        let skipped_source = source.skip_duration(position);

        // Create new sink
        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| PlayerError::InitializationError(e.to_string()))?;

        sink.append(skipped_source);

        if !was_playing {
            sink.pause();
        }

        self.sink = Some(sink);
        self.elapsed_before_pause = position;
        self.playback_start = if was_playing {
            Some(std::time::Instant::now())
        } else {
            None
        };
        self.state = if was_playing {
            PlaybackState::Playing
        } else {
            PlaybackState::Paused
        };

        Ok(())
    }

    /// Returns the current playback position.
    pub fn current_position(&self) -> Duration {
        match self.playback_start {
            Some(start) => self.elapsed_before_pause + start.elapsed(),
            None => self.elapsed_before_pause,
        }
    }

    /// Returns the total duration of the current track.
    pub fn duration(&self) -> Duration {
        self.current_duration
    }

    /// Returns the current playback state.
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Returns whether a track is currently loaded.
    pub fn has_track(&self) -> bool {
        self.sink.is_some()
    }

    /// Returns whether the current track has finished playing.
    pub fn is_finished(&self) -> bool {
        self.sink.as_ref().map_or(true, |s| s.empty())
    }

    /// Sets the playback volume (0.0 to 1.0).
    pub fn set_volume(&mut self, volume: f32) {
        if let Some(sink) = &self.sink {
            sink.set_volume(volume.clamp(0.0, 1.0));
        }
    }

    /// Returns the current volume (0.0 to 1.0).
    pub fn volume(&self) -> f32 {
        self.sink.as_ref().map_or(1.0, |s| s.volume())
    }

    /// Returns the current amplitude for waveform display.
    ///
    /// This is a simplified implementation that returns a value based on
    /// playback state and volume. For true amplitude tracking, we would need
    /// to process the raw audio samples.
    pub fn amplitude(&self) -> f32 {
        if self.state == PlaybackState::Playing {
            // Simulate amplitude based on volume
            // In a real implementation, this would sample the actual audio buffer
            let base_amplitude = 0.5 + (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as f32
                * 0.01)
                .sin()
                .abs()
                * 0.5;
            base_amplitude * self.volume()
        } else {
            0.0
        }
    }

    /// Returns the path of the currently loaded track.
    pub fn current_path(&self) -> Option<&str> {
        self.current_path.as_deref()
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new().expect("Failed to initialize audio player")
    }
}

/// Errors that can occur during player operations.
#[derive(Debug)]
pub enum PlayerError {
    InitializationError(String),
    FileError(String),
    DecodeError(String),
}

impl std::fmt::Display for PlayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayerError::InitializationError(msg) => write!(f, "Initialization error: {}", msg),
            PlayerError::FileError(msg) => write!(f, "File error: {}", msg),
            PlayerError::DecodeError(msg) => write!(f, "Decode error: {}", msg),
        }
    }
}

impl std::error::Error for PlayerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_initialization() {
        let player = Player::new();
        assert!(player.is_ok());

        let player = player.unwrap();
        assert_eq!(player.state(), PlaybackState::Stopped);
        assert!(!player.has_track());
        assert_eq!(player.current_position(), Duration::ZERO);
    }

    #[test]
    fn test_playback_state_transitions() {
        let mut player = Player::new().unwrap();

        // Initially stopped
        assert_eq!(player.state(), PlaybackState::Stopped);

        // Play without track does nothing
        player.play();
        assert_eq!(player.state(), PlaybackState::Stopped);
    }

    #[test]
    fn test_amplitude_when_stopped() {
        let player = Player::new().unwrap();
        assert_eq!(player.amplitude(), 0.0);
    }

    #[test]
    fn test_volume_control() {
        let mut player = Player::new().unwrap();

        // Default volume should be 1.0
        assert_eq!(player.volume(), 1.0);

        // Volume should clamp to valid range
        player.set_volume(1.5);
        // Can't test exact value without a loaded track
    }
}
