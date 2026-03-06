//! Main menu screen.
//!
//! ```text
//! ╭─────────────────────────────────────────────────────────────────╮
//! │                                                                 │
//! │  pdfplumber  ·  the one PDF tool you'll actually keep          │
//! │                                                                 │
//! │  ❯  extract     pull text from a PDF                           │
//! │     tables      extract tables to CSV or JSON                  │
//! │     grep        search across a folder of PDFs                 │
//! │     process     batch convert a whole directory                │
//! │     config      set up Ollama, output format, defaults         │
//! │                                                                 │
//! │  [↑↓] navigate  [enter] select  [q] quit                      │
//! ╰─────────────────────────────────────────────────────────────────╯
//! ```

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, Widget},
};

use super::{app::MenuState, theme, widgets};

const MENU_ITEMS: &[&str] = &[
    "extract     pull text from a PDF",
    "tables      extract tables to CSV or JSON",
    "grep        search across a folder of PDFs",
    "process     batch convert a whole directory",
    "config      set up Ollama, output format, defaults",
];

/// Render the main menu screen into `area`.
pub fn render(menu: &MenuState, area: Rect, buf: &mut Buffer) {
    // Outer bordered box
    let block = widgets::bordered_box(None, false);
    let inner = block.inner(area);
    block.render(area, buf);

    // Layout: top padding, header, gap, list, spacer, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),                       // top padding
            Constraint::Length(1),                       // header
            Constraint::Length(2),                       // gap
            Constraint::Length(MENU_ITEMS.len() as u16), // menu list
            Constraint::Min(1),                          // spacer
            Constraint::Length(1),                       // footer
        ])
        .split(inner);

    // Header
    widgets::render_header(buf, chunks[1]);

    // Menu list — manual prefix arrow, no StatefulWidget needed
    let items: Vec<ListItem> = MENU_ITEMS
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let selected = i == menu.selected;
            let prefix = if selected { "❯  " } else { "   " };
            let label_style = if selected {
                theme::accent_bold()
            } else {
                theme::text()
            };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, theme::accent()),
                Span::styled(*label, label_style),
            ]))
        })
        .collect();

    List::new(items).render(chunks[3], buf);

    // Footer
    widgets::render_footer(
        buf,
        chunks[5],
        &[("↑↓", "navigate"), ("enter", "select"), ("q", "quit")],
    );
}
