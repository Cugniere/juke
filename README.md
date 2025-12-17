# juke

A minimalist terminal music player written in Rust. Fast, lightweight, and distraction-free. Just your music and the command line.

## Installation

```bash
cargo install juke
```

Or build from source:

```bash
git clone https://github.com/cugniere/juke.git
cd juke
cargo build --release
```

## Usage

Play audio files from a directory:

```bash
juke /path/to/music
```

Play an M3U playlist:

```bash
juke playlist.m3u
```

If no path is given, juke plays files from the current directory.

## Features

- Supports MP3, FLAC, and OGG Vorbis
- M3U playlist support
- Shuffle and repeat modes
- Search and navigation through tracks
- Configurable keyboard shortcuts
- Live audio visualizer

## Controls

| Key | Action |
|-----|--------|
| Space | Play/pause |
| n, Right | Next track |
| p, Left | Previous track |
| Shift+Right/Left | Seek forward/backward |
| s | Toggle shuffle |
| r | Cycle repeat mode |
| t | Show track list |
| Up/Down (in track list) | Navigate tracks |
| Enter (in track list) | Play selected track |
| Type to search (in track list) | Filter tracks |
| ? | Show help |
| q, Esc | Quit |

## Configuration

On first run, juke creates a config file:

- Linux: `~/.config/juke/config.toml`
- macOS: `~/Library/Application Support/juke/config.toml`
- Windows: `%APPDATA%\juke\config.toml`

Example configuration:

```toml
[playback]
seek_step = 10  # seconds

[keys]
play_pause = "Space"
next = ["n", "Right"]
prev = ["p", "Left"]
seek_forward = "Shift+Right"
seek_back = "Shift+Left"
shuffle = "s"
repeat = "r"
track_list = "t"
help = ["?", "h"]
quit = ["q", "Esc"]
```

## System Requirements

**Linux:**
```bash
sudo apt install pkg-config libasound2-dev
```

**macOS and Windows:**
No additional dependencies required.

## License

ISC
