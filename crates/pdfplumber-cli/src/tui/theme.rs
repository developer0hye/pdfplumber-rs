//! Colour palette and style constants for the pdfplumber TUI.
//!
//! Design rules:
//! - Dark background (#1c1c1c approximated as terminal Black).
//! - One soft accent: Blue (cool, calm, professional).
//! - Muted grays for secondary text.
//! - Nothing garish. Nothing that looks like a Windows 95 dialog.

use ratatui::style::{Color, Modifier, Style};

/// Dark background — default terminal background (transparent).
pub const BG: Color = Color::Reset;

/// Primary accent: soft blue.
pub const ACCENT: Color = Color::Cyan;

/// Primary text: bright white.
pub const TEXT: Color = Color::White;

/// Secondary / muted text: dark gray.
pub const MUTED: Color = Color::DarkGray;

/// Error / warning: amber-ish yellow.
pub const WARN: Color = Color::Yellow;

/// Success: green.
pub const OK: Color = Color::Green;

/// Border: dark gray (not the accent — accent is for content).
pub const BORDER: Color = Color::DarkGray;

/// Selected item highlight.
pub const SELECTED_BG: Color = Color::Blue;
pub const SELECTED_FG: Color = Color::White;

// ---

/// Normal body text style.
pub fn text() -> Style {
    Style::default().fg(TEXT)
}

/// Muted / secondary text.
pub fn muted() -> Style {
    Style::default().fg(MUTED)
}

/// Accent text (headings, active indicators).
pub fn accent() -> Style {
    Style::default().fg(ACCENT)
}

/// Bold accent (selected menu item arrow, highlighted key).
pub fn accent_bold() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// Selected list item.
pub fn selected() -> Style {
    Style::default().bg(SELECTED_BG).fg(SELECTED_FG).add_modifier(Modifier::BOLD)
}

/// Warning / error text.
pub fn warn() -> Style {
    Style::default().fg(WARN)
}

/// Success text.
pub fn ok() -> Style {
    Style::default().fg(OK)
}

/// Box border (default, unfocused).
pub fn border() -> Style {
    Style::default().fg(BORDER)
}

/// Box border (focused / active pane).
pub fn border_focused() -> Style {
    Style::default().fg(ACCENT)
}

/// Key hint in footer: [key] label.
pub fn key_hint() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn key_label() -> Style {
    Style::default().fg(MUTED)
}
