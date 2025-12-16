//! Keyboard input handling and event processing.

use crate::app::App;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

/// Handles a single input event.
pub fn handle_input(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    // Poll for events with short timeout
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            handle_key_event(app, key)?;
        }
    }
    Ok(())
}

/// Handles a keyboard event.
fn handle_key_event(app: &mut App, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
    match key.code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Char(' ') => app.toggle_play_pause(),
        KeyCode::Char('n') => app.next_track()?,
        KeyCode::Char('p') => app.previous_track()?,
        KeyCode::Char('s') => app.toggle_shuffle(),
        KeyCode::Char('r') => app.cycle_repeat(),
        KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => app.seek_forward()?,
        KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => app.seek_backward()?,
        KeyCode::Right => app.next_track()?,
        KeyCode::Left => app.previous_track()?,
        KeyCode::Esc => app.quit(),
        _ => {}
    }
    Ok(())
}
