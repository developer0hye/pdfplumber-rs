//! Configuration screen.
//!
//! Three editable fields: Ollama URL, Ollama model, default output format.
//! Tab moves between fields. Enter starts editing. Escape cancels edit.
//! s saves to ~/.config/pdfplumber/config.toml.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use super::{app::ConfigState, config_persist, theme, widgets};

pub fn render(state: &ConfigState, area: Rect, buf: &mut Buffer) {
    let block = widgets::bordered_box(Some(" config "), false);
    let inner = block.inner(area);
    block.render(area, buf);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Length(1), // gap
            Constraint::Length(3), // field 0: ollama url
            Constraint::Length(3), // field 1: ollama model
            Constraint::Length(3), // field 2: output format
            Constraint::Length(1), // gap
            Constraint::Length(1), // config file path
            Constraint::Min(1),    // spacer
            Constraint::Length(1), // footer
        ])
        .split(inner);

    Paragraph::new(Span::styled("pdfplumber settings", theme::accent())).render(chunks[0], buf);

    render_field(
        buf,
        chunks[2],
        "Ollama URL",
        &state.ollama_url,
        state.focused == 0,
        state.editing && state.focused == 0,
    );
    render_field(
        buf,
        chunks[3],
        "Ollama model",
        &state.ollama_model,
        state.focused == 1,
        state.editing && state.focused == 1,
    );
    render_field(
        buf,
        chunks[4],
        "default output format",
        &state.output_format,
        state.focused == 2,
        state.editing && state.focused == 2,
    );

    // Show config file path so users know where to hand-edit
    let cfg_path_str = config_persist::config_path().display().to_string();
    let path_line = Line::from(vec![
        Span::styled("saved to  ", theme::muted()),
        Span::styled(cfg_path_str, theme::accent()),
    ]);
    Paragraph::new(path_line).render(chunks[6], buf);

    let hints: &[(&str, &str)] = if state.editing {
        &[("enter", "save field"), ("esc", "cancel edit")]
    } else {
        &[
            ("tab", "next field"),
            ("enter", "edit"),
            ("s", "save config"),
            ("esc", "menu"),
        ]
    };
    widgets::render_footer(buf, chunks[8], hints);
}

fn render_field(
    buf: &mut Buffer,
    area: Rect,
    label: &str,
    value: &str,
    focused: bool,
    editing: bool,
) {
    let label_area = Rect {
        height: 1,
        ..area
    };
    let value_area = Rect {
        y: area.y + 1,
        height: 1,
        ..area
    };

    // Label
    let label_style = if focused {
        theme::accent_bold()
    } else {
        theme::muted()
    };
    Paragraph::new(Span::styled(label, label_style)).render(label_area, buf);

    // Value with cursor if editing
    let display = if editing {
        format!("{}_", value) // simple block cursor via underscore
    } else {
        value.to_string()
    };
    let value_style = if editing {
        theme::text()
    } else if focused {
        theme::accent()
    } else {
        theme::muted()
    };
    Paragraph::new(Span::styled(display, value_style)).render(value_area, buf);
}
