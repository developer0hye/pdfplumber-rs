"""Pinned competitor suite and fail-closed local benchmark runner."""

from __future__ import annotations

import json
import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

import tomllib

from compat.harness import benchmark_corpus, benchmark_equivalence

SHA_PATTERN = re.compile(r"^[0-9a-f]{40}$")
ID_PATTERN = re.compile(r"^[a-z0-9]+(?:[.-][a-z0-9]+)*$")
REQUIRED_IMPLEMENTATIONS = {
    "pdfplumber-python": (
        "https://github.com/jsvine/pdfplumber",
        "7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
    ),
    "pdf-oxide": (
        "https://github.com/yfedoseev/pdf_oxide",
        "3be1951b171edb9d69a10f42ef72ee73f52e51bf",
    ),
    "pdfsink-rs": (
        "https://github.com/clark-labs-inc/pdfsink-rs",
        "980d9f7b8ec44456f3d54427f4ced747b6eb6154",
    ),
    "pdfplumber-rs": (
        "https://github.com/developer0hye/pdfplumber-rs",
        "repository-head",
    ),
}
REQUIRED_WORKLOADS = ("document-open", "text")
REFERENCE_IMPLEMENTATION = "pdfplumber-python"
CANDIDATE_IMPLEMENTATION = "pdfplumber-rs"
COMPETITOR_IMPLEMENTATIONS = ("pdf-oxide", "pdfsink-rs")


class CompetitorBenchmarkError(ValueError):
    """The competitor suite or a local run violates the benchmark contract."""


@dataclass(frozen=True)
class CompetitorImplementation:
    """One pinned implementation and its argv-only adapter command."""

    id: str
    display_name: str
    repository: str
    revision: str
    license: str
    source_reference: str
    command: tuple[str, ...]
    workloads: tuple[str, ...]


@dataclass(frozen=True)
class CompetitorCase:
    """One digest-bound implementation, fixture, and workload request."""

    implementation_id: str
    implementation_revision: str
    fixture_id: str
    fixture_path: str
    fixture_sha256: str
    fixture_password: str | None
    workload_id: str


@dataclass(frozen=True)
class CompetitorSuite:
    """Validated pinned suite bound to the corpus and equivalence policy."""

    id: str
    release: str
    corpus_id: str
    policy_id: str
    reference_implementation: str
    candidate_implementation: str
    rust_manifest: str
    rust_binary: str
    python_adapter: str
    implementations: tuple[CompetitorImplementation, ...]
    corpus: benchmark_corpus.BenchmarkCorpus
    policy: benchmark_equivalence.EquivalencePolicy

    def implementation(self, implementation_id: str) -> CompetitorImplementation:
        """Return one named implementation."""

        for implementation in self.implementations:
            if implementation.id == implementation_id:
                return implementation
        raise CompetitorBenchmarkError(
            f"unknown benchmark implementation: {implementation_id}"
        )


def audit_repository(
    repo_root: Path,
    suite_path: Path,
    corpus_path: Path,
    policy_path: Path,
    registry_path: Path,
) -> CompetitorSuite:
    """Validate pins, adapters, corpus cases, and request semantics."""

    corpus = benchmark_corpus.audit_repository(repo_root, corpus_path, registry_path)
    policy = benchmark_equivalence.audit_repository(
        repo_root,
        policy_path,
        corpus_path,
        registry_path,
    )
    try:
        with suite_path.open("rb") as suite_file:
            source = tomllib.load(suite_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise CompetitorBenchmarkError(
            f"cannot read competitor suite: {suite_path}"
        ) from error
    return validate_suite(source, repo_root, corpus, policy)


def validate_suite(
    source: Mapping[str, object],
    repo_root: Path,
    corpus: benchmark_corpus.BenchmarkCorpus,
    policy: benchmark_equivalence.EquivalencePolicy,
) -> CompetitorSuite:
    """Validate one parsed competitor-suite manifest."""

    schema = source.get("schema")
    if not isinstance(schema, dict) or schema.get("version") != 1:
        raise CompetitorBenchmarkError("schema.version must be 1")
    raw_suite = _required_mapping(source, "suite", "manifest")
    suite_id = _required_string(raw_suite, "id", "suite")
    if not ID_PATTERN.fullmatch(suite_id):
        raise CompetitorBenchmarkError(f"invalid suite id: {suite_id}")
    release = _required_string(raw_suite, "release", "suite")
    corpus_id = _required_string(raw_suite, "corpus_id", "suite")
    policy_id = _required_string(raw_suite, "policy_id", "suite")
    reference_implementation = _required_string(
        raw_suite, "reference_implementation", "suite"
    )
    candidate_implementation = _required_string(
        raw_suite, "candidate_implementation", "suite"
    )
    if corpus_id != corpus.id:
        raise CompetitorBenchmarkError(
            f"suite corpus {corpus_id} does not match {corpus.id}"
        )
    if policy_id != policy.id:
        raise CompetitorBenchmarkError(
            f"suite policy {policy_id} does not match {policy.id}"
        )
    if release != corpus.release or release != policy.release:
        raise CompetitorBenchmarkError("suite, corpus, and policy releases must match")
    if reference_implementation != REFERENCE_IMPLEMENTATION:
        raise CompetitorBenchmarkError(
            f"reference implementation must be {REFERENCE_IMPLEMENTATION}"
        )
    if candidate_implementation != CANDIDATE_IMPLEMENTATION:
        raise CompetitorBenchmarkError(
            f"candidate implementation must be {CANDIDATE_IMPLEMENTATION}"
        )

    rust_manifest = _required_repository_file(
        raw_suite, "rust_manifest", "suite", repo_root
    )
    rust_binary = _required_repository_path(raw_suite, "rust_binary", "suite")
    python_adapter = _required_repository_file(
        raw_suite, "python_adapter", "suite", repo_root
    )

    raw_implementations = source.get("implementations")
    if not isinstance(raw_implementations, list) or not raw_implementations:
        raise CompetitorBenchmarkError("implementations must be a non-empty array")
    implementations = tuple(
        sorted(
            (
                _validate_implementation(raw, repo_root, policy)
                for raw in raw_implementations
                if isinstance(raw, dict)
            ),
            key=lambda implementation: implementation.id,
        )
    )
    if len(implementations) != len(raw_implementations):
        raise CompetitorBenchmarkError("each implementation must be one table")
    implementation_ids = [implementation.id for implementation in implementations]
    if len(set(implementation_ids)) != len(implementation_ids):
        raise CompetitorBenchmarkError("implementation ids must be unique")
    if set(implementation_ids) != set(REQUIRED_IMPLEMENTATIONS):
        missing = set(REQUIRED_IMPLEMENTATIONS) - set(implementation_ids)
        extra = set(implementation_ids) - set(REQUIRED_IMPLEMENTATIONS)
        details = []
        if missing:
            details.append("missing " + ", ".join(sorted(missing)))
        if extra:
            details.append("unknown " + ", ".join(sorted(extra)))
        raise CompetitorBenchmarkError(
            "implementation set is invalid: " + "; ".join(details)
        )

    cargo_source = (repo_root / rust_manifest).read_text(encoding="utf-8")
    for source_directory in ("../.sources/pdf_oxide", "../.sources/pdfsink-rs"):
        if source_directory not in cargo_source:
            raise CompetitorBenchmarkError(
                f"Rust adapter manifest does not name prepared source {source_directory}"
            )
    if (
        python_adapter
        not in implementations_by_id(implementations)[REFERENCE_IMPLEMENTATION].command
    ):
        raise CompetitorBenchmarkError(
            "Python command does not name the audited adapter"
        )
    if rust_binary not in {
        implementation.command[0]
        for implementation in implementations
        if implementation.id != REFERENCE_IMPLEMENTATION
    }:
        raise CompetitorBenchmarkError("Rust commands do not name the audited binary")

    return CompetitorSuite(
        id=suite_id,
        release=release,
        corpus_id=corpus_id,
        policy_id=policy_id,
        reference_implementation=reference_implementation,
        candidate_implementation=candidate_implementation,
        rust_manifest=rust_manifest,
        rust_binary=rust_binary,
        python_adapter=python_adapter,
        implementations=implementations,
        corpus=corpus,
        policy=policy,
    )


def _validate_implementation(
    source: Mapping[str, object],
    repo_root: Path,
    policy: benchmark_equivalence.EquivalencePolicy,
) -> CompetitorImplementation:
    implementation_id = _required_string(source, "id", "implementation")
    expected = REQUIRED_IMPLEMENTATIONS.get(implementation_id)
    if expected is None:
        raise CompetitorBenchmarkError(
            f"unknown benchmark implementation: {implementation_id}"
        )
    repository = _required_string(
        source, "repository", f"implementation {implementation_id}"
    )
    revision = _required_string(
        source, "revision", f"implementation {implementation_id}"
    )
    if (repository, revision) != expected:
        raise CompetitorBenchmarkError(
            f"implementation {implementation_id} source pin is not the approved revision"
        )
    if revision != "repository-head" and not SHA_PATTERN.fullmatch(revision):
        raise CompetitorBenchmarkError(
            f"implementation {implementation_id} needs a full Git revision"
        )
    display_name = _required_string(
        source, "display_name", f"implementation {implementation_id}"
    )
    license_name = _required_string(
        source, "license", f"implementation {implementation_id}"
    )
    source_reference = _required_repository_file(
        source,
        "source_reference",
        f"implementation {implementation_id}",
        repo_root,
    )
    command = _required_string_array(
        source, "command", f"implementation {implementation_id}"
    )
    if not command or any(
        "\n" in argument or "\x00" in argument for argument in command
    ):
        raise CompetitorBenchmarkError(
            f"implementation {implementation_id} command must be safe argv"
        )
    workloads = _required_string_array(
        source, "workloads", f"implementation {implementation_id}"
    )
    if workloads != REQUIRED_WORKLOADS:
        raise CompetitorBenchmarkError(
            f"implementation {implementation_id} workloads must be {REQUIRED_WORKLOADS}"
        )
    for workload_id in workloads:
        if policy.workload(workload_id) is None:
            raise CompetitorBenchmarkError(
                f"implementation {implementation_id} names unknown workload {workload_id}"
            )
    return CompetitorImplementation(
        id=implementation_id,
        display_name=display_name,
        repository=repository,
        revision=revision,
        license=license_name,
        source_reference=source_reference,
        command=command,
        workloads=workloads,
    )


def expand_cases(suite: CompetitorSuite) -> tuple[CompetitorCase, ...]:
    """Expand identical digest-bound cases for every implementation."""

    cases: list[CompetitorCase] = []
    for implementation in suite.implementations:
        for workload_id in implementation.workloads:
            workload = suite.policy.workload(workload_id)
            if workload is None:
                raise CompetitorBenchmarkError(f"unknown workload: {workload_id}")
            for fixture in suite.corpus.fixtures:
                if workload.semantic_classes != ("*",) and not set(
                    fixture.semantic_classes
                ).intersection(workload.semantic_classes):
                    continue
                cases.append(
                    CompetitorCase(
                        implementation_id=implementation.id,
                        implementation_revision=implementation.revision,
                        fixture_id=fixture.id,
                        fixture_path=fixture.path,
                        fixture_sha256=fixture.sha256,
                        fixture_password=fixture.password,
                        workload_id=workload_id,
                    )
                )
    return tuple(
        sorted(
            cases,
            key=lambda case: (
                case.fixture_id,
                case.workload_id,
                case.implementation_id,
            ),
        )
    )


def synthetic_record(
    *,
    implementation_id: str,
    revision: str,
    fixture_id: str,
    fixture_sha256: str,
    workload_id: str,
    outcome: Mapping[str, object],
) -> dict[str, object]:
    """Build one canonical record for an adapter-produced outcome."""

    if workload_id == "document-open":
        output_schema = "page-count-v1"
        request: dict[str, object] = {
            "operation": "open",
            "page_selection": "all",
            "password_source": "fixture-metadata",
            "repair": "disabled",
        }
    elif workload_id == "text":
        output_schema = "page-text-v1"
        request = {
            "layout": False,
            "normalization": "none",
            "page_selection": "all",
            "preserve_page_boundaries": True,
        }
    else:
        raise CompetitorBenchmarkError(f"unsupported suite workload: {workload_id}")
    return {
        "schema_version": 1,
        "implementation": {"id": implementation_id, "revision": revision},
        "fixture": {"id": fixture_id, "sha256": fixture_sha256},
        "workload": {"id": workload_id, "output_schema": output_schema},
        "request": request,
        "outcome": dict(outcome),
    }


def build_timing_plan(
    records: Mapping[str, Mapping[str, object]],
) -> tuple[tuple[str, str, str], ...]:
    """Select triples whose reference, candidate, and competitor outputs match."""

    reference = records.get(REFERENCE_IMPLEMENTATION)
    candidate = records.get(CANDIDATE_IMPLEMENTATION)
    if not _outcomes_match(reference, candidate):
        return ()
    triples = []
    for competitor_id in COMPETITOR_IMPLEMENTATIONS:
        competitor = records.get(competitor_id)
        if _outcomes_match(reference, competitor):
            triples.append(
                (
                    REFERENCE_IMPLEMENTATION,
                    CANDIDATE_IMPLEMENTATION,
                    competitor_id,
                )
            )
    return tuple(triples)


def _outcomes_match(
    reference: Mapping[str, object] | None,
    candidate: Mapping[str, object] | None,
) -> bool:
    if reference is None or candidate is None:
        return False
    reference_outcome = reference.get("outcome")
    candidate_outcome = candidate.get("outcome")
    if not isinstance(reference_outcome, dict) or not isinstance(
        candidate_outcome, dict
    ):
        return False
    if reference_outcome.get("status") != "success":
        return False
    if candidate_outcome.get("status") != "success":
        return False
    return _canonical_json(reference_outcome.get("value")) == _canonical_json(
        candidate_outcome.get("value")
    )


def write_local_run(
    output_path: Path,
    *,
    records: Sequence[Mapping[str, object]],
    preflight_decisions: Sequence[Mapping[str, object]],
    timings: Sequence[Mapping[str, object]],
) -> None:
    """Write an explicitly unpublished local run without dropping rejections."""

    payload = {
        "schema_version": 1,
        "publication_status": "local-unpublished",
        "records": list(records),
        "preflight_decisions": list(preflight_decisions),
        "timings": list(timings),
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(payload, indent=2, ensure_ascii=False, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def render_markdown(suite: CompetitorSuite) -> str:
    """Render pinned sources and the local-only execution boundary."""

    rows = []
    for implementation in suite.implementations:
        revision = (
            "resolved exact repository head at run time"
            if implementation.revision == "repository-head"
            else f"`{implementation.revision}`"
        )
        rows.append(
            "| "
            + " | ".join(
                (
                    f"`{implementation.id}`",
                    f"[{implementation.display_name}]({implementation.repository})",
                    revision,
                    implementation.license,
                    ", ".join(f"`{value}`" for value in implementation.workloads),
                )
            )
            + " |"
        )
    return "\n".join(
        (
            f"# Pinned competitor suite {suite.release}",
            "",
            (
                f"Suite `{suite.id}` binds four implementations to corpus "
                f"`{suite.corpus_id}` and equivalence policy `{suite.policy_id}`."
            ),
            "",
            "| ID | Project | Revision | License | Overlapping workloads |",
            "|---|---|---|---|---|",
            *rows,
            "",
            "## Execution contract",
            "",
            (
                "Every implementation receives the same repository-owned fixture bytes, "
                "SHA-256 identity, fixture password metadata, page selection, and plain-text "
                "options. The adapters expose only `document-open` and page-preserving `text`, "
                "the two materially equivalent workloads available across all four pins."
            ),
            "",
            (
                "The complete output phase finishes before the timing phase starts. A timing "
                "triple requires pinned Python `pdfplumber`, the exact `pdfplumber-rs` run head, "
                "and one competitor to succeed and match exact canonical JSON. Errors, "
                "unsupported cases, and output differences remain in the local run and have no "
                "timing entry."
            ),
            "",
            "```console",
            "python3 scripts/run_competitor_benchmarks.py --check",
            "python3 scripts/run_competitor_benchmarks.py --build",
            (
                "python3 scripts/run_competitor_benchmarks.py --run "
                "--output /tmp/pdfplumber-rs-competitors.json"
            ),
            "```",
            "",
            (
                "A SCORE-003 run is deliberately local and is not published independently. It "
                "records one combined process wall-time sample only after equivalence; it is not "
                "a ranking or a publishable benchmark. SCORE-004 through SCORE-007 add separated "
                "clocks, resource metrics, explicit workload state, complete run metadata, five "
                "raw repetitions, and statistical summaries. SCORE-008 publishes only that "
                "complete exact-tag result bundle."
            ),
            "",
        )
    )


def implementations_by_id(
    implementations: Sequence[CompetitorImplementation],
) -> dict[str, CompetitorImplementation]:
    """Index validated implementations by stable identifier."""

    return {implementation.id: implementation for implementation in implementations}


def _canonical_json(value: object) -> str:
    try:
        return json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        )
    except (TypeError, ValueError) as error:
        raise CompetitorBenchmarkError("adapter output is not finite JSON") from error


def _required_mapping(
    source: Mapping[str, object], key: str, context: str
) -> dict[str, object]:
    value = source.get(key)
    if not isinstance(value, dict):
        raise CompetitorBenchmarkError(f"{context} needs one {key} table")
    return value


def _required_string(source: Mapping[str, object], key: str, context: str) -> str:
    value = source.get(key)
    if not isinstance(value, str) or not value:
        raise CompetitorBenchmarkError(f"{context} needs a non-empty {key}")
    return value


def _required_string_array(
    source: Mapping[str, object], key: str, context: str
) -> tuple[str, ...]:
    value = source.get(key)
    if (
        not isinstance(value, list)
        or not value
        or not all(isinstance(item, str) and item for item in value)
    ):
        raise CompetitorBenchmarkError(f"{context} needs a non-empty {key} array")
    values = tuple(value)
    if len(set(values)) != len(values):
        raise CompetitorBenchmarkError(f"{context} has duplicate {key} values")
    return values


def _required_repository_path(
    source: Mapping[str, object], key: str, context: str
) -> str:
    value = _required_string(source, key, context)
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        raise CompetitorBenchmarkError(f"{context} {key} is not repository-relative")
    return path.as_posix()


def _required_repository_file(
    source: Mapping[str, object],
    key: str,
    context: str,
    repo_root: Path,
) -> str:
    value = _required_repository_path(source, key, context)
    if not (repo_root / value).is_file():
        raise CompetitorBenchmarkError(f"{context} {key} is missing: {value}")
    return value
