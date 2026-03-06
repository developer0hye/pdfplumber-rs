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
    widgets::{List, ListItem, ListState, Paragraph, Widget},
};

use super::{app::MenuState, theme, widgets};

/// Render the main menu screen into `area`.
pub fn render(menu: &MenuState, area: Rect, buf: &mut Buffer) {
    // Outer bordered box
    let block = widgets::bordered_box(None, false);
    let inner = block.inner(area);
    block.render(area, buf);

    // Layout: top padding, header, gap, list, gap, footer, bottom padding
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // top padding
            Constraint::Length(1), // header
            Constraint::Length(1), // gap
            Constraint::Length(menu.item_count as u16), // menu list
            Constraint::Min(1),    // spacer
            Constraint::Length(1), // footer
        ])
        .split(inner);

    // Header
    widgets::render_header(buf, chunks[1]);

    // Menu list
    let items: Vec<ListItem> = App::menu_items()
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let prefix = if i == menu.selected { "❯  " } else { "   " };
            let style = if i == menu.selected {
                theme::accent_bold()
            } else {
                theme::text()
            };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, theme::accent()),
                Span::styled(*label, style),
            ]))
        })
        .collect();

    let list = List::new(items);
    let mut list_state = ListState::default();
    list_state.select(Some(menu.selected));
    ratatui::widgets::StatefulWidget::render(list, chunks[3], buf, &mut list_state);

    // Footer
    widgets::render_footer(
        buf,
        chunks[5],
        &[("↑↓", "navigate"), ("enter", "select"), ("q", "quit")],
    );
}

// Import App to access menu_items() — avoid circular dep by inlining the labels.
struct App;
impl App {
    fn menu_items() -> &'static [&'static str] {
        &[
            "extract     pull text from a PDF",
            "tables      extract tables to CSV or JSON",
            "grep        search across a folder of PDFs",
            "process     batch convert a whole directory",
            "config      set up Ollama, output format, defaults",
        ]
    }
}
