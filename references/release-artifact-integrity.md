# Release artifact integrity sources

Official sources used for the checksum, SBOM, provenance, and attestation gate.

- [GitHub artifact attestation guidance](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations)
  requires `id-token: write` and `attestations: write`, shows build-provenance
  and SBOM predicates, and documents `gh attestation verify`.
- [`actions/attest`](https://github.com/actions/attest) is the current unified
  GitHub action. Version 4 supports multiple file subjects, SPDX or CycloneDX
  SBOM predicates, and portable Sigstore bundle output.
- [GitHub's attestation model](https://docs.github.com/en/actions/concepts/security/artifact-attestations)
  records workflow, repository, commit, event, and OpenID Connect identity;
  GitHub cautions that this linkage is not a security guarantee by itself.
- [GitHub's SBOM export guidance](https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/establish-provenance-and-integrity/export-dependencies-as-sbom)
  lists SPDX as the standard format and Anchore's Syft-backed action as a
  supported Actions generator.
- [Anchore SBOM Action](https://github.com/anchore/sbom-action/blob/main/README.md)
  supports directory/file scans, explicit SPDX JSON output, and disabling its
  implicit artifact or release upload so this repository controls publication.

This repository groups subjects by the job that built them, binds every group
to exact SPDX bytes, and aggregates only after all subjects and attestations
reconcile. Pull requests remain unsigned; tagged releases and qualifying
`main` pushes use GitHub's short-lived Sigstore identity.
