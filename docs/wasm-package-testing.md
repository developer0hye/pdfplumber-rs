# WebAssembly package prepublication testing

The npm surface is experimental, but a release candidate must behave as an installable package before the tagged workflow can publish it. The reusable Continuous Integration workflow is a dependency of the npm publication job and exercises the exact source commit on Ubuntu Linux x86-64.

## Locked boundary

`wasm-package-test-policy.toml` pins Node.js 24.20.0, wasm-pack 0.15.0, TypeScript 7.0.2, Vite 8.2.2, Playwright 1.62.1, and Playwright's Chromium runner. The committed `compat/wasm-package-tests/package-lock.json` resolves the JavaScript test tools. Continuous Integration installs only that lock before downloading the Chromium binary associated with the pinned Playwright release.

The gate creates two candidates from the same Rust source:

- the `nodejs` wasm-pack target for the Node consumer;
- the `bundler` wasm-pack target, which is also the npm publication target, for the browser consumer.

Both packages receive the checked hand-authored declarations before validation. The checker runs `npm pack`, installs each fresh npm package archive into a separate empty consumer, and never imports either package from the repository source directory.

Before packaging, the checker requires `--source-commit` to equal the checkout `HEAD` and rejects any tracked or untracked worktree change. The retained report therefore cannot label package bytes from a different or dirty source tree as the requested commit.

## Required proof

The Node consumer and browser consumer are strict TypeScript programs. `tsc` checks each consumer before Node executes the compiled program or Vite bundles the browser program. Both runtimes open `tests/fixtures/generated/basic_text.pdf` and require one page plus byte-exact extracted text from `tests/fixtures/expected/cli-release-basic-text.jsonl`.

Playwright starts its pinned maintained Chromium build against a local static server. A browser-side exception, missing package load, WebAssembly instantiation failure, page-count drift, or text drift fails the job. No skipped fixture or non-empty-output substitute is accepted.

The job retains the Node and browser package archives plus `wasm-package-report.json`. The report binds the source commit, policy, tool versions, browser version, archive bytes, WebAssembly binaries, fixture, expected record, exact extracted text, and both runtime results by SHA-256.

## Reproduce

Install the locked tools and browser, build both candidates, copy the declarations, and execute the checker:

```bash
npm ci --prefix compat/wasm-package-tests --ignore-scripts
npx --prefix compat/wasm-package-tests playwright install chromium
wasm-pack build --target nodejs --out-dir pkg-node crates/pdfplumber-wasm
wasm-pack build --target bundler --out-dir pkg-browser crates/pdfplumber-wasm
cp crates/pdfplumber-wasm/pdfplumber-wasm.d.ts crates/pdfplumber-wasm/pkg-node/pdfplumber_wasm.d.ts
cp crates/pdfplumber-wasm/pdfplumber-wasm.d.ts crates/pdfplumber-wasm/pkg-browser/pdfplumber_wasm.d.ts
python3 scripts/check_wasm_package.py \
  --node-package crates/pdfplumber-wasm/pkg-node \
  --browser-package crates/pdfplumber-wasm/pkg-browser \
  --source-commit "$(git rev-parse HEAD)" \
  --output dist/wasm-package-report.json
```

Run the command from a clean checkout at the supplied source commit. Use the exact Node.js version and wasm-pack version from the policy. Linux Continuous Integration uses `playwright install --with-deps chromium` so the runner also has the required system libraries.

## Claim boundary

This proves fresh candidate-archive installation, strict type-checking, and exact fixture execution in Node.js 24.20.0 and the Chromium build shipped with Playwright 1.62.1. It does not prove compatibility with every browser, browser version, operating system, bundler, or application configuration. It also does not replace post-publication installation from npm under `DIST-007`, performance/resource gates, or the package's experimental maturity label.
