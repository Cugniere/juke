mod app;
mod config;
mod input;
mod player;
mod playlist;
mod ui;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::env;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = config::Config::load();

    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();

    let playlist = if args.len() > 1 {
        let path = &args[1];
        load_playlist(path)?
    } else {
        // Default to current directory
        load_playlist(".")?
    };

    if playlist.is_empty() {
        eprintln!("Error: No audio files found");
        eprintln!("Usage: {} [directory or playlist.m3u]", args.get(0).unwrap_or(&"juke".to_string()));
        std::process::exit(1);
    }

    // Setup terminal
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    // Setup signal handler for graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    // Setup panic hook to restore terminal state
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = cleanup_terminal();
        original_hook(panic_info);
    }));

    // Create and start the app (ensure cleanup on error)
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut app = app::App::new(playlist, config)?;
        app.start()?;

        // Main loop
        run_main_loop(&mut app, running)?;

        // Stop audio playback
        app.stop_playback();

        Ok(())
    })();

    // Cleanup - restore terminal state (always runs)
    cleanup_terminal()?;

    result
}

/// Cleans up terminal state before exit.
fn cleanup_terminal() -> Result<(), Box<dyn std::error::Error>> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    println!("Thanks for using juke!");
    Ok(())
}

/// Main application loop.
fn run_main_loop(
    app: &mut app::App,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    while app.is_running() && running.load(Ordering::SeqCst) {
        // Handle input
        input::handle_input(app)?;

        // Update app state (check for track end, update display)
        app.update()?;

        // Small sleep to avoid busy loop
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(())
}

/// Loads a playlist from a path (directory or M3U file).
fn load_playlist(path: &str) -> Result<playlist::Playlist, Box<dyn std::error::Error>> {
    let path = Path::new(path);

    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("m3u") {
        // Load M3U file
        Ok(playlist::Playlist::from_m3u(path)?)
    } else if path.is_dir() {
        // Scan directory
        Ok(playlist::Playlist::from_directory(path)?)
    } else {
        Err("Path must be a directory or .m3u file".into())
    }
}
