//! Single-file extraction view.
//!
//! Renders extracted text/words/tables/chars for a given page with scrolling.
//! Tab cycles extraction mode. ← → navigate pages. j/k or ↑↓ scroll output.
//! Enter + y copies current view to clipboard.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Widget},
};

use super::{app::ExtractState, theme, widgets};

pub fn render(state: &ExtractState, area: Rect, buf: &mut Buffer) {
    // Outer box
    let title = format!(
        " {} — page {}/{} — {} ",
        state
            .file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?"),
        state.page + 1,
        state.page_count,
        state.mode.label()
    );
    let block = widgets::bordered_box(Some(&title), true);
    let inner = block.inner(area);
    block.render(area, buf);

    // Layout: content area + footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    // Content
    if let Some(ref err) = state.error {
        Paragraph::new(Span::styled(err.as_str(), theme::warn())).render(chunks[0], buf);
    } else {
        let height = chunks[0].height as usize;
        let visible: Vec<ListItem> = state
            .lines
            .iter()
            .skip(state.scroll)
            .take(height)
            .map(|l| {
                ListItem::new(Line::from(Span::styled(l.as_str(), theme::text())))
            })
            .collect();
        List::new(visible).render(chunks[0], buf);
    }

    // Footer
    widgets::render_footer(
        buf,
        chunks[1],
        &[
            ("←→", "page"),
            ("tab", "mode"),
            ("↑↓", "scroll"),
            ("y", "copy"),
            ("esc", "menu"),
        ],
    );
}
