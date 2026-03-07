//! Per-screen keyboard action → state transition handlers.
//!
//! Each `handle_*` function receives a mutable reference to [`App`] plus the
//! decoded [`AppAction`] and updates state in-place.  The event loop calls the
//! correct handler based on `app.screen`.
//!
//! Extraction side-effects (loading pages, running grep) are triggered here by
//! calling into the [`extraction`] module; because extraction is synchronous
//! and potentially slow we run it on the calling thread.  For a future async
//! upgrade, replace the direct calls with channel messages.

use std::path::PathBuf;

use arboard::Clipboard;

use super::app::{App, ConfigState, ExtractMode, ExtractState, GrepState, ProcessState, Screen};
use super::config_persist;
use super::events::AppAction;
use super::extraction;
use super::process_scan;

// ── dispatch ─────────────────────────────────────────────────────────────────

/// Route the action to the right screen handler.
pub fn dispatch(app: &mut App, action: AppAction) {
    match action {
        AppAction::Resize(w, h) => {
            app.terminal_width = w;
            app.terminal_height = h;
            return;
        }
        AppAction::Tick => return,
        AppAction::Quit => {
            app.screen = Screen::Quit;
            return;
        }
        _ => {}
    }

    // Determine the active screen variant without keeping a borrow,
    // then call the appropriate handler (which takes &mut App).
    enum ScreenKind {
        Menu,
        Extract,
        Grep,
        Process,
        Config,
        Confirm,
        Quit,
    }
    let kind = match &app.screen {
        Screen::Menu => ScreenKind::Menu,
        Screen::Extract(_) => ScreenKind::Extract,
        Screen::Grep(_) => ScreenKind::Grep,
        Screen::Process(_) => ScreenKind::Process,
        Screen::Config(_) => ScreenKind::Config,
        Screen::Confirm(_) => ScreenKind::Confirm,
        Screen::Quit => ScreenKind::Quit,
    };
    match kind {
        ScreenKind::Menu => handle_menu(app, action),
        ScreenKind::Extract => handle_extract(app, action),
        ScreenKind::Grep => handle_grep(app, action),
        ScreenKind::Process => handle_process(app, action),
        ScreenKind::Config => handle_config(app, action),
        ScreenKind::Confirm => handle_confirm(app, action),
        ScreenKind::Quit => {}
    }
}

// ── menu ─────────────────────────────────────────────────────────────────────

fn handle_menu(app: &mut App, action: AppAction) {
    match action {
        AppAction::Up => app.menu.up(),
        AppAction::Down => app.menu.down(),
        AppAction::Enter => activate_menu_item(app),
        AppAction::Escape => app.screen = Screen::Quit,
        _ => {}
    }
}

fn activate_menu_item(app: &mut App) {
    // Resolve working directory: explicit arg > CWD
    let effective_dir = app
        .working_dir
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    match app.menu.selected {
        0 => {
            // extract — needs a file argument; show a hint if none was given.
            // (When a file is provided on startup the event_loop skips the menu
            // entirely and opens Extract directly.)
            app.status =
                Some("Pass a PDF file as argument: pdfplumber --tui <file.pdf>".to_string());
        }
        1 => {
            // tables — same: needs a file
            app.status =
                Some("Pass a PDF file as argument: pdfplumber --tui <file.pdf>".to_string());
        }
        2 => {
            // grep — open grep view with resolved dir
            app.screen = Screen::Grep(GrepState {
                dir: effective_dir,
                query: String::new(),
                editing: true,
                cursor: 0,
                case_insensitive: false,
                results: vec![],
                selected: 0,
                scroll: 0,
                context: None,
                searching: false,
                files_searched: 0,
                elapsed_ms: 0,
            });
        }
        3 => {
            // process — scan dir for PDFs, detect image-only pages
            let output_dir = effective_dir.join("out");
            // "configured" = URL field is non-empty (we don't probe the server
            // here since that would block the UI thread; errors surface later).
            let ollama_configured = !app.saved_config.ollama_url.is_empty();

            let mut st = ProcessState {
                dir: effective_dir.clone(),
                output_dir,
                files: vec![],
                confirmed: false,
                progress: (0, 0),
                scroll: 0,
                current_file: None,
                ollama_configured,
                ollama_needed: 0,
            };

            // Scan the directory — this is synchronous but fast (we only
            // check char/image counts, not extract full text).
            app.status = Some(format!("Scanning {}…", effective_dir.display()));
            process_scan::populate_process_state(&mut st, false);

            let file_count = st.files.len();
            let ollama_count = st.ollama_needed;
            app.screen = Screen::Process(st);

            // Update status with scan summary
            if ollama_count > 0 {
                app.status = Some(format!(
                    "Found {file_count} PDF(s) — {ollama_count} have image-only pages (need Ollama)"
                ));
            } else {
                app.status = Some(format!("Found {file_count} PDF(s) — all text-extractable"));
            }
        }
        4 => {
            // config — initialise from persisted values
            app.screen = Screen::Config(app.saved_config.clone());
        }
        _ => {}
    }
}

// ── extract ──────────────────────────────────────────────────────────────────

fn handle_extract(app: &mut App, action: AppAction) {
    // Guard: only handle this if we're on the Extract screen
    if !matches!(app.screen, Screen::Extract(_)) {
        return;
    }

    match action {
        AppAction::Escape => {
            app.screen = Screen::Menu;
        }
        AppAction::Left => {
            let Screen::Extract(ref mut st) = app.screen else {
                return;
            };
            if st.page > 0 {
                st.page -= 1;
                st.scroll = 0;
                reload_extract(st);
            }
        }
        AppAction::Right => {
            let Screen::Extract(ref mut st) = app.screen else {
                return;
            };
            if st.page + 1 < st.page_count {
                st.page += 1;
                st.scroll = 0;
                reload_extract(st);
            }
        }
        AppAction::Tab => {
            let Screen::Extract(ref mut st) = app.screen else {
                return;
            };
            st.mode = st.mode.cycle();
            st.scroll = 0;
            reload_extract(st);
        }
        AppAction::Up => {
            let Screen::Extract(ref mut st) = app.screen else {
                return;
            };
            if st.scroll > 0 {
                st.scroll -= 1;
            }
        }
        AppAction::Down => {
            let Screen::Extract(ref mut st) = app.screen else {
                return;
            };
            let max_scroll = st.lines.len().saturating_sub(1);
            if st.scroll < max_scroll {
                st.scroll += 1;
            }
        }
        AppAction::Yank => {
            // Build text in a scope that drops the borrow before we mutate app
            let text = {
                let Screen::Extract(ref st) = app.screen else {
                    return;
                };
                st.lines.join("\n")
            }; // borrow dropped here
            if let Ok(mut cb) = Clipboard::new() {
                if cb.set_text(&text).is_ok() {
                    app.copied = true;
                    app.status = Some("Copied to clipboard".to_string());
                }
            }
        }
        _ => {}
    }
}

/// Reload extraction lines for the current page+mode.
fn reload_extract(st: &mut ExtractState) {
    let result = match st.mode {
        ExtractMode::Text => extraction::extract_text_lines(&st.file, st.page),
        ExtractMode::Words => extraction::extract_word_lines(&st.file, st.page),
        ExtractMode::Tables => extraction::extract_table_lines(&st.file, st.page),
        ExtractMode::Chars => extraction::extract_char_lines(&st.file, st.page),
    };
    match result {
        Ok(lines) => {
            st.lines = lines;
            st.error = None;
        }
        Err(e) => {
            st.lines = vec![];
            st.error = Some(e);
        }
    }
}

// ── grep ─────────────────────────────────────────────────────────────────────

fn handle_grep(app: &mut App, action: AppAction) {
    // Extract editing flag without long-lived borrow
    let is_editing = matches!(&app.screen, Screen::Grep(st) if st.editing);

    if is_editing {
        match action {
            AppAction::Escape => {
                let Screen::Grep(ref mut st) = app.screen else {
                    return;
                };
                st.editing = false;
                let query_empty = st.query.is_empty();
                drop(st); // end borrow before mutating app.screen
                if query_empty {
                    app.screen = Screen::Menu;
                }
            }
            AppAction::Enter => {
                // End editing and run search (need owned state to call run_grep)
                let Screen::Grep(ref mut st) = app.screen else {
                    return;
                };
                st.editing = false;
                run_grep(st);
            }
            AppAction::Char(c) => {
                let Screen::Grep(ref mut st) = app.screen else {
                    return;
                };
                st.query.insert(st.cursor, c);
                st.cursor += 1;
            }
            AppAction::Backspace => {
                let Screen::Grep(ref mut st) = app.screen else {
                    return;
                };
                if st.cursor > 0 {
                    st.cursor -= 1;
                    st.query.remove(st.cursor);
                }
            }
            _ => {}
        }
        return;
    }

    // Results navigation mode
    match action {
        AppAction::Escape => {
            app.screen = Screen::Menu;
        }
        AppAction::Slash => {
            let Screen::Grep(ref mut st) = app.screen else {
                return;
            };
            st.editing = true;
            st.cursor = st.query.len();
        }
        AppAction::Up => {
            let Screen::Grep(ref mut st) = app.screen else {
                return;
            };
            if st.selected > 0 {
                st.selected -= 1;
                st.context = None;
                adjust_scroll(st.selected, &mut st.scroll, 10);
            }
        }
        AppAction::Down => {
            let Screen::Grep(ref mut st) = app.screen else {
                return;
            };
            if st.selected + 1 < st.results.len() {
                st.selected += 1;
                st.context = None;
                adjust_scroll(st.selected, &mut st.scroll, 10);
            }
        }
        AppAction::Enter => {
            let Screen::Grep(ref mut st) = app.screen else {
                return;
            };
            if st.context.is_some() {
                st.context = None;
            } else if let Some(m) = st.results.get(st.selected) {
                let page_idx = m.page.saturating_sub(1);
                if let Ok(lines) = extraction::extract_text_lines(&m.file, page_idx) {
                    st.context = Some(lines.join("\n"));
                }
            }
        }
        AppAction::Yank => {
            // Build text while borrowing state, then release borrow before
            // mutating app fields (copied, status)
            let text = {
                let Screen::Grep(ref st) = app.screen else {
                    return;
                };
                if let Some(ref ctx) = st.context {
                    ctx.clone()
                } else if let Some(m) = st.results.get(st.selected) {
                    format!("{}  p.{}  {}", m.file.display(), m.page + 1, m.snippet)
                } else {
                    return;
                }
            }; // borrow dropped here
            if let Ok(mut cb) = Clipboard::new() {
                if cb.set_text(&text).is_ok() {
                    app.copied = true;
                    app.status = Some("Copied to clipboard".to_string());
                }
            }
        }
        _ => {}
    }
}

fn run_grep(st: &mut GrepState) {
    let start = std::time::Instant::now();
    st.searching = true;
    match extraction::grep_dir(&st.dir, &st.query, st.case_insensitive) {
        Ok((results, files_searched)) => {
            st.results = results;
            st.files_searched = files_searched;
            st.selected = 0;
            st.scroll = 0;
            st.context = None;
        }
        Err(e) => {
            st.results = vec![];
            st.files_searched = 0;
            // Stash error in context display area
            st.context = Some(format!("Error: {e}"));
        }
    }
    st.elapsed_ms = start.elapsed().as_millis() as u64;
    st.searching = false;
}

// ── process ──────────────────────────────────────────────────────────────────

fn handle_process(app: &mut App, action: AppAction) {
    if !matches!(app.screen, Screen::Process(_)) {
        return;
    }

    // Extract confirmed flag without long-lived borrow
    let confirmed = matches!(&app.screen, Screen::Process(st) if st.confirmed);

    if confirmed {
        if matches!(action, AppAction::Escape) {
            app.screen = Screen::Menu;
        }
        return;
    }

    match action {
        AppAction::Escape => {
            app.screen = Screen::Menu;
        }
        AppAction::Up => {
            let Screen::Process(ref mut st) = app.screen else {
                return;
            };
            if st.scroll > 0 {
                st.scroll -= 1;
            }
        }
        AppAction::Down => {
            let Screen::Process(ref mut st) = app.screen else {
                return;
            };
            let max = st.files.len().saturating_sub(1);
            if st.scroll < max {
                st.scroll += 1;
            }
        }
        // 'y' is translated to AppAction::Yank globally; accept both here.
        AppAction::Yank | AppAction::Char('y') | AppAction::Enter => {
            // Check gate in a scope, then mutate
            let (can_proceed, needs_ollama_msg) = {
                let Screen::Process(ref st) = app.screen else {
                    return;
                };
                let ok = st.ollama_needed == 0 || st.ollama_configured;
                (ok, !ok)
            };
            if needs_ollama_msg {
                app.status = Some("Configure Ollama first (press 'c')".to_string());
                return;
            }
            if can_proceed {
                let Screen::Process(ref mut st) = app.screen else {
                    return;
                };
                st.confirmed = true;
                st.progress = (0, st.files.len());
                if let Some(f) = st.files.first() {
                    st.current_file = Some(f.name.clone());
                }
            }
        }
        AppAction::Char('n') => {
            app.screen = Screen::Menu;
        }
        AppAction::Char('c') => {
            app.screen = Screen::Config(app.saved_config.clone());
        }
        _ => {}
    }
}

// ── config ───────────────────────────────────────────────────────────────────

fn handle_config(app: &mut App, action: AppAction) {
    if !matches!(app.screen, Screen::Config(_)) {
        return;
    }

    let is_editing = matches!(&app.screen, Screen::Config(st) if st.editing);

    if is_editing {
        match action {
            AppAction::Escape | AppAction::Enter => {
                let Screen::Config(ref mut st) = app.screen else {
                    return;
                };
                st.editing = false;
            }
            AppAction::Char(c) => {
                let Screen::Config(ref mut st) = app.screen else {
                    return;
                };
                field_insert(st, c);
            }
            AppAction::Backspace => {
                let Screen::Config(ref mut st) = app.screen else {
                    return;
                };
                field_backspace(st);
            }
            _ => {}
        }
        return;
    }

    match action {
        AppAction::Escape => {
            app.screen = Screen::Menu;
        }
        AppAction::Up => {
            let Screen::Config(ref mut st) = app.screen else {
                return;
            };
            if st.focused > 0 {
                st.focused -= 1;
                st.cursor = field_len(st);
            }
        }
        AppAction::Down | AppAction::Tab => {
            let Screen::Config(ref mut st) = app.screen else {
                return;
            };
            if st.focused + 1 < st.field_count {
                st.focused += 1;
                st.cursor = field_len(st);
            }
        }
        AppAction::Enter => {
            let Screen::Config(ref mut st) = app.screen else {
                return;
            };
            st.editing = true;
            st.cursor = field_len(st);
        }
        AppAction::Save => {
            // Clone config state in a scope, then persist + update app
            let st_clone = {
                let Screen::Config(ref st) = app.screen else {
                    return;
                };
                st.clone()
            }; // borrow dropped
            match config_persist::save_config(&st_clone) {
                Ok(()) => {
                    app.saved_config = st_clone;
                    app.status = Some(format!(
                        "Config saved → {}",
                        config_persist::config_path().display()
                    ));
                }
                Err(e) => {
                    app.status = Some(format!("Save failed: {e}"));
                }
            }
            app.screen = Screen::Menu;
        }
        _ => {}
    }
}

fn field_insert(st: &mut ConfigState, c: char) {
    match st.focused {
        0 => {
            st.ollama_url.insert(st.cursor, c);
            st.cursor += 1;
        }
        1 => {
            st.ollama_model.insert(st.cursor, c);
            st.cursor += 1;
        }
        2 => {
            st.output_format.insert(st.cursor, c);
            st.cursor += 1;
        }
        _ => {}
    }
}

fn field_backspace(st: &mut ConfigState) {
    if st.cursor == 0 {
        return;
    }
    st.cursor -= 1;
    match st.focused {
        0 => {
            st.ollama_url.remove(st.cursor);
        }
        1 => {
            st.ollama_model.remove(st.cursor);
        }
        2 => {
            st.output_format.remove(st.cursor);
        }
        _ => {}
    }
}

fn field_len(st: &ConfigState) -> usize {
    match st.focused {
        0 => st.ollama_url.len(),
        1 => st.ollama_model.len(),
        2 => st.output_format.len(),
        _ => 0,
    }
}

// ── confirm ───────────────────────────────────────────────────────────────────

fn handle_confirm(app: &mut App, action: AppAction) {
    if !matches!(app.screen, Screen::Confirm(_)) {
        return;
    }

    match action {
        AppAction::Left | AppAction::Right | AppAction::Tab => {
            let Screen::Confirm(ref mut st) = app.screen else {
                return;
            };
            st.yes_focused = !st.yes_focused;
        }
        AppAction::Enter => {
            // Extract next screen in a scope that drops the borrow
            let next = {
                let Screen::Confirm(ref st) = app.screen else {
                    return;
                };
                if st.yes_focused {
                    *st.confirm_screen.clone()
                } else {
                    *st.cancel_screen.clone()
                }
            }; // borrow dropped
            app.screen = next;
        }
        AppAction::Escape => {
            let next = {
                let Screen::Confirm(ref st) = app.screen else {
                    return;
                };
                *st.cancel_screen.clone()
            }; // borrow dropped
            app.screen = next;
        }
        _ => {}
    }
}

// ── misc helpers ──────────────────────────────────────────────────────────────

/// Keep `scroll` such that `selected` is visible in a window of `height` rows.
fn adjust_scroll(selected: usize, scroll: &mut usize, height: usize) {
    if selected < *scroll {
        *scroll = selected;
    } else if selected >= *scroll + height {
        *scroll = selected.saturating_sub(height - 1);
    }
}
