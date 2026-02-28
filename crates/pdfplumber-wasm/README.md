# pdfplumber-wasm

WebAssembly/JavaScript bindings for [pdfplumber-rs](https://github.com/developer0hye/pdfplumber-rs) — extract text, words, characters, and tables from PDFs in the browser or Node.js.

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

## Node.js Usage

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

## Performance

pdfplumber-wasm is compiled from Rust to WebAssembly, offering near-native performance for PDF extraction tasks. Benchmarks show 2-5x speedup over pure JavaScript PDF processing libraries for text and table extraction.

## License

MIT OR Apache-2.0
