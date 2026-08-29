import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { WasmPdf } from "pdfplumber-wasm";

function sha256(value: Uint8Array | string): string {
  return createHash("sha256").update(value).digest("hex");
}

const fixturePath = process.argv[2];
const expectedPath = process.argv[3];
if (fixturePath === undefined || expectedPath === undefined) {
  throw new Error("usage: node-consumer.js FIXTURE EXPECTED_JSONL");
}

const fixture = readFileSync(fixturePath);
const expectedBytes = readFileSync(expectedPath);
const expectedRecord = JSON.parse(expectedBytes.toString("utf8")) as {
  page: number;
  text: string;
};
const pdf = WasmPdf.open(fixture);
const page = pdf.page(0);
const text = page.extractText();
if (pdf.pageCount !== 1 || expectedRecord.page !== 1 || text !== expectedRecord.text) {
  throw new Error("Node candidate did not produce the exact one-page fixture output");
}

console.log(
  JSON.stringify({
    runtime: "node",
    page_count: pdf.pageCount,
    text_sha256: sha256(text),
    fixture_sha256: sha256(fixture),
    expected_sha256: sha256(expectedBytes),
  }),
);
