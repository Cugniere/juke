//! Keyboard input handling and event processing.

use crate::app::{App, UIMode};
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
    match app.ui_mode() {
        UIMode::Normal => handle_normal_mode(app, key)?,
        UIMode::TrackList => handle_track_list_mode(app, key)?,
        UIMode::Help => handle_help_mode(app, key)?,
    }
    Ok(())
}

/// Handles keyboard events in normal mode.
fn handle_normal_mode(app: &mut App, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
    match key.code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Char(' ') => app.toggle_play_pause(),
        KeyCode::Char('n') => app.next_track()?,
        KeyCode::Char('p') => app.previous_track()?,
        KeyCode::Char('s') => app.toggle_shuffle(),
        KeyCode::Char('r') => app.cycle_repeat(),
        KeyCode::Char('t') => app.set_ui_mode(UIMode::TrackList),
        KeyCode::Char('?') => app.set_ui_mode(UIMode::Help),
        KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => app.seek_forward()?,
        KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => app.seek_backward()?,
        KeyCode::Right => app.next_track()?,
        KeyCode::Left => app.previous_track()?,
        KeyCode::Esc => app.quit(),
        _ => {}
    }
    Ok(())
}

/// Handles keyboard events in track list mode.
fn handle_track_list_mode(app: &mut App, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
    match key.code {
        KeyCode::Esc => app.set_ui_mode(UIMode::Normal),
        KeyCode::Up => app.track_list_up(),
        KeyCode::Down => app.track_list_down(),
        KeyCode::Enter => app.track_list_play_selected()?,
        KeyCode::Backspace => app.search_backspace(),
        KeyCode::Char(c) => app.search_input(c),
        _ => {}
    }
    Ok(())
}

/// Handles keyboard events in help mode.
fn handle_help_mode(app: &mut App, _key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
    // Any key closes help
    app.set_ui_mode(UIMode::Normal);
    Ok(())
}
