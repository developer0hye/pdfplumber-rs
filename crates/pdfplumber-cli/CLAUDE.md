# pdfplumber-cli — Agent Working Memory

```bash
cargo test -p pdfplumber-cli
cargo check -p pdfplumber-cli
cargo build --release -p pdfplumber-cli
```

**~30 tests | ~2,000 lines | 10 files | 2026-03-06**

---

## Project State

Command-line frontend for pdfplumber-rs. Five subcommands: `text`, `chars`, `words`, `tables`, `info`. Output formats: plain text/TSV/grid, JSON, CSV. Page selection (`--pages 1-3,5`). No library surface — binary only.

### What's Built

| File | Description |
|------|-------------|
| `cli.rs` | `clap`-based argument parsing, top-level dispatch |
| `text_cmd.rs` | `text` subcommand — extract text, layout mode |
| `chars_cmd.rs` | `chars` subcommand — char-level TSV/JSON/CSV |
| `words_cmd.rs` | `words` subcommand |
| `tables_cmd.rs` | `tables` subcommand — grid/JSON/CSV output |
| `info_cmd.rs` | `info` subcommand — metadata summary |
| `annots_cmd.rs` | `annots` subcommand — annotation listing |
| `bookmarks_cmd.rs` | `bookmarks` subcommand — PDF outline |
| `debug_cmd.rs` | `debug` subcommand — low-level diagnostics |
| `pages.rs` | `--pages` range parser |

### What's Not Done

- Shell completion generation (`clap_complete`) — would be a nice addition
- Man page generation

---

## How This Fits in the Workspace

| Dependency | What it gives us |
|------------|-----------------|
| `pdfplumber` | All extraction APIs |
| `clap` | Argument parsing |
| `serde_json` | JSON output formatting |

---

## Architecture Rules

1. **Binary only — no `lib.rs`.** This crate exposes no library API.
2. **Each subcommand is its own module.** New subcommands get a new `*_cmd.rs` file.
3. **Output goes to stdout; errors go to stderr.** `eprintln!` for errors, `println!` for output.
4. **Exit code 1 on any error.** No panics reaching the user.

---

## Decisions Log

- **2026-03-06**: Added CLAUDE.md as part of Shippable Crate Standard pass.
