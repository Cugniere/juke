//! Configuration file loading and management.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Main configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub playback: PlaybackConfig,
    pub keys: KeyConfig,
}

/// Playback configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackConfig {
    /// Seek step in seconds.
    pub seek_step: u32,
}

/// Keybinding configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyConfig {
    pub play_pause: KeyBinding,
    pub next: KeyBinding,
    pub prev: KeyBinding,
    pub seek_forward: KeyBinding,
    pub seek_back: KeyBinding,
    pub shuffle: KeyBinding,
    pub repeat: KeyBinding,
    pub track_list: KeyBinding,
    pub search: KeyBinding,
    pub help: KeyBinding,
    pub quit: KeyBinding,
}

/// A keybinding can be a single key or multiple keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyBinding {
    Single(String),
    Multiple(Vec<String>),
}

impl KeyBinding {
    /// Returns all key strings for this binding.
    #[allow(dead_code)]
    pub fn keys(&self) -> Vec<&str> {
        match self {
            KeyBinding::Single(key) => vec![key.as_str()],
            KeyBinding::Multiple(keys) => keys.iter().map(|s| s.as_str()).collect(),
        }
    }

    /// Checks if the binding contains the given key.
    #[allow(dead_code)]
    pub fn contains(&self, key: &str) -> bool {
        self.keys().contains(&key)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            playback: PlaybackConfig::default(),
            keys: KeyConfig::default(),
        }
    }
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self { seek_step: 10 }
    }
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            play_pause: KeyBinding::Single("Space".to_string()),
            next: KeyBinding::Multiple(vec!["n".to_string(), "Right".to_string()]),
            prev: KeyBinding::Multiple(vec!["p".to_string(), "Left".to_string()]),
            seek_forward: KeyBinding::Single("Shift+Right".to_string()),
            seek_back: KeyBinding::Single("Shift+Left".to_string()),
            shuffle: KeyBinding::Single("S".to_string()),
            repeat: KeyBinding::Single("r".to_string()),
            track_list: KeyBinding::Single("t".to_string()),
            search: KeyBinding::Single("/".to_string()),
            help: KeyBinding::Multiple(vec!["?".to_string(), "h".to_string()]),
            quit: KeyBinding::Multiple(vec!["q".to_string(), "Esc".to_string()]),
        }
    }
}

impl Config {
    /// Returns the path to the config file based on XDG Base Directory spec.
    ///
    /// - Linux: `~/.config/juke/config.toml`
    /// - macOS: `~/Library/Application Support/juke/config.toml`
    /// - Windows: `%APPDATA%\juke\config.toml`
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|mut path| {
            path.push("juke");
            path.push("config.toml");
            path
        })
    }

    /// Loads the configuration from the config file.
    ///
    /// If the file doesn't exist, creates it with default values.
    /// If the file is invalid, returns the default config and prints a warning.
    pub fn load() -> Self {
        let path = match Self::config_path() {
            Some(p) => p,
            None => {
                eprintln!("Warning: Could not determine config directory, using defaults");
                return Self::default();
            }
        };

        // If config doesn't exist, create it with defaults
        if !path.exists() {
            let config = Self::default();
            if let Err(e) = config.save(&path) {
                eprintln!("Warning: Could not create default config file: {}", e);
            }
            return config;
        }

        // Load and parse config
        match fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str::<Config>(&contents) {
                Ok(mut config) => {
                    config.validate();
                    config
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Could not parse config file at {:?}: {}",
                        path, e
                    );
                    eprintln!("Using default configuration");
                    Self::default()
                }
            },
            Err(e) => {
                eprintln!("Warning: Could not read config file at {:?}: {}", path, e);
                eprintln!("Using default configuration");
                Self::default()
            }
        }
    }

    /// Saves the configuration to the specified path.
    fn save(&self, path: &PathBuf) -> std::io::Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let toml_string = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        fs::write(path, toml_string)
    }

    /// Validates configuration values and applies constraints.
    fn validate(&mut self) {
        // Ensure seek_step is at least 1 second
        if self.playback.seek_step == 0 {
            eprintln!("Warning: seek_step must be at least 1, using default value of 10");
            self.playback.seek_step = 10;
        }

        // Could add more validation here:
        // - Check for duplicate keybindings
        // - Validate key string formats
        // - etc.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.playback.seek_step, 10);
        assert!(config.keys.play_pause.contains("Space"));
        assert!(config.keys.next.contains("n"));
        assert!(config.keys.next.contains("Right"));
    }

    #[test]
    fn test_keybinding_contains() {
        let single = KeyBinding::Single("Space".to_string());
        assert!(single.contains("Space"));
        assert!(!single.contains("Enter"));

        let multiple = KeyBinding::Multiple(vec!["n".to_string(), "Right".to_string()]);
        assert!(multiple.contains("n"));
        assert!(multiple.contains("Right"));
        assert!(!multiple.contains("Left"));
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();

        // Should be able to deserialize back
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.playback.seek_step, config.playback.seek_step);
    }

    #[test]
    fn test_validation() {
        let mut config = Config::default();
        config.playback.seek_step = 0;
        config.validate();
        assert_eq!(config.playback.seek_step, 10);
    }
}
