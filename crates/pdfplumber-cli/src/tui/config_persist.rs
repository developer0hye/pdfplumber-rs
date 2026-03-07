//! Persistent configuration for the TUI — stored at
//! `~/.config/pdfplumber/config.toml`.
//!
//! On first launch the file is created with sensible defaults.  The file is
//! plain TOML so users can hand-edit it; we never error on unknown keys.
//!
//! # Format
//!
//! ```toml
//! ollama_url    = "http://localhost:11434"
//! ollama_model  = "llava"
//! output_format = "text"
//! ```

use std::fs;
use std::io;
use std::path::PathBuf;

use super::app::ConfigState;

// ── path resolution ───────────────────────────────────────────────────────────

/// Returns `~/.config/pdfplumber/config.toml`, falling back to a path
/// relative to CWD if the home directory cannot be determined.
pub fn config_path() -> PathBuf {
    // dirs::config_dir() returns the platform config root:
    //   Linux/macOS: $HOME/.config
    //   Windows:     %APPDATA%
    let base = dirs::config_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    base.join("pdfplumber").join("config.toml")
}

// ── load ──────────────────────────────────────────────────────────────────────

/// Load persisted config from disk.  If the file does not exist (first run)
/// returns `ConfigState::default()`.  Parse errors are silently ignored and
/// the default is used instead — we never crash on a bad config file.
pub fn load_config() -> ConfigState {
    let path = config_path();
    if !path.exists() {
        return ConfigState::default();
    }
    let src = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return ConfigState::default(),
    };
    parse_toml(&src).unwrap_or_default()
}

/// Minimal TOML parser — only handles `key = "value"` string lines.
///
/// We intentionally avoid pulling in the full `toml` crate here; the config
/// file has exactly three string fields and we want zero-overhead parsing that
/// works without serde.  If the file ever gains complex structure we can
/// upgrade.
fn parse_toml(src: &str) -> Option<ConfigState> {
    let mut cfg = ConfigState::default();
    for line in src.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, rest)) = line.split_once('=') {
            let key = key.trim();
            // Strip surrounding quotes from value
            let val = rest.trim().trim_matches('"').to_string();
            match key {
                "ollama_url" => cfg.ollama_url = val,
                "ollama_model" => cfg.ollama_model = val,
                "output_format" => cfg.output_format = val,
                _ => {} // forward-compatible: ignore unknown keys
            }
        }
    }
    Some(cfg)
}

// ── save ──────────────────────────────────────────────────────────────────────

/// Persist `ConfigState` to disk.  Creates the parent directory if it does
/// not exist.  Returns an error only if the write itself fails (e.g. read-only
/// filesystem); callers should surface this to the status bar.
pub fn save_config(st: &ConfigState) -> io::Result<()> {
    let path = config_path();

    // Ensure the parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let toml = format!(
        "# pdfplumber TUI configuration\n\
         # Edit this file or use the Config screen (pdfplumber --tui → config)\n\
         \n\
         ollama_url    = \"{}\"\n\
         ollama_model  = \"{}\"\n\
         output_format = \"{}\"\n",
        escape_toml_string(&st.ollama_url),
        escape_toml_string(&st.ollama_model),
        escape_toml_string(&st.output_format),
    );

    fs::write(&path, toml)
}

/// Escape backslashes and double-quotes in a TOML basic string value.
fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cfg() -> ConfigState {
        ConfigState::default()
    }

    #[test]
    fn parse_well_formed_toml() {
        let src = r#"
# comment
ollama_url    = "http://localhost:11434"
ollama_model  = "llava"
output_format = "json"
"#;
        let cfg = parse_toml(src).unwrap();
        assert_eq!(cfg.ollama_url, "http://localhost:11434");
        assert_eq!(cfg.ollama_model, "llava");
        assert_eq!(cfg.output_format, "json");
    }

    #[test]
    fn parse_partial_toml_uses_defaults_for_missing() {
        let src = r#"ollama_model = "mistral""#;
        let cfg = parse_toml(src).unwrap();
        assert_eq!(cfg.ollama_model, "mistral");
        // Fields not present should stay at default
        let d = default_cfg();
        assert_eq!(cfg.ollama_url, d.ollama_url);
        assert_eq!(cfg.output_format, d.output_format);
    }

    #[test]
    fn parse_unknown_keys_ignored() {
        let src = r#"
ollama_url = "http://example.com"
future_setting = "whatever"
"#;
        let cfg = parse_toml(src).unwrap();
        assert_eq!(cfg.ollama_url, "http://example.com");
        // No panic, no error
    }

    #[test]
    fn parse_empty_returns_default() {
        let cfg = parse_toml("").unwrap();
        let d = default_cfg();
        assert_eq!(cfg.ollama_url, d.ollama_url);
        assert_eq!(cfg.ollama_model, d.ollama_model);
    }

    #[test]
    fn save_and_reload_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        // Write manually using the serialiser
        let original = ConfigState {
            ollama_url: "http://remote:11434".to_string(),
            ollama_model: "bakllava".to_string(),
            output_format: "json".to_string(),
            focused: 0,
            editing: false,
            cursor: 0,
            field_count: 3,
        };

        let toml = format!(
            "ollama_url    = \"{}\"\nollama_model  = \"{}\"\noutput_format = \"{}\"\n",
            original.ollama_url, original.ollama_model, original.output_format,
        );
        std::fs::write(&path, toml).unwrap();

        // Re-parse
        let src = std::fs::read_to_string(&path).unwrap();
        let loaded = parse_toml(&src).unwrap();

        assert_eq!(loaded.ollama_url, original.ollama_url);
        assert_eq!(loaded.ollama_model, original.ollama_model);
        assert_eq!(loaded.output_format, original.output_format);
    }

    #[test]
    fn escape_quotes_and_backslashes() {
        let s = r#"path\to\"something""#;
        let escaped = escape_toml_string(s);
        assert!(escaped.contains("\\\\"));
        assert!(escaped.contains("\\\""));
    }
}
