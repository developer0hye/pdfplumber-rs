# pdfplumber-wasm — Agent Working Memory

```bash
# Requires wasm-pack
wasm-pack build --target web
cargo check -p pdfplumber-wasm
```

**~20 tests | ~560 lines | 1 file | 2026-03-06**

---

## Project State

`wasm-bindgen`-based JavaScript/WASM bindings for pdfplumber-rs. Exposes `WasmPdf`, `WasmPage`, `WasmTable` classes to JavaScript. Serializes complex return types via `serde_wasm_bindgen`.

### What's Built

- `WasmPdf`: `open(bytes)`, `page_count()`, `page(idx)`, `metadata()`
- `WasmPage`: `extract_text()`, `chars()`, `words()`, `tables()`
- `WasmTable`: `rows()`, `to_json()`
- `examples/browser-demo.html`: full in-browser demo

### What's Not Done

- Published to npm
- TypeScript type declarations (`.d.ts`)
- Node.js target build (`--target nodejs`)
- Streaming/chunked loading for large PDFs (no WASM memory limit handling)

---

## How This Fits in the Workspace

| Dependency | What it gives us |
|------------|-----------------|
| `pdfplumber` | All extraction APIs (with `default-features = false` for WASM) |
| `wasm-bindgen` | JS/WASM FFI |
| `serde_wasm_bindgen` | Complex type serialization to JsValue |
| `js-sys` | JavaScript type access |
| Used by: browser demos, any web-based PDF tool |

---

## Architecture Rules

1. **`default-features = false` on `pdfplumber` dep.** This disables file-path APIs unavailable in WASM.
2. **No `std::fs` anywhere.** WASM receives bytes, not paths.
3. **`#[wasm_bindgen]` on public API only.** Internal helpers stay private.
4. **`console_error_panic_hook` in init.** Panic messages must be readable in browser devtools.

---

## Decisions Log

- **2026-03-06**: Added CLAUDE.md as part of Shippable Crate Standard pass.
