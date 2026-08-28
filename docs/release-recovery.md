# Release recovery and partial publication

This runbook contains a tagged release without pretending that published bytes
can be rolled back atomically. Registry records, release assets, and the exact
tag/commit remain the audit trail. Prefer a forward fix at a new version; use a
yank or deprecation to steer users away from a bad version.

Never move or recreate the release tag, reuse a published version, overwrite
evidence silently, or weaken a compatibility gate to make a release look
complete.

## Release topology and first response

The crates.io dependency chain is `pdfplumber-core` → `pdfplumber-parse` →
`pdfplumber` → `pdfplumber-cli` → GitHub Release. The PyPI `pdfplumber-rs` and
npm `pdfplumber-wasm` branches are independent of the crates.io chain and of
one another after the shared Continuous Integration, metadata, and scorecard
gates. A green or failed job is therefore not proof of every registry's state.

1. Freeze new publication and cancel the active run with
   `gh run cancel <run-id>`. Record any job that was already in progress.
2. Do not re-run the whole workflow: immutable versions that already landed
   will fail as duplicates while independent jobs may publish more artifacts.
3. Open the incident record below and query every registry. Treat timeouts and
   missing evidence as `unknown`, not `not published`.
4. Announce that the release is incomplete until every expected target is
   verified or explicitly contained.

## Incident record

Create a private incident note if credential material may be involved. Record
facts and UTC timestamps, never a token or secret value.

- release tag and intended version
- exact commit SHA and clean-tree confirmation
- workflow run URL, job URL, and first observed UTC timestamp
- incident owner and decision approver
- trigger, observed error, and last known safe action
- artifact digest, attestation, and scorecard digest where available
- target, expected version, observed state, query, response, and observed UTC

Use only these target states: `not published`, `published`, `withdrawn`
(yanked, deprecated, or removed), and `unknown`. Track crates.io packages
individually and also record PyPI, npm, the GitHub Release, tag, and each release
asset. Append corrections; do not rewrite earlier observations.

## Registry containment and verification

Run queries from a clean environment and save their output in the incident
record. A registry page alone may be stale, so verify with its client or API.

| Target | Verify the exact version | Contain a bad version |
|---|---|---|
| crates.io | `cargo info <crate>@<version>` for each of the four crates | `cargo yank <crate>@<version>`; restore only after review with `cargo yank --undo <crate>@<version>` |
| PyPI | `curl -fsS https://pypi.org/pypi/pdfplumber-rs/<version>/json` and inspect every expected file | Use `https://pypi.org/manage/project/pdfplumber-rs/releases/` to yank the whole release and give a reason |
| npm | `npm view "pdfplumber-wasm@<version>" version --registry https://registry.npmjs.org` | `npm deprecate "pdfplumber-wasm@<version>" "<reason and replacement>"` |
| GitHub | `gh release view <tag>` and `git ls-remote --tags origin refs/tags/<tag>` | Correct the release notes and affected assets if mutable; otherwise publish a clearly linked correction |

Cargo yank does not delete crate data: existing lockfiles and direct downloads
can still use it. PyPI yank is a non-destructive alternative to deletion and an
exact pin can still select the release. npm unpublish is irreversible,
policy-gated, and never makes the version reusable; deprecate by default.
Never try to reuse the same version; publish a new version for corrected bytes.

GitHub release deletion, asset deletion, or tag deletion does not retract any
registry package. Immutable GitHub releases also prevent editing assets and the
tag, so publish a correction instead of destroying the provenance chain.

## Registry lag

Freeze the dependent publish step; do not publish downstream packages while an
upstream state is `unknown`. Use a bounded poll with a recorded UTC deadline
and query the exact endpoint above until the expected version is resolvable.
Back off between queries and consult the registry status page; do not convert a
timeout into proof that upload failed.

Resume only the blocked step after the exact version and digest or file set are
visible. If the deadline expires, cancel publication, classify the release as
partial, and follow **One-package failure**. Close this scenario only after all
independent targets have also been queried.

## One-package failure

Stop all downstream publication and cancel still-running independent jobs when
continued publication would widen the incident. Build the target-state table
from registry queries; job output cannot override an already published record.

Do not republish an already published version. If the candidate is still valid
and the failure is transient, resume only missing targets from a clean checkout
of the exact tag, preserving crates.io dependency order and waiting for each
predecessor to resolve before its dependent. If bytes or metadata are wrong,
do not publish the remaining targets: prepare a new coordinated version, then
yank or deprecate the bad version with replacement guidance. When possible,
publish the compatible crates.io replacement before yanking its predecessor.

Exit partial-release mode only when every target is `published`, `withdrawn`,
or intentionally `not published`, with a public incomplete-release notice and
a linked recovery version. Never call a mixed or `unknown` state complete.

## Compromised credentials

Cancel active runs, freeze all publication, and revoke the credential at its
provider immediately. Deleting a GitHub Actions secret does not revoke the
provider credential. Replace the repository secret only after provider
revocation; use a new least-privilege or identity-based credential.

Audit the compromise scope: Actions logs and runs, GitHub security/authentication
events, registry publication history, package owners, new versions, yanks,
deprecations, release/tag changes, and the credential's issue and last-used
times. Treat any unauthorized package as a supply-chain incident and follow the
registry's security/support process in addition to this runbook.

Resume publication only after revocation is confirmed, the audit scope is
bounded, unexpected changes are contained, and a clean exact-tag candidate has
passed every gate again. Close with rotation evidence and incident timestamps,
but never record the old or replacement credential itself.

## Incorrect compatibility claim

Stop the release and do not publish further artifacts based on the claim.
Preserve the original scorecards, checksums, release text, run URL, and raw
evidence before changing any public presentation. Reproduce the discrepancy
against the pinned compatibility target; do not weaken checks, exclusions, or
unsupported classifications to obtain a preferred result.

If the GitHub Release is mutable, remove the misleading asset from public use
and edit the notes with an explicit correction that links the incident and
replacement evidence. Do not overwrite or replace silently. If the release is
immutable, publish a separate correction and link it from every mutable project
surface. Published package metadata or bytes require a new version; yank the
PyPI/crates.io release or deprecate npm when the false claim makes use unsafe.

Resume or close only after corrected evidence passes the unchanged gates, the
old claim is visibly withdrawn, affected registries point to the replacement,
and users have an actionable upgrade or avoidance path.

## Close the incident

Before declaring recovery complete, independently re-query every target, run
installed-artifact checks when available, verify the tag and commit, and attach
the final state table and command output to the incident. Publish a concise
correction or postmortem that names impact, affected versions, containment,
replacement, and remaining unknowns without exposing credentials.

Link the incident from the relevant release notes and changelog. Convert any
missing automation into a follow-up task; do not describe a manual recovery as
an automated guarantee.

The registry and credential semantics used here are summarized in the
[release-recovery reference note](../references/release-recovery.md).
