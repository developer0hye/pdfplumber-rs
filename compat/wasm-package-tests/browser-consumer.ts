import { WasmPdf } from "pdfplumber-wasm";

async function sha256(value: Uint8Array | string): Promise<string> {
  const bytes: Uint8Array<ArrayBuffer> =
    typeof value === "string" ? new TextEncoder().encode(value) : Uint8Array.from(value);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

async function run(): Promise<void> {
  const [fixtureResponse, expectedResponse] = await Promise.all([
    fetch("/document.pdf"),
    fetch("/expected.jsonl"),
  ]);
  if (!fixtureResponse.ok || !expectedResponse.ok) {
    throw new Error("browser fixture inputs were not served");
  }

  const fixture = new Uint8Array(await fixtureResponse.arrayBuffer());
  const expectedBytes = new Uint8Array(await expectedResponse.arrayBuffer());
  const expectedRecord = JSON.parse(new TextDecoder().decode(expectedBytes)) as {
    page: number;
    text: string;
  };
  const pdf = WasmPdf.open(fixture);
  const page = pdf.page(0);
  const text = page.extractText();
  if (pdf.pageCount !== 1 || expectedRecord.page !== 1 || text !== expectedRecord.text) {
    throw new Error("browser candidate did not produce the exact one-page fixture output");
  }

  const result = {
    runtime: "browser",
    page_count: pdf.pageCount,
    text_sha256: await sha256(text),
    fixture_sha256: await sha256(fixture),
    expected_sha256: await sha256(expectedBytes),
  };
  const output = document.querySelector<HTMLElement>("#result");
  if (output === null) {
    throw new Error("missing browser result element");
  }
  output.textContent = JSON.stringify(result);
  document.body.dataset.wasmStatus = "passed";
}

void run().catch((error: unknown) => {
  document.body.dataset.wasmStatus = "failed";
  document.body.textContent = error instanceof Error ? error.stack ?? error.message : String(error);
});
