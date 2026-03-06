//! Terminal initialisation, main render+event loop, cleanup.
//!
//! Entry point: [`run`].  Caller passes an initial [`App`] (already
//! configured with any startup file / directory) and this function owns the
//! terminal for its lifetime.
//!
//! # Terminal lifecycle
//!
//! ```text
//! enable_raw_mode()
//! EnterAlternateScreen
//!   loop {
//!     terminal.draw(render)
//!     next_event(16 ms) → dispatch → state mutation
//!     if app.should_quit() { break }
//!   }
//! LeaveAlternateScreen
//! disable_raw_mode()
//! ```
//!
//! Cleanup happens in the [`TerminalGuard`] drop impl so it runs even on
//! panic.
//!
//! # Render convention
//!
//! Each screen module exposes:
//! ```ignore
//! pub fn render(state: &ScreenState, area: Rect, buf: &mut Buffer)
//! ```
//! The event loop bridges this to ratatui's `Frame` API using
//! `frame.buffer_mut()` and `frame.area()`.

use std::io;
use std::panic;
use std::time::Duration;

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::{Terminal, backend::CrosstermBackend};

use super::app::{App, ConfirmState, Screen};
use super::events::next_event;
use super::input_handlers::dispatch;
use super::theme;

// ── render dispatch ───────────────────────────────────────────────────────────

fn render(app: &App, frame: &mut ratatui::Frame) {
    use super::{
        screen_config, screen_extract, screen_grep, screen_menu, screen_process, widgets,
    };
    use ratatui::layout::Rect;

    let area = frame.area();
    let buf = frame.buffer_mut();

    // Reserve the last row for the status bar (if there is a status message)
    // or leave the full area to the screen.
    let (screen_area, status_area) = if app.status.is_some() && area.height > 2 {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        (split[0], Some(split[1]))
    } else {
        (area, None)
    };

    // Render the active screen
    match &app.screen {
        Screen::Menu => {
            screen_menu::render(&app.menu, screen_area, buf);
        }
        Screen::Extract(st) => {
            screen_extract::render(st, screen_area, buf);
        }
        Screen::Grep(st) => {
            screen_grep::render(st, screen_area, buf);
        }
        Screen::Process(st) => {
            screen_process::render(st, screen_area, buf);
        }
        Screen::Config(st) => {
            screen_config::render(st, screen_area, buf);
        }
        Screen::Confirm(st) => {
            render_confirm(st, screen_area, buf);
        }
        Screen::Quit => {}
    }

    // Render transient status bar
    if let (Some(msg), Some(sa)) = (&app.status, status_area) {
        let style = if msg.starts_with("Error") || msg.starts_with("Save failed") {
            theme::warn()
        } else if msg.starts_with("Config saved") || msg.starts_with("Copied") {
            theme::ok()
        } else {
            theme::muted()
        };
        widgets::render_status(buf, sa, msg, style);
    }
}

fn render_confirm(st: &ConfirmState, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
    use ratatui::{
        layout::{Alignment, Constraint, Direction, Layout, Rect},
        text::{Line, Span},
        widgets::{Block, Borders, Clear, Paragraph, Widget},
    };

    // Centre a 52×7 dialog
    let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(7),
            Constraint::Fill(1),
        ])
        .split(area);
    let hchunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(52),
            Constraint::Fill(1),
        ])
        .split(vchunks[1]);

    let dialog_area = hchunks[1];

    // Clear background behind dialog
    Clear.render(dialog_area, buf);

    let block = Block::default()
        .title(Span::styled(" Confirm ", theme::accent()))
        .borders(Borders::ALL)
        .border_style(theme::border_focused());

    let inner = block.inner(dialog_area);
    block.render(dialog_area, buf);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    Paragraph::new(st.message.as_str())
        .style(theme::text())
        .alignment(Alignment::Center)
        .render(rows[0], buf);

    let yes_style = if st.yes_focused {
        theme::selected()
    } else {
        theme::text()
    };
    let no_style = if !st.yes_focused {
        theme::selected()
    } else {
        theme::text()
    };

    Paragraph::new(Line::from(vec![
        Span::styled("  [ ", theme::muted()),
        Span::styled("yes", yes_style),
        Span::styled(" ]", theme::muted()),
    ]))
    .render(rows[1], buf);

    Paragraph::new(Line::from(vec![
        Span::styled("  [ ", theme::muted()),
        Span::styled("no ", no_style),
        Span::styled(" ]", theme::muted()),
    ]))
    .render(rows[2], buf);
}

// ── terminal guard ────────────────────────────────────────────────────────────

/// RAII guard: restores terminal on drop (including panics).
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stderr(), LeaveAlternateScreen);
    }
}

// ── public entry point ────────────────────────────────────────────────────────

/// Run the TUI until the user quits.
///
/// Draws to stderr so stdout stays clean for piped output.
pub fn run(mut app: App) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stderr());
    let mut terminal = Terminal::new(backend)?;

    // Install panic hook so cleanup always runs
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stderr(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let _guard = TerminalGuard;

    // Track how many frames the status has been shown (clear after 3 seconds ~= 180 frames)
    let mut status_frames: u32 = 0;
    const STATUS_TTL_FRAMES: u32 = 180;

    loop {
        // Update terminal dimensions
        let size = terminal.size()?;
        app.terminal_width = size.width;
        app.terminal_height = size.height;

        terminal.draw(|frame| render(&app, frame))?;

        // Age out transient status
        if app.status.is_some() {
            status_frames += 1;
            if status_frames >= STATUS_TTL_FRAMES {
                app.status = None;
                status_frames = 0;
            }
        } else {
            status_frames = 0;
        }

        // Clear "copied" flash flag after one render
        if app.copied {
            app.copied = false;
        }

        match next_event(Duration::from_millis(16))? {
            Some(action) => dispatch(&mut app, action),
            None => {}
        }

        if app.should_quit() {
            break;
        }
    }

    Ok(())
}
