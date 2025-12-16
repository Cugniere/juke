mod app;
mod config;
mod input;
mod player;
mod playlist;
mod ui;

fn main() {
    // Load configuration
    let config = config::Config::load();

    println!("juke - minimalist terminal music player");
    println!("Configuration loaded successfully!");
    println!("Seek step: {} seconds", config.playback.seek_step);

    if let Some(path) = config::Config::config_path() {
        println!("Config file: {}", path.display());
    }
}
