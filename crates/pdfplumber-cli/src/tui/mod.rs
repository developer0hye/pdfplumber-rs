//! Interactive TUI for pdfplumber — feature-gated behind `tui`.
//!
//! # Entry point
//!
//! ```no_run
//! use pdfplumber_cli::tui;
//! use std::path::PathBuf;
//!
//! tui::run(Some(PathBuf::from("doc.pdf")), None).unwrap();
//! ```
//!
//! Pass `file` to pre-load the Extract screen, `dir` to open Grep/Process
//! with a working directory.  Either may be `None` to start at the Menu.

pub mod app;
pub mod config_persist;
pub mod event_loop;
pub mod events;
pub mod extraction;
pub mod input_handlers;
pub mod process_scan;
pub mod screen_config;
pub mod screen_extract;
pub mod screen_grep;
pub mod screen_menu;
pub mod screen_process;
pub mod theme;
pub mod widgets;

use std::path::PathBuf;

use app::{App, ExtractMode, ExtractState, Screen};

/// Run the TUI. Blocks until the user quits.
///
/// - `file` — if `Some`, open the Extract screen for that PDF on startup.
/// - `dir`  — if `Some`, use as working directory for Grep/Process screens.
pub fn run(file: Option<PathBuf>, dir: Option<PathBuf>) -> std::io::Result<()> {
    let mut app = App::new();

    // Load persisted config so the Config screen starts with real values
    app.saved_config = config_persist::load_config();

    // If a file was provided, jump straight to Extract view
    if let Some(path) = file {
        let page_count = extraction::page_count(&path).unwrap_or(1);
        let mut st = ExtractState {
            file: path.clone(),
            mode: ExtractMode::Text,
            page: 0,
            page_count,
            scroll: 0,
            lines: vec![],
            error: None,
        };
        // Pre-load page 0
        match extraction::extract_text_lines(&path, 0) {
            Ok(lines) => st.lines = lines,
            Err(e) => st.error = Some(e),
        }
        app.screen = Screen::Extract(st);
    }

    // Store the working dir so grep/process screens use it instead of CWD
    if let Some(path) = dir {
        app.working_dir = Some(path);
    }

    event_loop::run(app)
}
