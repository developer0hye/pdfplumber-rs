//! Application state machine for the pdfplumber interactive TUI.
//!
//! The TUI is structured as a state machine with five top-level screens:
//! [`Screen::Menu`], [`Screen::Extract`], [`Screen::Grep`],
//! [`Screen::Process`], and [`Screen::Config`].
//!
//! Each screen owns its own state struct. The [`App`] holds the active screen
//! and drives all transitions via [`App::handle_key`].

use std::path::PathBuf;

/// Which screen is currently displayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// Top-level menu.
    Menu,
    /// Single-file text/word/table extraction view.
    Extract(ExtractState),
    /// Cross-directory keyword search.
    Grep(GrepState),
    /// Batch directory processing with pre-flight confirmation.
    Process(ProcessState),
    /// Ollama + output-format configuration.
    Config(ConfigState),
    /// Confirmation dialog (destructive action gating).
    Confirm(ConfirmState),
    /// Quitting — signal the event loop to exit.
    Quit,
}

/// Menu cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuState {
    /// Index of the highlighted menu item (0-based).
    pub selected: usize,
    /// Total number of menu items.
    pub item_count: usize,
}

impl MenuState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            item_count: 5, // extract, tables, grep, process, config — must match MENU_ITEMS in screen_menu
        }
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn down(&mut self) {
        if self.selected + 1 < self.item_count {
            self.selected += 1;
        }
    }
}

/// State for the single-file extraction view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractState {
    /// PDF file being extracted.
    pub file: PathBuf,
    /// Which extraction mode is active.
    pub mode: ExtractMode,
    /// Current page index (0-based).
    pub page: usize,
    /// Total page count.
    pub page_count: usize,
    /// Vertical scroll offset in the output pane.
    pub scroll: usize,
    /// Extracted text lines for the current page + mode.
    pub lines: Vec<String>,
    /// Error message if extraction failed.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractMode {
    Text,
    Words,
    Tables,
    Chars,
}

impl ExtractMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Words => "words",
            Self::Tables => "tables",
            Self::Chars => "chars",
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            Self::Text => Self::Words,
            Self::Words => Self::Tables,
            Self::Tables => Self::Chars,
            Self::Chars => Self::Text,
        }
    }
}

/// State for the grep / cross-directory search view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepState {
    /// Directory being searched.
    pub dir: PathBuf,
    /// Current search query (mutable while typing).
    pub query: String,
    /// Whether the user is in query-editing mode.
    pub editing: bool,
    /// Cursor position within `query`.
    pub cursor: usize,
    /// Case-insensitive flag.
    pub case_insensitive: bool,
    /// Search results.
    pub results: Vec<GrepMatch>,
    /// Cursor over results list.
    pub selected: usize,
    /// Vertical scroll in the results list.
    pub scroll: usize,
    /// Expanded context for the selected result (if enter was pressed).
    pub context: Option<String>,
    /// Search in progress.
    pub searching: bool,
    /// Total files searched.
    pub files_searched: usize,
    /// Elapsed seconds.
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepMatch {
    pub file: PathBuf,
    pub page: usize,
    pub snippet: String,
}

/// State for the batch directory processing view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessState {
    /// Directory to process.
    pub dir: PathBuf,
    /// Output directory.
    pub output_dir: PathBuf,
    /// Pre-flight file list.
    pub files: Vec<FilePreview>,
    /// Whether confirmation has been given.
    pub confirmed: bool,
    /// Processing progress: (done, total).
    pub progress: (usize, usize),
    /// Scroll offset in the file list.
    pub scroll: usize,
    /// Currently processing file name (shown in status bar).
    pub current_file: Option<String>,
    /// Whether Ollama is configured.
    pub ollama_configured: bool,
    /// Number of scanned PDFs that contain at least one image-only page (need Ollama fallback).
    pub ollama_needed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePreview {
    pub name: String,
    pub pages: usize,
    pub needs_ollama: bool,
}

/// State for the configuration screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigState {
    /// Ollama base URL.
    pub ollama_url: String,
    /// Ollama model.
    pub ollama_model: String,
    /// Default output format.
    pub output_format: String,
    /// Currently focused field index.
    pub focused: usize,
    /// Whether a field is being edited.
    pub editing: bool,
    /// Cursor within the edited string.
    pub cursor: usize,
    /// Total number of editable fields.
    pub field_count: usize,
}

impl Default for ConfigState {
    fn default() -> Self {
        Self {
            ollama_url: "http://localhost:11434".to_string(),
            ollama_model: "llava".to_string(),
            output_format: "text".to_string(),
            focused: 0,
            editing: false,
            cursor: 0,
            field_count: 3,
        }
    }
}

/// Generic confirmation dialog state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmState {
    /// Short description of what is about to happen.
    pub message: String,
    /// The screen to return to if the user cancels.
    pub cancel_screen: Box<Screen>,
    /// The screen to go to if the user confirms.
    pub confirm_screen: Box<Screen>,
    /// Whether "yes" is focused (vs "no").
    pub yes_focused: bool,
}

/// Top-level application state.
pub struct App {
    /// Current menu state (cursor lives here even when on other screens).
    pub menu: MenuState,
    /// Active screen.
    pub screen: Screen,
    /// Terminal height in rows (updated on each render).
    pub terminal_height: u16,
    /// Terminal width in columns.
    pub terminal_width: u16,
    /// Status bar message (transient, cleared after one frame).
    pub status: Option<String>,
    /// Whether something was just copied to clipboard.
    pub copied: bool,
    /// Persisted config loaded at startup; used to initialise Config screen.
    pub saved_config: ConfigState,
    /// Working directory override (passed via `--tui --dir`).
    pub working_dir: Option<std::path::PathBuf>,
}

impl App {
    pub fn new() -> Self {
        Self {
            menu: MenuState::new(),
            screen: Screen::Menu,
            terminal_height: 24,
            terminal_width: 80,
            status: None,
            copied: false,
            saved_config: ConfigState::default(),
            working_dir: None,
        }
    }

    /// Returns true when the app should exit.
    pub fn should_quit(&self) -> bool {
        matches!(self.screen, Screen::Quit)
    }

    /// Navigate to the menu from any screen.
    pub fn go_menu(&mut self) {
        self.screen = Screen::Menu;
    }

    /// Menu item labels in order.
    pub fn menu_items() -> &'static [&'static str] {
        &[
            "extract     pull text from a PDF",
            "tables      extract tables to CSV or JSON",
            "grep        search across a folder of PDFs",
            "process     batch convert a whole directory",
            "config      set up Ollama, output format, defaults",
        ]
    }
}
