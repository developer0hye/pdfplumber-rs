"""Fail-closed output-equivalence preflight for benchmark comparisons."""

from __future__ import annotations

import hashlib
import json
import math
import re
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path

import tomllib

from compat.harness import benchmark_corpus

POLICY_ID_PATTERN = re.compile(r"^[a-z0-9]+(?:[.-][a-z0-9]+)*$")
WORKLOAD_ID_PATTERN = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
SEMVER_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
REQUIRED_WORKLOAD_SCHEMAS = {
    "document-open": "page-count-v1",
    "graphics": "page-graphics-v1",
    "images": "page-images-v1",
    "tables": "page-tables-v1",
    "text": "page-text-v1",
    "words": "page-words-v1",
}
OUTCOME_STATUSES = frozenset({"success", "unsupported", "error"})


class BenchmarkEquivalenceError(ValueError):
    """The policy or an untimed output record is malformed."""


@dataclass(frozen=True)
class WorkloadContract:
    id: str
    output_schema: str
    semantic_classes: tuple[str, ...]
    request: dict[str, object]


@dataclass(frozen=True)
class EquivalencePolicy:
    id: str
    release: str
    corpus_id: str
    reference: str
    workloads: tuple[WorkloadContract, ...]

    def workload(self, workload_id: str) -> WorkloadContract | None:
        """Return a workload contract, or ``None`` when it is not declared."""

        for workload in self.workloads:
            if workload.id == workload_id:
                return workload
        return None


@dataclass(frozen=True)
class OutputRecord:
    implementation_id: str
    implementation_revision: str
    fixture_id: str
    fixture_sha256: str
    workload_id: str
    output_schema: str
    request: dict[str, object]
    outcome_status: str
    outcome_value: object | None


@dataclass(frozen=True)
class EquivalenceDecision:
    case_id: str
    reference_implementation: str
    candidate_implementation: str
    eligible_for_timing: bool
    reasons: tuple[str, ...]
    reference_output_sha256: str | None
    candidate_output_sha256: str | None

    def to_dict(self) -> dict[str, object]:
        """Return the deterministic machine-readable preflight decision."""

        return {
            "schema_version": 1,
            "case_id": self.case_id,
            "reference_implementation": self.reference_implementation,
            "candidate_implementation": self.candidate_implementation,
            "eligible_for_timing": self.eligible_for_timing,
            "reasons": list(self.reasons),
            "reference_output_sha256": self.reference_output_sha256,
            "candidate_output_sha256": self.candidate_output_sha256,
        }


def load_policy(path: Path) -> EquivalencePolicy:
    """Load and validate one repository-owned equivalence policy."""

    try:
        with path.open("rb") as policy_file:
            source = tomllib.load(policy_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BenchmarkEquivalenceError(
            f"cannot read benchmark equivalence policy: {path}"
        ) from error
    return validate_policy(source, path.parents[1])


def audit_repository(
    repo_root: Path,
    policy_path: Path,
    corpus_path: Path,
    registry_path: Path,
) -> EquivalencePolicy:
    """Validate the policy and bind it to the audited benchmark corpus."""

    corpus = benchmark_corpus.audit_repository(
        repo_root,
        corpus_path,
        registry_path,
    )
    policy = validate_policy(_load_toml(policy_path), repo_root)
    if policy.corpus_id != corpus.id:
        raise BenchmarkEquivalenceError(
            f"policy corpus {policy.corpus_id} does not match {corpus.id}"
        )
    if policy.release != corpus.release:
        raise BenchmarkEquivalenceError(
            f"policy release {policy.release} does not match {corpus.release}"
        )
    return policy


def _load_toml(path: Path) -> dict[str, object]:
    try:
        with path.open("rb") as source_file:
            return tomllib.load(source_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BenchmarkEquivalenceError(f"cannot read policy: {path}") from error


def validate_policy(
    source: Mapping[str, object],
    repo_root: Path,
) -> EquivalencePolicy:
    """Validate one versioned workload and request-semantics contract."""

    schema = source.get("schema")
    if not isinstance(schema, dict) or schema.get("version") != 1:
        raise BenchmarkEquivalenceError("schema.version must be 1")
    raw_policy = source.get("policy")
    if not isinstance(raw_policy, dict):
        raise BenchmarkEquivalenceError("policy must be one table")
    policy_id = _required_string(raw_policy, "id", "policy")
    if not POLICY_ID_PATTERN.fullmatch(policy_id):
        raise BenchmarkEquivalenceError(f"invalid policy id: {policy_id}")
    release = _required_string(raw_policy, "release", "policy")
    if not SEMVER_PATTERN.fullmatch(release):
        raise BenchmarkEquivalenceError(f"invalid policy release: {release}")
    corpus_id = _required_string(raw_policy, "corpus_id", "policy")
    reference = _required_repository_path(
        raw_policy,
        "reference",
        "policy",
        repo_root,
    )

    raw_workloads = source.get("workloads")
    if not isinstance(raw_workloads, list) or not raw_workloads:
        raise BenchmarkEquivalenceError("workloads must be a non-empty array")
    workloads: list[WorkloadContract] = []
    workload_ids: set[str] = set()
    for position, raw_workload in enumerate(raw_workloads, start=1):
        if not isinstance(raw_workload, dict):
            raise BenchmarkEquivalenceError(f"workload {position} must be a table")
        workload = _validate_workload(raw_workload)
        if workload.id in workload_ids:
            raise BenchmarkEquivalenceError(f"duplicate workload id: {workload.id}")
        workload_ids.add(workload.id)
        workloads.append(workload)

    missing = set(REQUIRED_WORKLOAD_SCHEMAS) - workload_ids
    if missing:
        raise BenchmarkEquivalenceError(
            "missing workloads: " + ", ".join(sorted(missing))
        )
    extra = workload_ids - set(REQUIRED_WORKLOAD_SCHEMAS)
    if extra:
        raise BenchmarkEquivalenceError(
            "unknown workloads: " + ", ".join(sorted(extra))
        )
    return EquivalencePolicy(
        id=policy_id,
        release=release,
        corpus_id=corpus_id,
        reference=reference,
        workloads=tuple(sorted(workloads, key=lambda workload: workload.id)),
    )


def _validate_workload(raw_workload: Mapping[str, object]) -> WorkloadContract:
    workload_id = _required_string(raw_workload, "id", "workload")
    if not WORKLOAD_ID_PATTERN.fullmatch(workload_id):
        raise BenchmarkEquivalenceError(f"invalid workload id: {workload_id}")
    output_schema = _required_string(
        raw_workload,
        "output_schema",
        f"workload {workload_id}",
    )
    expected_schema = REQUIRED_WORKLOAD_SCHEMAS.get(workload_id)
    if expected_schema is not None and output_schema != expected_schema:
        raise BenchmarkEquivalenceError(
            f"workload {workload_id} output schema must be {expected_schema}"
        )

    raw_classes = raw_workload.get("semantic_classes")
    if not isinstance(raw_classes, list) or not raw_classes:
        raise BenchmarkEquivalenceError(
            f"workload {workload_id} needs semantic_classes"
        )
    if not all(isinstance(value, str) for value in raw_classes):
        raise BenchmarkEquivalenceError(
            f"workload {workload_id} has a non-string semantic class"
        )
    semantic_classes = tuple(raw_classes)
    if len(set(semantic_classes)) != len(semantic_classes):
        raise BenchmarkEquivalenceError(
            f"workload {workload_id} has duplicate semantic classes"
        )
    if semantic_classes != tuple(sorted(semantic_classes)):
        raise BenchmarkEquivalenceError(
            f"workload {workload_id} semantic_classes must be sorted"
        )
    if "*" in semantic_classes and semantic_classes != ("*",):
        raise BenchmarkEquivalenceError(
            f"workload {workload_id} wildcard must be exclusive"
        )
    for semantic_class in semantic_classes:
        if (
            semantic_class != "*"
            and semantic_class not in benchmark_corpus.REQUIRED_SEMANTIC_CLASSES
        ):
            raise BenchmarkEquivalenceError(f"unknown semantic class: {semantic_class}")

    raw_request = raw_workload.get("request")
    if not isinstance(raw_request, dict) or not raw_request:
        raise BenchmarkEquivalenceError(
            f"workload {workload_id} needs a non-empty request"
        )
    _validate_json_value(raw_request, f"workload {workload_id} request")
    return WorkloadContract(
        id=workload_id,
        output_schema=output_schema,
        semantic_classes=semantic_classes,
        request=dict(raw_request),
    )


def load_record(path: Path) -> dict[str, object]:
    """Load one untimed canonical-output JSON record."""

    def reject_constant(value: str) -> object:
        raise BenchmarkEquivalenceError(
            f"record needs a finite JSON number, got {value}"
        )

    try:
        value = json.loads(
            path.read_text(encoding="utf-8"),
            parse_constant=reject_constant,
        )
    except BenchmarkEquivalenceError:
        raise
    except (OSError, json.JSONDecodeError) as error:
        raise BenchmarkEquivalenceError(f"cannot read output record: {path}") from error
    if not isinstance(value, dict):
        raise BenchmarkEquivalenceError("output record must be one JSON object")
    return value


def preflight(
    reference_source: Mapping[str, object],
    candidate_source: Mapping[str, object],
    policy: EquivalencePolicy,
    corpus: benchmark_corpus.BenchmarkCorpus,
) -> EquivalenceDecision:
    """Reject any case whose input, request, outcome, or output differs."""

    reference = _parse_record(reference_source)
    candidate = _parse_record(candidate_source)
    reasons: list[str] = []

    if reference.implementation_id == candidate.implementation_id:
        reasons.append("implementations must be distinct")
    _check_record("reference", reference, policy, corpus, reasons)
    _check_record("candidate", candidate, policy, corpus, reasons)
    if reference.fixture_id != candidate.fixture_id:
        reasons.append("fixtures differ")
    if reference.fixture_sha256 != candidate.fixture_sha256:
        reasons.append("fixture digests differ")
    if reference.workload_id != candidate.workload_id:
        reasons.append("workloads differ")
    if reference.output_schema != candidate.output_schema:
        reasons.append("output schemas differ")
    if _canonical_json(reference.request) != _canonical_json(candidate.request):
        reasons.append("requested semantics differ")
    if reference.outcome_status != "success":
        reasons.append(f"reference outcome is {reference.outcome_status}")
    if candidate.outcome_status != "success":
        reasons.append(f"candidate outcome is {candidate.outcome_status}")

    reference_digest = _output_digest(reference)
    candidate_digest = _output_digest(candidate)
    if (
        reference_digest is not None
        and candidate_digest is not None
        and reference_digest != candidate_digest
    ):
        reasons.append("canonical output differs")

    unique_reasons = tuple(dict.fromkeys(reasons))
    return EquivalenceDecision(
        case_id=f"{reference.fixture_id}:{reference.workload_id}",
        reference_implementation=reference.implementation_id,
        candidate_implementation=candidate.implementation_id,
        eligible_for_timing=not unique_reasons,
        reasons=unique_reasons,
        reference_output_sha256=reference_digest,
        candidate_output_sha256=candidate_digest,
    )


def _parse_record(source: Mapping[str, object]) -> OutputRecord:
    _require_exact_fields(
        source,
        {
            "schema_version",
            "implementation",
            "fixture",
            "workload",
            "request",
            "outcome",
        },
        "record",
    )
    if source.get("schema_version") != 1:
        raise BenchmarkEquivalenceError("record schema_version must be 1")

    implementation = _required_mapping(source, "implementation", "record")
    _require_exact_fields(
        implementation,
        {"id", "revision"},
        "implementation",
    )
    implementation_id = _required_string(
        implementation,
        "id",
        "implementation",
    )
    implementation_revision = _required_string(
        implementation,
        "revision",
        "implementation",
    )

    fixture = _required_mapping(source, "fixture", "record")
    _require_exact_fields(fixture, {"id", "sha256"}, "fixture")
    fixture_id = _required_string(fixture, "id", "fixture")
    fixture_sha256 = _required_string(fixture, "sha256", "fixture")
    if not SHA256_PATTERN.fullmatch(fixture_sha256):
        raise BenchmarkEquivalenceError("fixture sha256 must be 64 lowercase hex")

    workload = _required_mapping(source, "workload", "record")
    _require_exact_fields(workload, {"id", "output_schema"}, "workload")
    workload_id = _required_string(workload, "id", "workload")
    output_schema = _required_string(
        workload,
        "output_schema",
        "workload",
    )
    request = _required_mapping(source, "request", "record")
    _validate_json_value(request, "record request")

    outcome = _required_mapping(source, "outcome", "record")
    outcome_status = _required_string(outcome, "status", "outcome")
    if outcome_status not in OUTCOME_STATUSES:
        raise BenchmarkEquivalenceError(f"invalid outcome status: {outcome_status}")
    outcome_value: object | None = None
    if outcome_status == "success":
        if "value" not in outcome:
            raise BenchmarkEquivalenceError("success outcome needs value")
        _require_exact_fields(outcome, {"status", "value"}, "success outcome")
        outcome_value = outcome["value"]
        _validate_json_value(outcome_value, "success outcome value")
    elif outcome_status == "unsupported":
        _require_exact_fields(
            outcome,
            {"status", "reason"},
            "unsupported outcome",
        )
        _required_string(outcome, "reason", "unsupported outcome")
    else:
        _require_exact_fields(outcome, {"status", "error"}, "error outcome")
        error = _required_mapping(outcome, "error", "error outcome")
        _require_exact_fields(error, {"kind", "message"}, "error")
        _required_string(error, "kind", "error")
        _required_string(error, "message", "error")

    return OutputRecord(
        implementation_id=implementation_id,
        implementation_revision=implementation_revision,
        fixture_id=fixture_id,
        fixture_sha256=fixture_sha256,
        workload_id=workload_id,
        output_schema=output_schema,
        request=dict(request),
        outcome_status=outcome_status,
        outcome_value=outcome_value,
    )


def _check_record(
    label: str,
    record: OutputRecord,
    policy: EquivalencePolicy,
    corpus: benchmark_corpus.BenchmarkCorpus,
    reasons: list[str],
) -> None:
    try:
        fixture = next(
            fixture for fixture in corpus.fixtures if fixture.id == record.fixture_id
        )
    except StopIteration:
        reasons.append(f"{label} fixture is not in benchmark corpus")
        fixture = None
    if fixture is not None and record.fixture_sha256 != fixture.sha256:
        reasons.append(f"{label} fixture digest does not match corpus")

    workload = policy.workload(record.workload_id)
    if workload is None:
        reasons.append(f"{label} workload is not in equivalence policy")
        return
    if record.output_schema != workload.output_schema:
        reasons.append(f"{label} output schema does not match workload")
    if _canonical_json(record.request) != _canonical_json(workload.request):
        reasons.append(f"{label} request does not match workload contract")
    if (
        fixture is not None
        and workload.semantic_classes != ("*",)
        and not set(fixture.semantic_classes).intersection(workload.semantic_classes)
    ):
        reasons.append(f"{label} fixture is outside workload semantic classes")


def _output_digest(record: OutputRecord) -> str | None:
    if record.outcome_status != "success":
        return None
    canonical = _canonical_json(record.outcome_value)
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


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
        raise BenchmarkEquivalenceError("value is not canonical finite JSON") from error


def _validate_json_value(value: object, context: str) -> None:
    if value is None or isinstance(value, (str, bool, int)):
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise BenchmarkEquivalenceError(f"{context} needs a finite JSON number")
        return
    if isinstance(value, list):
        for item in value:
            _validate_json_value(item, context)
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                raise BenchmarkEquivalenceError(f"{context} needs string object keys")
            _validate_json_value(item, context)
        return
    raise BenchmarkEquivalenceError(f"{context} is not a JSON value")


def _require_exact_fields(
    table: Mapping[str, object],
    expected: set[str],
    context: str,
) -> None:
    actual = set(table)
    extra = actual - expected
    if extra:
        raise BenchmarkEquivalenceError(
            f"unexpected {context} fields: " + ", ".join(sorted(extra))
        )
    missing = expected - actual
    if missing:
        raise BenchmarkEquivalenceError(
            f"missing {context} fields: " + ", ".join(sorted(missing))
        )


def _required_mapping(
    table: Mapping[str, object],
    key: str,
    context: str,
) -> dict[str, object]:
    value = table.get(key)
    if not isinstance(value, dict):
        raise BenchmarkEquivalenceError(f"{context} needs one {key} object")
    return value


def _required_string(
    table: Mapping[str, object],
    key: str,
    context: str,
) -> str:
    value = table.get(key)
    if not isinstance(value, str) or not value:
        raise BenchmarkEquivalenceError(f"{context} needs a non-empty {key}")
    return value


def _required_repository_path(
    table: Mapping[str, object],
    key: str,
    context: str,
    repo_root: Path,
) -> str:
    value = _required_string(table, key, context)
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or not (repo_root / path).is_file():
        raise BenchmarkEquivalenceError(
            f"{context} {key} is not a repository file: {value}"
        )
    return path.as_posix()


def render_markdown(policy: EquivalencePolicy) -> str:
    """Render the public preflight policy without any performance result."""

    rows = []
    for workload in policy.workloads:
        semantic_classes = (
            "all corpus fixtures"
            if workload.semantic_classes == ("*",)
            else ", ".join(
                f"`{semantic_class}`" for semantic_class in workload.semantic_classes
            )
        )
        rows.append(
            "| "
            + " | ".join(
                (
                    f"`{workload.id}`",
                    f"`{workload.output_schema}`",
                    semantic_classes,
                    f"`{_canonical_json(workload.request)}`",
                )
            )
            + " |"
        )

    return "\n".join(
        (
            f"# Output-equivalence preflight {policy.release}",
            "",
            (
                f"Policy `{policy.id}` is the correctness gate for benchmark "
                f"corpus `{policy.corpus_id}`. It compares untimed canonical-output "
                "records before any performance command is allowed to run."
            ),
            "",
            (
                "A case is eligible for timing only when both implementations are "
                "distinct, select the same indexed fixture and digest, use the exact "
                "declared semantic request and output schema, succeed, and produce "
                "the same canonical JSON bytes after object-key sorting. Array "
                "order, JSON number types, strings, null placement, and nested "
                "values remain exact."
            ),
            "",
            (
                "Errors, unsupported workloads, missing fields, extra timing "
                "fields, non-finite numbers, fixture drift, request drift, schema "
                "drift, and output drift reject the case. A rejected case may be "
                "reported as incompatible, but it cannot contribute a timing result."
            ),
            "",
            "## Workload contracts",
            "",
            "| Workload | Canonical output | Applicable fixtures | Exact request |",
            "|---|---|---|---|",
            *rows,
            "",
            (
                "The preflight follows the accuracy-before-performance separation "
                f"in [`{policy.reference}`](../../{policy.reference}): a reference "
                "output contract is established independently from the performance "
                "phase. This project uses exact canonical equality rather than a "
                "quality threshold because benchmark wins must never weaken PDF "
                "semantics."
            ),
            "",
            "## Record protocol",
            "",
            (
                "Each JSON record contains only `schema_version`, "
                "`implementation`, `fixture`, `workload`, `request`, and `outcome`. "
                "A successful outcome contains `value`; unsupported and error "
                "outcomes contain diagnostics and are always ineligible. The "
                "decision contains output digests and rejection reasons, but no "
                "measured duration."
            ),
            "",
            "```bash",
            "python3 scripts/check_benchmark_equivalence.py --check",
            "python3 scripts/check_benchmark_equivalence.py \\",
            "  --reference reference-output.json \\",
            "  --candidate candidate-output.json",
            "```",
            "",
            (
                "The comparison command exits `0` only for an eligible case, `1` "
                "for a well-formed rejected case, and `2` for malformed policy or "
                "record input. It does not run or accept a benchmark duration."
            ),
            "",
        )
    )
