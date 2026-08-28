"""Fail-closed benchmark-regression alerts with an explicit noise policy."""

from __future__ import annotations

import json
import math
import re
import statistics
from collections.abc import Mapping, Sequence
from dataclasses import asdict, dataclass
from pathlib import Path

import tomllib

from compat.harness import benchmark_provenance, benchmark_retention

ID_PATTERN = re.compile(r"^[a-z0-9]+(?:[.-][a-z0-9]+)*$")
SHA_PATTERN = re.compile(r"^[0-9a-f]{40}$")
EXPECTED_RUN_ORDER = ("baseline", "candidate", "candidate", "baseline")
PASS = "pass"
REGRESSION = "regression"
SEMANTIC_FAILURE = "semantic-failure"
INCONCLUSIVE = "inconclusive"


class BenchmarkRegressionError(ValueError):
    """The regression policy or a compared run violates its contract."""


@dataclass(frozen=True)
class RegressionPolicy:
    """Versioned paired-run, control-normalization, and alert thresholds."""

    schema_version: int
    id: str
    release: str
    baseline_tag: str
    runner: str
    run_order: tuple[str, ...]
    runs_per_revision: int
    repetitions_per_run: int
    target_implementations: tuple[str, ...]
    control_implementations: tuple[str, ...]
    minimum_control_groups: int
    minimum_slowdown_fraction: float
    noise_multiplier: float
    lower_quantile: float
    upper_quantile: float
    baseline_revision: str = ""


@dataclass(frozen=True)
class GroupComparison:
    """One control-normalized target timing-group comparison."""

    implementation_id: str
    scenario_id: str
    case_id: str
    baseline_median_wall_time_ns: float
    candidate_median_wall_time_ns: float
    control_normalized_candidate_median_wall_time_ns: float
    normalized_slowdown_fraction: float
    baseline_relative_median_absolute_deviation: float
    candidate_relative_median_absolute_deviation: float
    noise_margin_fraction: float
    baseline_upper_quantile_wall_time_ns: float
    candidate_lower_quantile_wall_time_ns: float
    distributions_separated: bool
    status: str


@dataclass(frozen=True)
class RegressionDecision:
    """Machine-readable outcome of one completed paired benchmark audit."""

    schema_version: int
    status: str
    policy_id: str
    baseline_revision: str
    candidate_revision: str
    control_scale: float | None
    control_group_count: int
    reasons: tuple[str, ...]
    comparisons: tuple[GroupComparison, ...]


def audit_repository(
    repo_root: Path,
    regression_path: Path,
    retention_path: Path,
    publication_path: Path,
    provenance_path: Path,
    scenario_path: Path,
    competitor_path: Path,
    corpus_path: Path,
    equivalence_path: Path,
    registry_path: Path,
) -> RegressionPolicy:
    """Validate the regression policy and every inherited benchmark input."""

    retention_plan = benchmark_retention.audit_repository(
        repo_root,
        retention_path,
        publication_path,
        provenance_path,
        scenario_path,
        competitor_path,
        corpus_path,
        equivalence_path,
        registry_path,
    )
    try:
        with regression_path.open("rb") as regression_file:
            source = tomllib.load(regression_file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BenchmarkRegressionError(
            f"cannot read regression policy: {regression_path}"
        ) from error
    return validate_policy(source, retention_plan)


def validate_policy(
    source: Mapping[str, object],
    retention_plan: benchmark_retention.RetentionPlan,
) -> RegressionPolicy:
    """Validate one parsed regression policy against retained-result identity."""

    schema = source.get("schema")
    if not isinstance(schema, dict) or schema.get("version") != 1:
        raise BenchmarkRegressionError("schema.version must be 1")
    raw_policy = _required_mapping(source, "policy", "manifest")
    policy_id = _required_string(raw_policy, "id", "policy")
    if not ID_PATTERN.fullmatch(policy_id):
        raise BenchmarkRegressionError(f"invalid policy id: {policy_id}")
    release = _required_string(raw_policy, "release", "policy")
    publication = retention_plan.publication_plan
    if release != publication.release:
        raise BenchmarkRegressionError("regression and publication releases differ")
    if _required_string(raw_policy, "publication_id", "policy") != publication.id:
        raise BenchmarkRegressionError("regression names the wrong publication")
    baseline_tag = _required_string(raw_policy, "baseline_tag", "policy")
    if baseline_tag != retention_plan.release_tag:
        raise BenchmarkRegressionError("regression baseline is not the retained tag")
    if retention_plan.status != benchmark_retention.RETAINED:
        raise BenchmarkRegressionError("regression baseline result is not retained")
    runner = _required_string(raw_policy, "runner", "policy")
    if runner != publication.runner:
        raise BenchmarkRegressionError("regression and publication runners differ")

    run_order = _required_string_tuple(
        raw_policy, "run_order", "policy", require_unique=False
    )
    if run_order != EXPECTED_RUN_ORDER:
        raise BenchmarkRegressionError(
            f"policy.run_order must be {list(EXPECTED_RUN_ORDER)}"
        )
    runs_per_revision = _required_integer(raw_policy, "runs_per_revision", "policy")
    if runs_per_revision != 2:
        raise BenchmarkRegressionError("policy.runs_per_revision must be 2")
    repetitions_per_run = _required_integer(raw_policy, "repetitions_per_run", "policy")
    if repetitions_per_run != publication.provenance_plan.repetitions:
        raise BenchmarkRegressionError(
            "regression repetitions must match the provenance suite"
        )

    target_implementations = _required_string_tuple(
        raw_policy, "target_implementations", "policy"
    )
    control_implementations = _required_string_tuple(
        raw_policy, "control_implementations", "policy"
    )
    if not target_implementations or not control_implementations:
        raise BenchmarkRegressionError(
            "target and control implementations must both be non-empty"
        )
    if set(target_implementations) & set(control_implementations):
        raise BenchmarkRegressionError(
            "target and control implementations must be disjoint"
        )
    expected_targets = {"pdfplumber-rs", "pdfplumber-rs-python"}
    if set(target_implementations) != expected_targets:
        raise BenchmarkRegressionError(
            f"policy targets must be exactly {sorted(expected_targets)}"
        )
    expected_controls = {"pdfplumber-python", "pdf-oxide", "pdfsink-rs"}
    if set(control_implementations) != expected_controls:
        raise BenchmarkRegressionError(
            f"policy controls must be exactly {sorted(expected_controls)}"
        )

    minimum_control_groups = _required_integer(
        raw_policy, "minimum_control_groups", "policy"
    )
    if minimum_control_groups < 3:
        raise BenchmarkRegressionError("policy requires at least 3 control groups")
    minimum_slowdown_fraction = _required_number(
        raw_policy, "minimum_slowdown_fraction", "policy"
    )
    if not 0 < minimum_slowdown_fraction < 1:
        raise BenchmarkRegressionError(
            "policy.minimum_slowdown_fraction must be between 0 and 1"
        )
    noise_multiplier = _required_number(raw_policy, "noise_multiplier", "policy")
    if noise_multiplier < 1:
        raise BenchmarkRegressionError("policy.noise_multiplier must be at least 1")
    lower_quantile = _required_number(raw_policy, "lower_quantile", "policy")
    upper_quantile = _required_number(raw_policy, "upper_quantile", "policy")
    if not 0 < lower_quantile < 0.5 < upper_quantile < 1:
        raise BenchmarkRegressionError(
            "policy quantiles must straddle the median inside (0, 1)"
        )

    return RegressionPolicy(
        schema_version=1,
        id=policy_id,
        release=release,
        baseline_tag=baseline_tag,
        runner=runner,
        run_order=run_order,
        runs_per_revision=runs_per_revision,
        repetitions_per_run=repetitions_per_run,
        target_implementations=target_implementations,
        control_implementations=control_implementations,
        minimum_control_groups=minimum_control_groups,
        minimum_slowdown_fraction=minimum_slowdown_fraction,
        noise_multiplier=noise_multiplier,
        lower_quantile=lower_quantile,
        upper_quantile=upper_quantile,
        baseline_revision=retention_plan.source_revision,
    )


def compare_runs(
    policy: RegressionPolicy,
    baseline_runs: Sequence[Mapping[str, object]],
    candidate_runs: Sequence[Mapping[str, object]],
) -> RegressionDecision:
    """Compare completed ABBA runs, checking semantics before wall time."""

    try:
        baseline_revision = _validate_run_set(policy, "baseline", baseline_runs)
        candidate_revision = _validate_run_set(policy, "candidate", candidate_runs)
        if policy.baseline_revision and baseline_revision != policy.baseline_revision:
            raise BenchmarkRegressionError(
                "baseline run revision does not match the retained tag target"
            )
        _validate_shared_environment((*baseline_runs, *candidate_runs))
    except BenchmarkRegressionError as error:
        return _decision(
            policy,
            INCONCLUSIVE,
            _run_revision(baseline_runs),
            _run_revision(candidate_runs),
            None,
            0,
            (str(error),),
            (),
        )

    semantic_reasons = _semantic_failure_reasons(policy, baseline_runs, candidate_runs)
    if semantic_reasons:
        return _decision(
            policy,
            SEMANTIC_FAILURE,
            baseline_revision,
            candidate_revision,
            None,
            0,
            semantic_reasons,
            (),
        )

    control_projections = [
        _control_timing_projection(policy, run)
        for run in (*baseline_runs, *candidate_runs)
    ]
    if any(
        projection != control_projections[0] for projection in control_projections[1:]
    ):
        return _decision(
            policy,
            INCONCLUSIVE,
            baseline_revision,
            candidate_revision,
            None,
            0,
            ("control timing identity or semantic output changed",),
            (),
        )

    baseline_groups = _pooled_groups(policy, baseline_runs)
    candidate_groups = _pooled_groups(policy, candidate_runs)
    control_keys = sorted(
        key for key in baseline_groups if key[0] in policy.control_implementations
    )
    if set(control_keys) != {
        key for key in candidate_groups if key[0] in policy.control_implementations
    }:
        return _decision(
            policy,
            INCONCLUSIVE,
            baseline_revision,
            candidate_revision,
            None,
            len(control_keys),
            ("control timing-group identities changed between revisions",),
            (),
        )
    if len(control_keys) < policy.minimum_control_groups:
        return _decision(
            policy,
            INCONCLUSIVE,
            baseline_revision,
            candidate_revision,
            None,
            len(control_keys),
            (
                f"only {len(control_keys)} common control groups; "
                + f"policy requires {policy.minimum_control_groups}",
            ),
            (),
        )
    control_ratios = [
        statistics.median(candidate_groups[key])
        / statistics.median(baseline_groups[key])
        for key in control_keys
    ]
    control_scale = statistics.median(control_ratios)
    if not math.isfinite(control_scale) or control_scale <= 0:
        return _decision(
            policy,
            INCONCLUSIVE,
            baseline_revision,
            candidate_revision,
            None,
            len(control_keys),
            ("control normalization scale is not finite and positive",),
            (),
        )

    target_keys = sorted(
        key for key in baseline_groups if key[0] in policy.target_implementations
    )
    if set(target_keys) != {
        key for key in candidate_groups if key[0] in policy.target_implementations
    }:
        return _decision(
            policy,
            SEMANTIC_FAILURE,
            baseline_revision,
            candidate_revision,
            control_scale,
            len(control_keys),
            (
                "eligible target timing groups changed after output-equivalence preflight",
            ),
            (),
        )

    comparisons = tuple(
        _compare_group(
            policy,
            key,
            baseline_groups[key],
            candidate_groups[key],
            control_scale,
        )
        for key in target_keys
    )
    regressions = tuple(
        comparison for comparison in comparisons if comparison.status == REGRESSION
    )
    reasons = tuple(
        f"{comparison.implementation_id}/{comparison.scenario_id}/"
        f"{comparison.case_id} slowed by "
        f"{comparison.normalized_slowdown_fraction:.1%} after control normalization"
        for comparison in regressions
    )
    return _decision(
        policy,
        REGRESSION if regressions else PASS,
        baseline_revision,
        candidate_revision,
        control_scale,
        len(control_keys),
        reasons,
        comparisons,
    )


def serialize_decision(decision: RegressionDecision) -> str:
    """Serialize a paired-run decision deterministically."""

    return (
        json.dumps(
            {
                "schema_version": decision.schema_version,
                "status": decision.status,
                "policy_id": decision.policy_id,
                "baseline_revision": decision.baseline_revision,
                "candidate_revision": decision.candidate_revision,
                "control_scale": decision.control_scale,
                "control_group_count": decision.control_group_count,
                "reasons": list(decision.reasons),
                "comparisons": [
                    asdict(comparison) for comparison in decision.comparisons
                ],
            },
            allow_nan=False,
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )


def render_policy(policy: RegressionPolicy) -> str:
    """Render the public noise and alert policy from the validated source."""

    targets = ", ".join(f"`{item}`" for item in policy.target_implementations)
    controls = ", ".join(f"`{item}`" for item in policy.control_implementations)
    samples = policy.runs_per_revision * policy.repetitions_per_run
    return "\n".join(
        [
            "# Benchmark regression alerts v0.3.0",
            "",
            f"Policy: `{policy.id}`. Baseline: `{policy.baseline_tag}`. Runner: `{policy.runner}`.",
            "",
            "## Measurement protocol",
            "",
            f"The workflow builds the retained baseline and current revision on one runner, then executes `{policy.run_order[0]} → {policy.run_order[1]} → {policy.run_order[2]} → {policy.run_order[3]}`. Each run retains {policy.repetitions_per_run} round-robin samples, giving {samples} samples per revision and timing group. This ABBA order balances first-versus-last drift; it does not claim to eliminate hosted-runner noise.",
            "",
            f"Targets are {targets}. Pinned controls are {controls}. The median current-to-baseline control ratio across at least {policy.minimum_control_groups} common groups normalizes shared host movement.",
            "",
            "## Alert rule",
            "",
            f"A target group alerts only when all three conditions hold after control normalization: median slowdown is at least {policy.minimum_slowdown_fraction:.0%}; slowdown is at least {policy.noise_multiplier:g} times the larger relative median absolute deviation; and the current {policy.lower_quantile:.0%} quantile is above the baseline {policy.upper_quantile:.0%} quantile. Otherwise the group is recorded as within policy or noise-overlap, not promoted into a regression claim.",
            "",
            "## Semantic gate",
            "",
            "Untimed records, output-equivalence decisions, eligible target group identities, fixture bindings, and semantic output digests are compared before wall time. Any target semantic or eligibility drift is `semantic-failure`; thresholds are never consulted, and making output checks weaker cannot turn it into a performance pass.",
            "",
            "A missing run, changed host/toolchain identity, malformed summary, insufficient controls, or non-finite normalization is `inconclusive`. `regression`, `semantic-failure`, and `inconclusive` all fail the workflow after its machine-readable decision artifact is uploaded. The alert is a guard for investigation, not a ranking, confidence interval, or broad product performance claim.",
            "",
        ]
    )


def _validate_run_set(
    policy: RegressionPolicy,
    label: str,
    runs: Sequence[Mapping[str, object]],
) -> str:
    if len(runs) != policy.runs_per_revision:
        raise BenchmarkRegressionError(
            f"{label} requires {policy.runs_per_revision} completed runs"
        )
    revisions: set[str] = set()
    for index, run in enumerate(runs, start=1):
        if run.get("schema_version") != 1:
            raise BenchmarkRegressionError(
                f"{label} run {index} schema_version must be 1"
            )
        metadata = _required_mapping(run, "run_metadata", f"{label} run {index}")
        try:
            benchmark_provenance.validate_run_metadata(
                metadata, repetitions=policy.repetitions_per_run
            )
        except benchmark_provenance.BenchmarkProvenanceError as error:
            raise BenchmarkRegressionError(
                f"{label} run {index} metadata failed: {error}"
            ) from error
        source = _required_mapping(metadata, "source", "run metadata")
        revision = _required_string(source, "revision", "run source")
        revisions.add(revision)
        timings = _required_mapping_array(
            run, "scenario_timings", f"{label} run {index}"
        )
        summaries = _required_mapping_array(
            run, "statistical_summaries", f"{label} run {index}"
        )
        try:
            expected = benchmark_provenance.summarize_samples(
                timings, repetitions=policy.repetitions_per_run
            )
        except benchmark_provenance.BenchmarkProvenanceError as error:
            raise BenchmarkRegressionError(
                f"{label} run {index} samples failed: {error}"
            ) from error
        if _canonical_json(expected) != _canonical_json(summaries):
            raise BenchmarkRegressionError(
                f"{label} run {index} statistical summaries do not match raw samples"
            )
    if len(revisions) != 1:
        raise BenchmarkRegressionError(f"{label} runs use different source revisions")
    revision = revisions.pop()
    if not SHA_PATTERN.fullmatch(revision):
        raise BenchmarkRegressionError(f"{label} source revision is not a full Git SHA")
    return revision


def _validate_shared_environment(runs: Sequence[Mapping[str, object]]) -> None:
    fingerprints: set[str] = set()
    for run in runs:
        metadata = _required_mapping(run, "run_metadata", "run")
        fingerprint = {
            "host": metadata.get("host"),
            "toolchains": metadata.get("toolchains"),
        }
        fingerprints.add(_canonical_json(fingerprint))
    if len(fingerprints) != 1:
        raise BenchmarkRegressionError(
            "paired runs do not share exact host and toolchain identities"
        )


def _semantic_failure_reasons(
    policy: RegressionPolicy,
    baseline_runs: Sequence[Mapping[str, object]],
    candidate_runs: Sequence[Mapping[str, object]],
) -> tuple[str, ...]:
    all_runs = (*baseline_runs, *candidate_runs)
    reasons: list[str] = []
    decision_projections = [
        _target_decision_projection(policy, run) for run in all_runs
    ]
    if any(
        projection != decision_projections[0] for projection in decision_projections[1:]
    ):
        reasons.append("target output-equivalence decisions changed between revisions")
    if any(
        not decision.get("eligible_for_timing")
        for run in all_runs
        for decision in _target_decisions(policy, run)
    ):
        reasons.append(
            "target output-equivalence preflight is not exact and timing-eligible"
        )
    record_projections = [_target_record_projection(policy, run) for run in all_runs]
    if any(
        projection != record_projections[0] for projection in record_projections[1:]
    ):
        reasons.append("target untimed semantic records changed between revisions")
    timing_projections = [_target_timing_projection(policy, run) for run in all_runs]
    if any(
        projection != timing_projections[0] for projection in timing_projections[1:]
    ):
        reasons.append("target eligible timing identity or semantic output changed")
    observed_targets = {
        key[0]
        for key in _pooled_groups(policy, baseline_runs)
        if key[0] in policy.target_implementations
    }
    missing_targets = set(policy.target_implementations) - observed_targets
    if missing_targets:
        reasons.append(
            "target implementations have no eligible timing groups: "
            + ", ".join(sorted(missing_targets))
        )
    return tuple(dict.fromkeys(reasons))


def _target_decisions(
    policy: RegressionPolicy, run: Mapping[str, object]
) -> tuple[Mapping[str, object], ...]:
    decisions = _required_mapping_array(run, "preflight_decisions", "run")
    return tuple(
        decision
        for decision in decisions
        if decision.get("implementation_id") in policy.target_implementations
    )


def _target_decision_projection(
    policy: RegressionPolicy, run: Mapping[str, object]
) -> tuple[str, ...]:
    return tuple(
        sorted(_canonical_json(decision) for decision in _target_decisions(policy, run))
    )


def _target_record_projection(
    policy: RegressionPolicy, run: Mapping[str, object]
) -> tuple[str, ...]:
    records = _required_mapping_array(run, "records", "run")
    projections: list[str] = []
    for record in records:
        implementation = record.get("implementation")
        if not isinstance(implementation, dict):
            continue
        if implementation.get("id") not in policy.target_implementations:
            continue
        normalized = dict(record)
        normalized_implementation = dict(implementation)
        normalized_implementation.pop("revision", None)
        normalized["implementation"] = normalized_implementation
        projections.append(_canonical_json(normalized))
    return tuple(sorted(projections))


def _target_timing_projection(
    policy: RegressionPolicy, run: Mapping[str, object]
) -> tuple[str, ...]:
    timings = _required_mapping_array(run, "scenario_timings", "run")
    projections: set[str] = set()
    for timing in timings:
        implementation = _required_mapping(timing, "implementation", "timing")
        if implementation.get("id") not in policy.target_implementations:
            continue
        stable = {
            key: value
            for key, value in timing.items()
            if key not in {"repetition", "wall_time_ns"}
        }
        normalized_implementation = dict(implementation)
        normalized_implementation.pop("revision", None)
        stable["implementation"] = normalized_implementation
        projections.add(_canonical_json(stable))
    return tuple(sorted(projections))


def _control_timing_projection(
    policy: RegressionPolicy, run: Mapping[str, object]
) -> tuple[str, ...]:
    timings = _required_mapping_array(run, "scenario_timings", "run")
    projections: set[str] = set()
    for timing in timings:
        implementation = _required_mapping(timing, "implementation", "timing")
        if implementation.get("id") not in policy.control_implementations:
            continue
        stable = {
            key: value
            for key, value in timing.items()
            if key not in {"repetition", "wall_time_ns"}
        }
        projections.add(_canonical_json(stable))
    return tuple(sorted(projections))


def _pooled_groups(
    policy: RegressionPolicy,
    runs: Sequence[Mapping[str, object]],
) -> dict[tuple[str, str, str], tuple[int, ...]]:
    groups: dict[tuple[str, str, str], list[int]] = {}
    for run in runs:
        timings = _required_mapping_array(run, "scenario_timings", "run")
        for timing in timings:
            implementation = _required_mapping(timing, "implementation", "timing")
            implementation_id = _required_string(
                implementation, "id", "timing implementation"
            )
            key = (
                implementation_id,
                _required_string(timing, "scenario_id", "timing"),
                _required_string(timing, "case_id", "timing"),
            )
            wall_time = timing.get("wall_time_ns")
            if isinstance(wall_time, bool) or not isinstance(wall_time, int):
                raise BenchmarkRegressionError("timing.wall_time_ns must be an integer")
            if wall_time <= 0:
                raise BenchmarkRegressionError("timing.wall_time_ns must be positive")
            groups.setdefault(key, []).append(wall_time)
    expected_count = policy.runs_per_revision * policy.repetitions_per_run
    for key, values in groups.items():
        if len(values) != expected_count:
            raise BenchmarkRegressionError(
                f"{key} has {len(values)} samples; expected {expected_count}"
            )
    return {key: tuple(values) for key, values in groups.items()}


def _compare_group(
    policy: RegressionPolicy,
    key: tuple[str, str, str],
    baseline_samples: Sequence[int],
    candidate_samples: Sequence[int],
    control_scale: float,
) -> GroupComparison:
    normalized_candidate = [sample / control_scale for sample in candidate_samples]
    baseline_median = statistics.median(baseline_samples)
    candidate_median = statistics.median(candidate_samples)
    normalized_candidate_median = statistics.median(normalized_candidate)
    slowdown = normalized_candidate_median / baseline_median - 1
    baseline_relative_mad = _relative_median_absolute_deviation(baseline_samples)
    candidate_relative_mad = _relative_median_absolute_deviation(normalized_candidate)
    noise_margin = policy.noise_multiplier * max(
        baseline_relative_mad, candidate_relative_mad
    )
    baseline_upper = _quantile(baseline_samples, policy.upper_quantile)
    candidate_lower = _quantile(normalized_candidate, policy.lower_quantile)
    separated = candidate_lower > baseline_upper
    if (
        slowdown >= policy.minimum_slowdown_fraction
        and slowdown >= noise_margin
        and separated
    ):
        status = REGRESSION
    elif slowdown >= policy.minimum_slowdown_fraction:
        status = "noise-overlap"
    else:
        status = "within-policy"
    return GroupComparison(
        implementation_id=key[0],
        scenario_id=key[1],
        case_id=key[2],
        baseline_median_wall_time_ns=baseline_median,
        candidate_median_wall_time_ns=candidate_median,
        control_normalized_candidate_median_wall_time_ns=(normalized_candidate_median),
        normalized_slowdown_fraction=slowdown,
        baseline_relative_median_absolute_deviation=baseline_relative_mad,
        candidate_relative_median_absolute_deviation=candidate_relative_mad,
        noise_margin_fraction=noise_margin,
        baseline_upper_quantile_wall_time_ns=baseline_upper,
        candidate_lower_quantile_wall_time_ns=candidate_lower,
        distributions_separated=separated,
        status=status,
    )


def _relative_median_absolute_deviation(values: Sequence[int | float]) -> float:
    median = statistics.median(values)
    if median <= 0:
        raise BenchmarkRegressionError("timing median must be positive")
    deviation = statistics.median(abs(value - median) for value in values)
    return deviation / median


def _quantile(values: Sequence[int | float], probability: float) -> float:
    ordered = sorted(values)
    if not ordered:
        raise BenchmarkRegressionError("cannot compute a quantile of no samples")
    position = probability * (len(ordered) - 1)
    lower_index = math.floor(position)
    upper_index = math.ceil(position)
    if lower_index == upper_index:
        return float(ordered[lower_index])
    weight = position - lower_index
    return float(ordered[lower_index] * (1 - weight) + ordered[upper_index] * weight)


def _decision(
    policy: RegressionPolicy,
    status: str,
    baseline_revision: str,
    candidate_revision: str,
    control_scale: float | None,
    control_group_count: int,
    reasons: Sequence[str],
    comparisons: Sequence[GroupComparison],
) -> RegressionDecision:
    return RegressionDecision(
        schema_version=1,
        status=status,
        policy_id=policy.id,
        baseline_revision=baseline_revision,
        candidate_revision=candidate_revision,
        control_scale=control_scale,
        control_group_count=control_group_count,
        reasons=tuple(dict.fromkeys(reasons)),
        comparisons=tuple(comparisons),
    )


def _run_revision(runs: Sequence[Mapping[str, object]]) -> str:
    if not runs:
        return ""
    metadata = runs[0].get("run_metadata")
    if not isinstance(metadata, dict):
        return ""
    source = metadata.get("source")
    if not isinstance(source, dict):
        return ""
    revision = source.get("revision")
    return revision if isinstance(revision, str) else ""


def _required_mapping(
    value: Mapping[str, object], key: str, context: str
) -> Mapping[str, object]:
    item = value.get(key)
    if not isinstance(item, dict):
        raise BenchmarkRegressionError(f"{context}.{key} must be an object")
    return item


def _required_mapping_array(
    value: Mapping[str, object], key: str, context: str
) -> tuple[Mapping[str, object], ...]:
    item = value.get(key)
    if not isinstance(item, list) or any(not isinstance(entry, dict) for entry in item):
        raise BenchmarkRegressionError(f"{context}.{key} must be an object array")
    return tuple(item)


def _required_string(value: Mapping[str, object], key: str, context: str) -> str:
    item = value.get(key)
    if not isinstance(item, str) or not item:
        raise BenchmarkRegressionError(f"{context}.{key} must be a non-empty string")
    return item


def _required_string_tuple(
    value: Mapping[str, object],
    key: str,
    context: str,
    *,
    require_unique: bool = True,
) -> tuple[str, ...]:
    item = value.get(key)
    if (
        not isinstance(item, list)
        or not item
        or any(not isinstance(entry, str) or not entry for entry in item)
    ):
        raise BenchmarkRegressionError(
            f"{context}.{key} must be a non-empty string array"
        )
    if require_unique and len(set(item)) != len(item):
        raise BenchmarkRegressionError(f"{context}.{key} must be unique")
    return tuple(item)


def _required_integer(value: Mapping[str, object], key: str, context: str) -> int:
    item = value.get(key)
    if isinstance(item, bool) or not isinstance(item, int):
        raise BenchmarkRegressionError(f"{context}.{key} must be an integer")
    return item


def _required_number(value: Mapping[str, object], key: str, context: str) -> float:
    item = value.get(key)
    if isinstance(item, bool) or not isinstance(item, (int, float)):
        raise BenchmarkRegressionError(f"{context}.{key} must be a number")
    number = float(item)
    if not math.isfinite(number):
        raise BenchmarkRegressionError(f"{context}.{key} must be finite")
    return number


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
        raise BenchmarkRegressionError(
            "benchmark data is not canonical JSON"
        ) from error
