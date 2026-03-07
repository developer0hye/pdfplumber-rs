//! Batch directory processing screen with pre-flight confirmation.
//!
//! ```text
//! ╭─ process: ./contracts/ ─────────────────────────────────────────╮
//! │  47 PDFs  ·  output → ./contracts/extracted/                    │
//! │                                                                  │
//! │  ┌──────────────────────────────────┬───────┬──────────────┐   │
//! │  │ file                             │ pages │ note         │   │
//! │  ├──────────────────────────────────┼───────┼──────────────┤   │
//! │  │ contract_2024_001.pdf            │    12 │              │   │
//! │  │ annual_report_SCANNED.pdf        │    44 │ ⚠ image-only │   │
//! │  └──────────────────────────────────┴───────┴──────────────┘   │
//! │                                                                  │
//! │  proceed?  [y] yes  [n] no  [c] configure first                │
//! ╰──────────────────────────────────────────────────────────────────╯
//! ```

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Paragraph, Row, Table, Widget},
};

use super::{app::ProcessState, theme, widgets};

pub fn render(state: &ProcessState, area: Rect, buf: &mut Buffer) {
    let title = format!(" process: {} ", state.dir.display());
    let block = widgets::bordered_box(Some(&title), true);
    let inner = block.inner(area);
    block.render(area, buf);

    if state.confirmed {
        render_progress(state, inner, buf);
    } else {
        render_preflight(state, inner, buf);
    }
}

fn render_preflight(state: &ProcessState, area: Rect, buf: &mut Buffer) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // summary line
            Constraint::Length(1), // gap
            Constraint::Min(1),    // file table
            Constraint::Length(1), // gap
            Constraint::Length(2), // ollama notice
            Constraint::Length(1), // footer
        ])
        .split(area);

    // Summary
    let summary = format!(
        "{} PDFs  ·  output → {}/",
        state.files.len(),
        state.output_dir.display()
    );
    Paragraph::new(Span::styled(summary, theme::text())).render(chunks[0], buf);

    // File table
    let name_width = (chunks[2].width as usize)
        .saturating_sub(7 + 14 + 6)
        .max(20) as u16;
    let header = Row::new(vec!["file", "pages", "note"])
        .style(theme::accent())
        .height(1);

    let height = chunks[2].height as usize;
    let rows_with_note: Vec<Row> = state
        .files
        .iter()
        .skip(state.scroll)
        .take(height.saturating_sub(2))
        .map(|f| {
            let note = if f.needs_ollama {
                " ⚠ image-only"
            } else {
                ""
            };
            Row::new(vec![
                widgets::truncate_to_width(&f.name, name_width as usize),
                format!("{:>5}", f.pages),
                note.to_string(),
            ])
        })
        .collect();

    Table::new(
        rows_with_note,
        [
            Constraint::Length(name_width),
            Constraint::Length(6),
            Constraint::Min(14),
        ],
    )
    .header(header)
    .column_spacing(1)
    .render(chunks[2], buf);

    // Ollama notice
    if state.ollama_needed > 0 {
        let ollama_line = format!(
            "{} pages across files need Ollama fallback",
            state.ollama_needed
        );
        Paragraph::new(Span::styled(ollama_line, theme::warn())).render(chunks[4], buf);
        let config_line = if state.ollama_configured {
            "ollama: configured ✓".to_string()
        } else {
            "ollama: not configured  ·  run config to enable".to_string()
        };
        let config_style = if state.ollama_configured {
            theme::ok()
        } else {
            theme::muted()
        };
        let second_row = Rect {
            y: chunks[4].y + 1,
            height: 1,
            ..chunks[4]
        };
        Paragraph::new(Span::styled(config_line, config_style)).render(second_row, buf);
    }

    // Footer / confirmation prompt
    widgets::render_footer(
        buf,
        chunks[5],
        &[("y", "proceed"), ("n", "cancel"), ("c", "configure first")],
    );
}

fn render_progress(state: &ProcessState, area: Rect, buf: &mut Buffer) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // progress line
            Constraint::Length(1), // current file
            Constraint::Min(1),    // spacer
            Constraint::Length(1), // footer
        ])
        .split(area);

    let (done, total) = state.progress;
    let pct = if total > 0 { (done * 100) / total } else { 0 };
    Paragraph::new(Line::from(vec![
        Span::styled(format!("{}/{} files  ", done, total), theme::text()),
        Span::styled(format!("({}%)", pct), theme::accent()),
    ]))
    .render(chunks[0], buf);

    if let Some(ref f) = state.current_file {
        Paragraph::new(Span::styled(format!("processing: {}", f), theme::muted()))
            .render(chunks[1], buf);
    }

    widgets::render_footer(buf, chunks[3], &[("esc", "abort")]);
}
