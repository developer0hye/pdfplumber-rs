# WebAssembly package test references

Primary sources used to define the prepublication package-consumer gate.

## Node.js

- [Node.js release schedule](https://nodejs.org/en/about/previous-releases) identifies supported release lines and recommends Active or Maintenance Long-Term Support releases for production applications. The repository pins the current Node.js 24 Long-Term Support patch in `wasm-package-test-policy.toml` instead of using an end-of-life odd-numbered host release.

## wasm-pack

- [wasm-pack build documentation](https://rustwasm.github.io/docs/wasm-pack/print.html) defines `bundler` as the package form with an ES module entry for a bundler and `nodejs` as the CommonJS package form for Node.js. DIST-013 builds and installs both rather than treating one target as proof for the other.
- [wasm-pack-action](https://github.com/marketplace/actions/wasm-pack-action) exposes an explicit version input. Continuous Integration pins that input to the policy version.
- [wasm-pack v0.15.0](https://github.com/wasm-bindgen/wasm-pack/releases/tag/v0.15.0) is the maintained release used by the gate; it replaces deprecated installation dependencies and the stale pre-move download path in 0.14.0.

## TypeScript and Vite

- [Playwright TypeScript guidance](https://playwright.dev/docs/test-typescript) states that Playwright transforms TypeScript but does not type-check it and recommends running `tsc --noEmit` separately. The gate type-checks both consumers before browser execution.
- [Vite WebAssembly documentation](https://vite.dev/guide/features.html#webassembly) describes WebAssembly ES module integration and asynchronous instantiation. The browser candidate uses the `bundler` package through the pinned Vite release rather than importing repository files directly.

## Playwright and Chromium

- [Playwright browser management](https://playwright.dev/docs/browsers) states that each Playwright version requires matching browser binaries, supports installing Chromium explicitly, and recommends regular Playwright updates to test recent browser builds.
- [Playwright command line](https://playwright.dev/docs/test-cli) documents `playwright install --with-deps chromium`, which the Linux job uses to install the pinned Chromium build and its operating-system dependencies.

## Local policy decisions

- Exact patches are locked so a passing report identifies the tested tool and browser family instead of inheriting a moving `latest` tag.
- A real fixture and byte-exact text assertion are required in both runtimes; a successful import or non-empty string is insufficient.
- The npm publication target is the browser candidate. The additional Node target proves the documented Node surface without changing the published package format.
