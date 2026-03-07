//! Shared widget helpers used by multiple screens.
//!
//! These are thin wrappers over ratatui primitives that enforce the design rules:
//! - Fixed-width column arithmetic so lines always align.
//! - Every screen has a footer with active keybinds.
//! - Escape always goes back.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use super::theme;

/// Render the product header line.
///
/// ```
/// pdfplumber  ·  the one PDF tool you'll actually keep
/// ```
pub fn render_header(buf: &mut Buffer, area: Rect) {
    let line = Line::from(vec![
        Span::styled("pdfplumber", theme::accent_bold()),
        Span::styled("  ·  ", theme::muted()),
        Span::styled("the one PDF tool you'll actually keep", theme::muted()),
    ]);
    Paragraph::new(line).render(area, buf);
}

/// Render the keybind footer bar.
///
/// `hints` is a slice of (key, label) pairs, e.g. `&[("↑↓", "navigate"), ("enter", "select")]`.
pub fn render_footer(buf: &mut Buffer, area: Rect, hints: &[(&str, &str)]) {
    let mut spans: Vec<Span> = Vec::new();
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", theme::muted()));
        }
        spans.push(Span::styled(format!("[{}]", key), theme::key_hint()));
        spans.push(Span::styled(format!(" {}", label), theme::key_label()));
    }
    Paragraph::new(Line::from(spans)).render(area, buf);
}

/// Render a bordered box with an optional title.
///
/// Returns the inner area (inside the borders) for the caller to render content into.
pub fn bordered_box<'a>(title: Option<&'a str>, focused: bool) -> Block<'a> {
    let border_style = if focused {
        theme::border_focused()
    } else {
        theme::border()
    };
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);
    if let Some(t) = title {
        block = block.title(Span::styled(t, theme::accent()));
    }
    block
}

/// Render a status line (single row, right-aligned within `area`).
pub fn render_status(buf: &mut Buffer, area: Rect, msg: &str, style: Style) {
    let truncated = truncate_to_width(msg, area.width as usize);
    Paragraph::new(Span::styled(truncated, style)).render(area, buf);
}

/// Truncate a string so it fits within `max_chars` display columns.
///
/// Adds "…" suffix when truncation occurs.
pub fn truncate_to_width(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    // Use char count as proxy for display width (good enough for ASCII-dominant UI text).
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else if max_chars <= 1 {
        "…".to_string()
    } else {
        let truncated: String = chars[..max_chars - 1].iter().collect();
        format!("{}…", truncated)
    }
}

/// Pad a string to exactly `width` display columns.
pub fn pad_to_width(s: &str, width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= width {
        truncate_to_width(s, width)
    } else {
        format!("{}{}", s, " ".repeat(width - chars.len()))
    }
}

/// Format a file size in bytes as a human-readable string.
pub fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
