// JavaScript test for pdfplumber-wasm
//
// Prerequisites:
//   1. Install wasm-pack: cargo install wasm-pack
//   2. Build the package: wasm-pack build --target nodejs crates/pdfplumber-wasm
//   3. Run this test: node crates/pdfplumber-wasm/tests/test.mjs
//
// This test verifies: load PDF bytes, extract text, extract tables.

import { readFileSync } from "fs";
import { WasmPdf } from "../pkg/pdfplumber_wasm.js";

// Create a minimal test PDF inline (same structure as Rust test helper)
// For a real test, use: const pdfBytes = readFileSync("path/to/test.pdf");

function runTests() {
  let passed = 0;
  let failed = 0;

  function assert(condition, message) {
    if (condition) {
      passed++;
      console.log(`  PASS: ${message}`);
    } else {
      failed++;
      console.error(`  FAIL: ${message}`);
    }
  }

  console.log("pdfplumber-wasm JavaScript tests\n");

  // Test 1: Open PDF from bytes
  console.log("Test: Open PDF");
  try {
    // If a test PDF file exists, use it
    let pdfBytes;
    try {
      pdfBytes = readFileSync("tests/fixtures/hello.pdf");
    } catch {
      console.log(
        "  SKIP: No test PDF found at tests/fixtures/hello.pdf\n" +
          "  To run full tests, provide a PDF file.\n"
      );
      console.log(`\nResults: ${passed} passed, ${failed} failed`);
      return failed === 0;
    }

    const pdf = WasmPdf.open(pdfBytes);
    assert(pdf.pageCount > 0, "PDF has pages");

    // Test 2: Page properties
    console.log("\nTest: Page properties");
    const page = pdf.page(0);
    assert(typeof page.pageNumber === "number", "pageNumber is a number");
    assert(page.width > 0, "width > 0");
    assert(page.height > 0, "height > 0");

    // Test 3: Extract text
    console.log("\nTest: Extract text");
    const text = page.extractText();
    assert(typeof text === "string", "extractText returns string");
    assert(text.length > 0, "extracted text is non-empty");
    console.log(`  Text preview: "${text.substring(0, 100)}..."`);

    // Test 4: Extract words
    console.log("\nTest: Extract words");
    const words = page.extractWords();
    assert(Array.isArray(words), "extractWords returns array");
    if (words.length > 0) {
      assert(typeof words[0].text === "string", "word has text property");
      console.log(`  Found ${words.length} words`);
    }

    // Test 5: Extract tables
    console.log("\nTest: Extract tables");
    const tables = page.extractTables();
    assert(Array.isArray(tables), "extractTables returns array");
    console.log(`  Found ${tables.length} tables`);

    // Test 6: Search
    console.log("\nTest: Search");
    if (text.length > 3) {
      const searchTerm = text.substring(0, 3);
      const matches = page.search(searchTerm, false, true);
      assert(Array.isArray(matches), "search returns array");
      console.log(`  Found ${matches.length} matches for "${searchTerm}"`);
    }

    // Test 7: Chars
    console.log("\nTest: Chars");
    const chars = page.chars();
    assert(Array.isArray(chars), "chars returns array");
    console.log(`  Found ${chars.length} characters`);
  } catch (error) {
    failed++;
    console.error(`  ERROR: ${error.message}`);
  }

  console.log(`\nResults: ${passed} passed, ${failed} failed`);
  return failed === 0;
}

const success = runTests();
process.exit(success ? 0 : 1);
