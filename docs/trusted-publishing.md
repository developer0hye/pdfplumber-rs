# Trusted release publishing

The tagged release workflow uses identity-based credentials instead of stored
publication secrets. GitHub issues an OpenID Connect identity only to the
publishing job; crates.io, PyPI, or npm validates that identity and returns a
short-lived credential for the one job. GitHub Releases use the separate,
job-scoped `GITHUB_TOKEN`, which expires with the job.

## Exact trust bindings

Each registry binding must match case-sensitively. Enter only `release.yml` as
the workflow filename, not its `.github/workflows/` path.

| Target | Registry project | GitHub owner/repository | Workflow | Environment | Allowed operation |
|---|---|---|---|---|---|
| crates.io | `pdfplumber-core`, `pdfplumber-parse`, `pdfplumber`, and `pdfplumber-cli` (one binding per crate) | `developer0hye/pdfplumber-rs` | `release.yml` | `crates-io` | publish |
| PyPI | `pdfplumber-rs` | `developer0hye/pdfplumber-rs` | `release.yml` | `pypi` | publish |
| npm | `pdfplumber-wasm` | `developer0hye/pdfplumber-rs` | `release.yml` | `npm` | `npm publish` |
| GitHub | this repository | automatic repository identity | `release.yml` | none | create the GitHub Release and upload assets |

The repository environments carry no package-registry password. They narrow
the identity asserted to each registry, and each must use a custom deployment
tag policy of `v*.*.*` to match the release trigger. The npm job pins Node
24.5.0, whose bundled npm 11.5.1 is the minimum release with npm trusted
publishing support. The crates.io action automatically revokes its temporary
token after the job. PyPI's publishing action and npm perform their own OpenID
Connect exchanges without an explicit password or token.

## Configure, verify, then revoke

1. Configure all four crates.io projects, the PyPI project, and the npm package
   with the exact bindings above. Registry publisher configuration is private
   account state and is not publicly visible or proved by this source tree.
2. Verify the three GitHub environments and registry entries independently.
   Keep a private maintainer record of the observed settings without copying
   tokens, session data, or recovery codes.
3. Merge only after the workflow contract and ordinary Continuous Integration
   pass. Do not create a tag or publish an immutable version merely to test the
   identity exchange; the next intended release is the live verification.
4. On that release, verify each published version and the GitHub Release using
   the commands in the [release recovery runbook](release-recovery.md). A green
   job alone is not registry evidence.
5. After every target has succeeded through trusted publishing, revoke the old
   registry automation tokens at crates.io, PyPI, and npm, then delete the
   unused GitHub Actions secrets. Deleting a repository secret does not revoke
   its provider credential.

There is no secret fallback in `release.yml`. If a registry rejects the
identity, cancel the release, preserve each target's actual state, correct the
registry binding, and resume only the missing target according to the recovery
runbook. Never restore a broad long-lived token just to make a release green.

## Evidence boundary

Local and pull-request checks can prove permission scope, exact identities,
minimum client versions, and the absence of secret references without
publishing. They cannot prove private registry configuration or a future token
exchange. [`DIST-006`](../PRD.md#824-p1--distribution-and-installation) remains
open until those bindings are independently verified; public-registry install
and execution remain the separate [`DIST-007`](../PRD.md#824-p1--distribution-and-installation)
gate.
