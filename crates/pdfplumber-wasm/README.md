# pdfplumber-wasm

Experimental WebAssembly bindings for evidence-driven PDF extraction with [pdfplumber-rs](https://github.com/developer0hye/pdfplumber-rs).

The npm package and import name are `pdfplumber-wasm`. Release `0.3.0` is experimental, uses the `Apache-2.0` license, and comes from `https://github.com/developer0hye/pdfplumber-rs`. The observed npm release remains `0.2.0`; source `0.3.0` has not yet been published there.

## Features

- **Text extraction** with optional layout detection
- **Word extraction** with configurable tolerance
- **Table detection and extraction** (lattice + stream strategies)
- **Character-level access** with font, size, and position data
- **Regex search** across page content
- **Zero native dependencies** — runs entirely in WebAssembly

## Installation

```bash
npm install pdfplumber-wasm
```

## Browser Usage

```html
<script type="module">
import init, { WasmPdf } from './pdfplumber_wasm.js';

await init();

const response = await fetch('document.pdf');
const bytes = new Uint8Array(await response.arrayBuffer());
const pdf = WasmPdf.open(bytes);

console.log(`Pages: ${pdf.pageCount}`);

const page = pdf.page(0);
console.log(page.extractText());

// Extract tables
const tables = page.extractTables();
for (const table of tables) {
  console.table(table);
}
</script>
```

### With a Bundler (Webpack, Vite, etc.)

```typescript
import { WasmPdf } from 'pdfplumber-wasm';

const response = await fetch('/document.pdf');
const bytes = new Uint8Array(await response.arrayBuffer());
const pdf = WasmPdf.open(bytes);

const page = pdf.page(0);
const text = page.extractText();
```

## Quick Start (Node.js)

```javascript
import { readFileSync } from 'fs';
import { WasmPdf } from 'pdfplumber-wasm';

const bytes = readFileSync('document.pdf');
const pdf = WasmPdf.open(bytes);

console.log(`Pages: ${pdf.pageCount}`);

const page = pdf.page(0);

// Extract text
console.log(page.extractText());

// Extract words with bounding boxes
const words = page.extractWords();
for (const word of words) {
  console.log(`"${word.text}" at (${word.x0}, ${word.top})`);
}

// Extract tables as 2D arrays
const tables = page.extractTables();
for (const table of tables) {
  for (const row of table) {
    console.log(row.join(' | '));
  }
}

// Search for text
const matches = page.search('hello', false, false);
console.log(`Found ${matches.length} matches`);
```

## API Reference

### `WasmPdf`

| Method / Property | Description |
|---|---|
| `WasmPdf.open(data: Uint8Array)` | Open a PDF from raw bytes |
| `.pageCount` | Number of pages |
| `.page(index: number)` | Get a page by 0-based index |
| `.metadata` | Document metadata (title, author, etc.) |

### `WasmPage`

| Method / Property | Description |
|---|---|
| `.pageNumber` | Page index (0-based) |
| `.width` | Page width in points |
| `.height` | Page height in points |
| `.extractText(layout?)` | Extract text (optional layout detection) |
| `.extractWords(xTol?, yTol?)` | Extract words with bounding boxes |
| `.chars()` | Get all characters with font/position data |
| `.findTables()` | Detect tables with cell structure |
| `.extractTables()` | Extract tables as 2D text arrays |
| `.search(pattern, regex?, case?)` | Search for text patterns |

### TypeScript Types

Import type definitions for rich typing:

```typescript
import type {
  PdfChar,
  PdfWord,
  PdfTable,
  PdfTableData,
  PdfSearchMatch,
  PdfMetadata,
  BBox,
} from 'pdfplumber-wasm';
```

## Building from Source

```bash
# Install wasm-pack
cargo install wasm-pack

# Build for bundlers (Webpack, Vite, Rollup)
wasm-pack build --target bundler crates/pdfplumber-wasm

# Build for Node.js
wasm-pack build --target nodejs crates/pdfplumber-wasm

# Build for browser (no bundler)
wasm-pack build --target web crates/pdfplumber-wasm
```

## Comparison with Other Tools

| Feature | pdfplumber-wasm | pdf.js | pdf-lib |
|---|---|---|---|
| Text extraction | Yes | Yes | No |
| Table detection | Yes | No | No |
| Word grouping | Yes | Partial | No |
| Character positions | Yes | Yes | No |
| Regex search | Yes | No | No |
| Runs in browser | Yes | Yes | Yes |
| Runs in Node.js | Yes | Yes | Yes |

## Performance evidence

No browser, Node.js, or cross-project performance result is currently claimed for
this WebAssembly surface.
The [versioned benchmark corpus](../../docs/benchmarks/corpus-v0.3.0.md) defines
shared inputs, and the [output-equivalence preflight](../../docs/benchmarks/equivalence-v0.3.0.md)
rejects mismatched semantics or canonical results before timing. The
[separated stage suite](../../docs/benchmarks/stages-v0.3.0.md) isolates the
native component clocks. The [resource and artifact suite](../../docs/benchmarks/metrics-v0.3.0.md)
measures the candidate Node package's WebAssembly module plus JavaScript glue
and uses a fresh Node process for each in-process module-load/startup clock.
The [workload-scenario suite](../../docs/benchmarks/scenarios-v0.3.0.md) defines
native cold/warm, page-scope, cache-hit, and bounded parallel workloads without
claiming that they cover browser execution. Those one-sample results remain local
and unpublished; browser startup, memory, and statistical gates remain open.
The [run-provenance contract](../../docs/benchmarks/provenance-v0.3.0.md)
records environment/build inputs and retains five raw repetitions plus descriptive summaries
for the native comparison workloads; it does not expand browser coverage.
The versioned `SCORE-008` [benchmark result assets](../../docs/benchmarks/results-v0.3.0.md)
retain that complete exact-tag native run; `SCORE-009` and the
[comparison policy](../../docs/comparison.md) govern removal without expanding browser evidence.

## License

Licensed under the [Apache License, Version 2.0](../../LICENSE).
