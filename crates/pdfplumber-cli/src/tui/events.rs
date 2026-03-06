//! Event handling — keyboard input translated into [`AppAction`].
//!
//! Each [`AppAction`] is a concrete intent (not a raw keypress) so that screen
//! rendering logic stays clean and key bindings can be changed in one place.

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

/// User intents derived from keyboard input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    /// Move cursor / selection up.
    Up,
    /// Move cursor / selection down.
    Down,
    /// Move to previous page (extract view) or previous field (config).
    Left,
    /// Move to next page (extract view) or next field (config).
    Right,
    /// Confirm / enter / select.
    Enter,
    /// Cancel / back / escape.
    Escape,
    /// Tab — cycle modes in extract, move fields in config.
    Tab,
    /// Type a printable character (grep search, config edit).
    Char(char),
    /// Backspace in text input.
    Backspace,
    /// Yank / copy to clipboard.
    Yank,
    /// Quit application.
    Quit,
    /// Slash — start new search in grep view.
    Slash,
    /// Save — in config view.
    Save,
    /// Terminal was resized.
    Resize(u16, u16),
    /// No action — used to flush the poll loop.
    Tick,
}

/// Poll for the next event, blocking up to `timeout`.
///
/// Returns `None` if the poll timed out with no event.
pub fn next_event(timeout: Duration) -> std::io::Result<Option<AppAction>> {
    if !event::poll(timeout)? {
        return Ok(Some(AppAction::Tick));
    }
    match event::read()? {
        Event::Key(KeyEvent { code, modifiers, .. }) => Ok(Some(key_to_action(code, modifiers))),
        Event::Resize(w, h) => Ok(Some(AppAction::Resize(w, h))),
        _ => Ok(Some(AppAction::Tick)),
    }
}

fn key_to_action(code: KeyCode, modifiers: KeyModifiers) -> AppAction {
    // Ctrl+C always quits
    if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
        return AppAction::Quit;
    }

    match code {
        KeyCode::Up | KeyCode::Char('k') => AppAction::Up,
        KeyCode::Down | KeyCode::Char('j') => AppAction::Down,
        KeyCode::Left | KeyCode::Char('h') => AppAction::Left,
        KeyCode::Right | KeyCode::Char('l') => AppAction::Right,
        KeyCode::Enter => AppAction::Enter,
        KeyCode::Esc => AppAction::Escape,
        KeyCode::Tab => AppAction::Tab,
        KeyCode::Backspace => AppAction::Backspace,
        KeyCode::Char('q') => AppAction::Quit,
        KeyCode::Char('y') => AppAction::Yank,
        KeyCode::Char('/') => AppAction::Slash,
        KeyCode::Char('s') => AppAction::Save,
        KeyCode::Char(c) => AppAction::Char(c),
        _ => AppAction::Tick,
    }
}
