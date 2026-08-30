# Post-publish verification

The release workflow treats publication and usability as separate facts. After
uploading, it resolves the exact version from the public registries, installs it
in a new temporary consumer, and exercises the repository's hash-bound
one-page PDF. A package is not considered released merely because its upload
job returned success.

## Public checks

The three checks run independently on Ubuntu x86-64 after their matching
publisher succeeds:

- crates.io: wait for `pdfplumber-cli@<version>`, then run `cargo install` with
  the exact version, the `crates-io` registry, the published lockfile, and a new
  install root. Running the installed `pdfplumber` binary must produce the
  byte-exact JSON fixture output. This installation also resolves the published
  `pdfplumber`, `pdfplumber-parse`, and `pdfplumber-core` dependency chain.
- PyPI: wait for the exact `pdfplumber-rs` release JSON, create an isolated
  CPython 3.13 environment, and install only a matching public wheel from the
  PyPI index. Both the distribution and native-module versions, import origins,
  page count, and exact extracted text must match.
- npm: wait for the exact `pdfplumber-wasm` version, install it from the public
  npm registry into a new consumer, type-check and bundle that installed
  package, and assert the exact PDF output in the maintained Chromium runner.
  This follows the same installed-consumer pattern as the
  [prepublication WebAssembly gate](wasm-package-testing.md).

Every registry lookup uses bounded exponential backoff with a ten-minute
deadline. Install commands name public registry endpoints and exact versions;
they do not accept checkout paths, local archives, or registry caches as a
substitute.

## Fail-closed release state

The GitHub Release waits for all three post-publish checks. If visibility,
installation, import, execution, or exact output fails, the workflow fails and
does not create the GitHub Release. Registry uploads that already succeeded are
immutable external state, so the version is immediately considered incomplete;
follow the [release recovery runbook](release-recovery.md) instead of rerunning
the entire workflow or reusing the version.

Successful and failed checks retain JSON evidence with the release tag, source
commit, exact version, attempt count, and smoke result. A real tagged run must
produce all three passing reports before `DIST-007` can be checked. This single
post-publication host proves registry installation and exact behavior; the
separate prepublication matrices remain the authority for broader platform
coverage.

## Reproduction

From a clean checkout of the exact tag, with each surface's required tools
installed, run one family at a time:

```console
python scripts/check_public_registry_release.py crates --release-tag vX.Y.Z --timeout-seconds 600 --output dist/postpublish-crates.json
python scripts/check_public_registry_release.py pypi --release-tag vX.Y.Z --timeout-seconds 600 --output dist/postpublish-pypi.json
python scripts/check_public_registry_release.py npm --release-tag vX.Y.Z --timeout-seconds 600 --output dist/postpublish-npm.json
```

The tag must equal the workspace version exactly. The npm check additionally
uses the locked tools in `compat/wasm-package-tests` and their matching
Playwright Chromium installation.
