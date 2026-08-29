# Compatibility terminology

This policy defines the compatibility language used by `pdfplumber-rs` for its Python migration surface. It does not redefine Cargo Semantic Versioning compatibility, platform support, package installability, or the maturity of the Rust, Command-Line Interface, and WebAssembly surfaces.

The fixed reference is Python `pdfplumber` `0.11.10` at commit `7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62`, as recorded by the [pinned upstream target](../../compat/upstream.toml). Changing that reference requires a separate target-upgrade review; a newer upstream release is not silently substituted.

The [machine-readable scorecard](scorecard-v0.3.0.json) retains individual observations, and the [human workflow scorecard](workflows-v0.3.0.md) groups those observations without turning them into a blanket package claim.

The versioned [Python-release compatibility matrix](python-release-matrix-v0.3.0.md) keeps those observations attached to the exact upstream release that produced them. A scorecard for one release is never evidence for an older, newer, or unlisted release.

## Compatible

**Compatible** means that observed behavior matches the pinned Python reference exactly within a fully named scope. It is not a synonym for “implemented,” “builds,” “installs,” “usually works,” or “resembles the upstream API.”

Every compatibility claim must identify these fields:

| Field | Required meaning |
| --- | --- |
| `reference` | Exact upstream project, version, and revision. |
| `surface` | Python package, module, class, function, method, property, or Command-Line Interface surface being compared. |
| `operation` | The invoked behavior and its observable result or failure contract. |
| `options` | Positional and keyword arguments, defaults, and relevant configuration. |
| `input` | Exact fixture or input class, including page when the observation is page-scoped. |
| `environment` | Interpreter, operating system, architecture, and other runtime facts material to the result. |
| `artifact` | Source checkout, wheel, source distribution, or other candidate bytes actually exercised. |

An omitted field is untested, not inferred from a nearby result. A package build, import smoke test, or successful extraction is useful installation evidence but does not by itself establish API or output compatibility.

At observation level, **Exact** means the reference and candidate have the same normalized outcome, value structure, ordering, runtime-relevant types, and failure behavior covered by that contract. An unqualified **compatible** claim requires every observation in its named scope to be **Exact**.

A scope containing an approved deviation may be described only as **compatible with approved deviations**, with every deviation named and linked beside the claim. It must not be shortened to an unqualified compatible or drop-in-compatible claim.

Unsupported, Reference failure, Candidate failure, and Not tested are never compatible results.

“Drop-in compatible” is a package-wide claim. It additionally requires the declared public module and export tree, signatures, types, descriptors, errors, lifecycle behavior, side effects, outputs, supported options, and required installed-artifact matrix to satisfy their gates. The current alpha Python package does not make that claim.

## Extension

An **extension** is an intentionally additive public capability that is absent from the pinned Python reference. It belongs outside the strict compatibility surface, is reached through an explicit namespace, and carries its own maturity, documentation, and tests.

An extension never counts as parity evidence, cannot fill an untested compatibility cell, and cannot hide or compensate for an incompatible reference behavior. Adding three Rust-only conveniences, for example, does not offset one mismatched Python method. An extension that accepts or returns compatibility objects must preserve their declared schemas rather than leaking extra fields into strict Python dictionaries.

This product term is distinct from a Python native extension module such as `pdfplumber._native`: the latter is a packaging term for compiled code. A compiled module may implement compatible behavior, extensions, or both; its file type proves none of those semantic outcomes.

## Approved deviation

**Approved deviation** and the scorecard label **Approved delta** mean the same thing. It is one exact, intentional difference from the pinned reference that a maintainer has reviewed and accepted for a stated reason, risk, regression test, and review condition.

The canonical [approved-deviation registry](../../compat/approved_deltas.toml) is bound to the same upstream version and commit. Every entry identifies all of these fields:

| Identity and observation | Review record |
| --- | --- |
| `fixture`, `page`, `api` | `technical_reason` |
| `upstream_result`, `upstream_sha256` | `compatibility_risk` |
| `rust_result`, `rust_sha256` | `approving_maintainer` |
| Exact target version and commit | `regression_test`, `review_condition` |

Wildcards are not supported. The fixture, page, API, and both result digests must match the observed difference exactly. A changed observation is unregistered, an unused entry is stale, and either condition fails the gate until it is reviewed explicitly.

An approval does not turn a difference into **Exact**, erase the compatibility risk, or authorize neighboring differences. Approved deviations remain reported separately from Exact and are never folded into a success percentage. The committed registry is currently empty; that means no difference is approved, not that every known difference is acceptable.

## Reading the scorecard

The scorecard vocabulary is exhaustive for each recorded observation:

| Outcome | What it permits a reader to conclude |
| --- | --- |
| **Exact** | The compared observation matches within its recorded scope. |
| **Approved delta** | The exact difference matches a live approved-deviation entry; inspect its risk and review condition. |
| **Unsupported** | The candidate explicitly does not implement the requested behavior. |
| **Reference failure** | The pinned reference did not produce a comparable result. |
| **Candidate failure** | The candidate errored or differed without a matching approval. |
| **Not tested** | No comparison ran for the required scope. |

Counts describe retained evidence; they are not a success percentage. A workflow with observed exact cases may still contain failures or untested cells, and a result from one platform or artifact does not transfer to another. Consult the separate support and readiness documents for release-maturity claims.

The separation of run metadata, individual observations, and explicit untested outcomes follows the repository's [wpt.fyi and EARL reference review](../../references/compatibility-scorecards.md). This project retains its domain-specific exact, deviation, unsupported, failure, and not-tested vocabulary.
