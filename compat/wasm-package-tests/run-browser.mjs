import { chromium } from "@playwright/test";

const url = process.argv[2];
if (url === undefined) {
  throw new Error("usage: run-browser.mjs URL");
}

const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto(url, { waitUntil: "networkidle" });
  await page.locator('body[data-wasm-status="passed"]').waitFor();
  if (pageErrors.length !== 0) {
    throw new Error(`browser page errors: ${pageErrors.join("; ")}`);
  }
  const serialized = await page.locator("#result").textContent();
  if (serialized === null) {
    throw new Error("browser result was not rendered");
  }
  console.log(
    JSON.stringify({
      browser: `Chromium ${browser.version()}`,
      result: JSON.parse(serialized),
    }),
  );
} finally {
  await browser.close();
}
