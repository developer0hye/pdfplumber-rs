# Release artifact integrity

Tagged releases publish one checked integrity set for the verified Rust release
archives, Python wheels, Python source distribution, and five native
Command-Line Interface archives. Publication is blocked unless every produced
subject belongs to exactly one build group and all four artifact families are
present.

The four families are release archives, wheels, one source distribution, and
native Command-Line Interface binaries.

## Published evidence

Every build group retains:

- the exact release subjects produced by that runner;
- a Syft-generated SPDX 2.3 Software Bill of Materials for the group;
- a deterministic group manifest binding the source commit, optional release
  tag, subject sizes and SHA-256 digests, and exact SBOM bytes;
- a GitHub Actions build-provenance attestation and portable Sigstore bundle;
  and
- an SPDX SBOM attestation and portable Sigstore bundle.

The final aggregation rejects missing, repeated, changed, or unregistered
subjects before crates.io, PyPI, or the GitHub Release can publish. It emits
`SHA256SUMS` for release subjects only and `release-artifacts.json` for the
complete group, SBOM, provenance, and attestation inventory. Tagged GitHub
Releases attach the subjects and all integrity assets. PyPI receives the exact
attested wheel and source-distribution files downloaded from the same workflow
run.

Pull requests build the same subjects, SPDX documents, group manifests,
checksums, and index but do not sign test artifacts. A qualifying push to
`main` creates the provenance and SBOM attestations with GitHub-issued
short-lived identity, then requires the retained Sigstore bundles during final
aggregation.

## Verify a downloaded release

Download the release into one directory and verify every listed subject before
execution or installation:

```bash
gh release download v0.3.0 --repo developer0hye/pdfplumber-rs --dir release
(cd release && shasum -a 256 --check SHA256SUMS)
```

Verify build provenance for any release archive, wheel, source distribution,
or native Command-Line Interface binary archive:

```bash
gh attestation verify release/<artifact> \
  --repo developer0hye/pdfplumber-rs
```

Verify the artifact's signed SPDX predicate separately:

```bash
gh attestation verify release/<artifact> \
  --repo developer0hye/pdfplumber-rs \
  --predicate-type https://spdx.dev/Document/v2.3
```

The matching `*.spdx.json`, `*.provenance.sigstore.json`, and
`*.sbom.sigstore.json` assets preserve the exact release evidence outside the
online verification command. `release-artifacts.json` identifies the build
group and integrity filenames for each subject.

## Boundaries

An attestation proves which GitHub workflow, repository, commit, event, and
runner identity produced an artifact; it does not prove that the artifact is
secure, reproducible, compatible with an untested platform, or semantically
correct. SBOMs are group-scoped dependency inventories rather than a claim
that every binary embeds every listed component.

The attested `.crate` files are the exact verified preflight copies attached to
the GitHub Release. Cargo performs its own package operation during registry
publication; exact post-publication registry download and execution remain
open under [`DIST-007`](../PRD.md#824-p1--distribution-and-installation).
The npm/WebAssembly package is outside the four `DIST-005` artifact families;
its build and public installation boundaries remain separately tracked.
