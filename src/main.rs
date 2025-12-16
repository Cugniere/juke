mod app;
mod config;
mod input;
mod player;
mod playlist;
mod ui;

use std::time::Duration;

fn main() {
    println!("=== juke - minimalist terminal music player ===\n");

    // Load configuration
    let config = config::Config::load();
    println!("[Config] Loaded successfully");
    println!("[Config] Seek step: {} seconds", config.playback.seek_step);
    if let Some(path) = config::Config::config_path() {
        println!("[Config] File: {}\n", path.display());
    }

    // Test player
    println!("--- Testing Audio Player ---");
    let mut player = match player::Player::new() {
        Ok(p) => {
            println!("[Player] Initialized successfully");
            p
        }
        Err(e) => {
            eprintln!("[Player] Error: {}", e);
            return;
        }
    };

    // Try to load a test track
    let test_track = "music/Drew_Redman__1000xResistOST__A_Teardrop.mp3";
    println!("[Player] Loading track: {}", test_track);

    match player.load_track(test_track) {
        Ok(()) => {
            println!("[Player] Track loaded successfully");
            println!("[Player] Duration: {:?}", player.duration());
            println!("[Player] State: {:?}", player.state());

            // Test playback controls
            println!("\n[Player] Testing playback controls...");
            player.play();
            println!("[Player] Playing... State: {:?}", player.state());

            std::thread::sleep(Duration::from_secs(2));
            println!("[Player] Position: {:?}", player.current_position());
            println!("[Player] Amplitude: {:.2}", player.amplitude());

            player.pause();
            println!("[Player] Paused. State: {:?}", player.state());

            // Test seeking
            println!("\n[Player] Testing seek...");
            let seek_step = Duration::from_secs(config.playback.seek_step as u64);
            if let Err(e) = player.seek_forward(seek_step) {
                eprintln!("[Player] Seek error: {}", e);
            } else {
                println!("[Player] Seeked forward {} seconds", config.playback.seek_step);
                println!("[Player] New position: {:?}", player.current_position());
            }

            player.stop();
            println!("[Player] Stopped. State: {:?}", player.state());
        }
        Err(e) => {
            eprintln!("[Player] Error loading track: {}", e);
        }
    }

    println!("\n=== All systems operational ===");
}
