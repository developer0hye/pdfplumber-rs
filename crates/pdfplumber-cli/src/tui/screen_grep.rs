//! Grep / cross-directory search screen.
//!
//! ```text
//! ╭─ grep: "indemnification" in ./contracts/ ───────────────────────╮
//! │                                                                 │
//! │  3 matches · 47 files · 2.1s                                   │
//! │                                                                 │
//! │  ❯ contract_2024_001.pdf   p.7   "...indemnification clause..." │
//! │    contract_2024_017.pdf   p.3   "...waiver of indemnifica..."  │
//! │    contract_2024_031.pdf  p.12   "...see indemnification, app…" │
//! │                                                                 │
//! │  [↑↓] scroll  [enter] expand  [y] copy  [/] new search        │
//! ╰─────────────────────────────────────────────────────────────────╯
//! ```

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Widget},
};

use super::{app::GrepState, theme, widgets};

pub fn render(state: &GrepState, area: Rect, buf: &mut Buffer) {
    let title = if state.query.is_empty() {
        format!(
            " grep: (type /) in {} ",
            state.dir.display()
        )
    } else {
        format!(
            " grep: \"{}\" in {} ",
            state.query,
            state.dir.display()
        )
    };

    let block = widgets::bordered_box(Some(&title), true);
    let inner = block.inner(area);
    block.render(area, buf);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // stats bar
            Constraint::Length(1), // gap
            Constraint::Min(1),    // results
            Constraint::Length(1), // footer
        ])
        .split(inner);

    // Stats bar
    let stats = if state.searching {
        format!(
            "searching… {} files",
            state.files_searched
        )
    } else if state.results.is_empty() && !state.query.is_empty() {
        "no matches".to_string()
    } else {
        format!(
            "{} matches · {} files · {:.1}s",
            state.results.len(),
            state.files_searched,
            state.elapsed_ms as f64 / 1000.0
        )
    };
    Paragraph::new(Span::styled(stats, theme::muted())).render(chunks[0], buf);

    // Results list
    if let Some(ref ctx) = state.context {
        // Expanded context for selected result
        Paragraph::new(ctx.as_str())
            .style(theme::text())
            .render(chunks[2], buf);
    } else {
        let height = chunks[2].height as usize;
        let name_width = (chunks[2].width as usize).saturating_sub(20).max(20);

        let items: Vec<ListItem> = state
            .results
            .iter()
            .skip(state.scroll)
            .take(height)
            .enumerate()
            .map(|(i, m)| {
                let abs_i = i + state.scroll;
                let selected = abs_i == state.selected;
                let prefix = if selected { "❯ " } else { "  " };
                let name = m
                    .file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?");
                let name_cell = widgets::pad_to_width(name, name_width);
                let page_cell = widgets::pad_to_width(&format!("p.{}", m.page + 1), 6);
                let snippet = widgets::truncate_to_width(
                    &m.snippet,
                    (chunks[2].width as usize)
                        .saturating_sub(name_width + 6 + prefix.len() + 4),
                );

                let line = Line::from(vec![
                    Span::styled(prefix, theme::accent()),
                    Span::styled(name_cell, if selected { theme::selected() } else { theme::text() }),
                    Span::styled("  ", theme::muted()),
                    Span::styled(page_cell, theme::muted()),
                    Span::styled("  ", theme::muted()),
                    Span::styled(
                        format!("\"{}\"", snippet),
                        if selected { theme::accent() } else { theme::muted() },
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        List::new(items).render(chunks[2], buf);
    }

    // Footer
    let hints: &[(&str, &str)] = if state.context.is_some() {
        &[("esc", "results"), ("y", "copy")]
    } else if state.editing {
        &[("enter", "search"), ("esc", "cancel")]
    } else {
        &[
            ("↑↓", "scroll"),
            ("enter", "expand"),
            ("y", "copy"),
            ("/", "new search"),
            ("esc", "menu"),
        ]
    };
    widgets::render_footer(buf, chunks[3], hints);
}
