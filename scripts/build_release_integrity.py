#!/usr/bin/env python3
"""Build deterministic release checksums and validate integrity evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

REPOSITORY = "https://github.com/developer0hye/pdfplumber-rs"
REQUIRED_FAMILIES = frozenset(
    {"rust-crate", "python-wheel", "python-sdist", "cli-binary"}
)
GROUP_ID_PATTERN = re.compile(r"[a-z0-9][a-z0-9._-]*")
SHA1_PATTERN = re.compile(r"[0-9a-f]{40}")
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")
SEMVER_TAG_PATTERN = re.compile(
    r"v(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
)
SIGSTORE_MEDIA_TYPE_PREFIX = "application/vnd.dev.sigstore.bundle."
MAX_SBOM_BYTES = 16 * 1024 * 1024


class ReleaseIntegrityError(RuntimeError):
    """Raised when release evidence is missing, ambiguous, or inconsistent."""


@dataclass(frozen=True)
class FileEvidence:
    """Stable identity for one regular release file."""

    name: str
    sha256: str
    size: int

    def as_dict(self) -> dict[str, str | int]:
        return {"name": self.name, "sha256": self.sha256, "size": self.size}


@dataclass(frozen=True)
class GroupEvidence:
    """Validated subjects and integrity assets from one build job."""

    group_id: str
    family: str
    subjects: tuple[FileEvidence, ...]
    sbom: FileEvidence
    manifest: FileEvidence
    provenance_bundle: FileEvidence | None
    sbom_bundle: FileEvidence | None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    group = subparsers.add_parser(
        "group", description="bind one build-job artifact group to its SPDX SBOM"
    )
    group.add_argument("--group-id", required=True)
    group.add_argument("--family", choices=sorted(REQUIRED_FAMILIES), required=True)
    group.add_argument("--subjects-dir", type=Path, required=True)
    group.add_argument("--subject-glob", required=True)
    group.add_argument("--sbom", type=Path, required=True)
    group.add_argument("--source-commit", required=True)
    group.add_argument("--release-tag", default="")
    group.add_argument("--output", type=Path, required=True)

    aggregate = subparsers.add_parser(
        "aggregate",
        description="build a complete checksum and release-evidence index",
    )
    aggregate.add_argument("--bundle-dir", type=Path, required=True)
    aggregate.add_argument("--output-dir", type=Path, required=True)
    aggregate.add_argument("--source-commit", required=True)
    aggregate.add_argument("--release-tag", default="")
    aggregate.add_argument(
        "--attestations",
        choices=("optional", "required"),
        required=True,
    )
    return parser.parse_args()


def validate_group_id(group_id: str) -> str:
    if not GROUP_ID_PATTERN.fullmatch(group_id):
        raise ReleaseIntegrityError(f"invalid artifact group id: {group_id!r}")
    return group_id


def validate_source_commit(source_commit: str) -> str:
    normalized = source_commit.lower()
    if not SHA1_PATTERN.fullmatch(normalized):
        raise ReleaseIntegrityError(
            "source commit must be a full 40-character lowercase SHA-1"
        )
    return normalized


def validate_release_tag(release_tag: str) -> str | None:
    if not release_tag:
        return None
    if not SEMVER_TAG_PATTERN.fullmatch(release_tag):
        raise ReleaseIntegrityError(
            f"release tag must be empty or exact v-prefixed SemVer: {release_tag!r}"
        )
    return release_tag


def validate_basename(name: str, description: str) -> str:
    if (
        not name
        or name in {".", ".."}
        or Path(name).name != name
        or "/" in name
        or "\\" in name
    ):
        raise ReleaseIntegrityError(f"{description} must be a safe basename: {name!r}")
    return name


def validate_subject_glob(pattern: str) -> str:
    try:
        validate_basename(pattern, "subject glob")
    except ReleaseIntegrityError as error:
        raise ReleaseIntegrityError(
            f"subject glob must be a safe basename glob: {pattern!r}"
        ) from error
    return pattern


def require_regular_file(path: Path, description: str) -> Path:
    if path.is_symlink() or not path.is_file():
        raise ReleaseIntegrityError(f"{description} must be a regular file: {path}")
    return path


def hash_file(path: Path) -> FileEvidence:
    require_regular_file(path, "release evidence")
    digest = hashlib.sha256()
    size = 0
    try:
        with path.open("rb") as release_file:
            while chunk := release_file.read(1024 * 1024):
                digest.update(chunk)
                size += len(chunk)
    except OSError as error:
        raise ReleaseIntegrityError(f"cannot hash {path}: {error}") from error
    if size == 0:
        raise ReleaseIntegrityError(f"release evidence is empty: {path}")
    return FileEvidence(name=path.name, sha256=digest.hexdigest(), size=size)


def load_json(path: Path, description: str) -> dict[str, Any]:
    require_regular_file(path, description)
    try:
        raw = path.read_bytes()
    except OSError as error:
        raise ReleaseIntegrityError(f"cannot read {description} {path}: {error}") from error
    if not raw:
        raise ReleaseIntegrityError(f"{description} is empty: {path}")
    try:
        document = json.loads(raw)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ReleaseIntegrityError(f"invalid JSON in {description} {path}: {error}") from error
    if not isinstance(document, dict):
        raise ReleaseIntegrityError(f"{description} must contain a JSON object: {path}")
    return document


def validate_spdx(path: Path) -> FileEvidence:
    require_regular_file(path, "SPDX SBOM")
    try:
        sbom_size = path.stat().st_size
    except OSError as error:
        raise ReleaseIntegrityError(
            f"cannot inspect SPDX SBOM {path}: {error}"
        ) from error
    if sbom_size > MAX_SBOM_BYTES:
        raise ReleaseIntegrityError(
            f"SPDX SBOM exceeds the 16 MiB attestation limit: {path}"
        )
    document = load_json(path, "SPDX SBOM")
    required = {
        "spdxVersion": "SPDX-2.3",
        "SPDXID": "SPDXRef-DOCUMENT",
        "dataLicense": "CC0-1.0",
    }
    for key, expected in required.items():
        if document.get(key) != expected:
            raise ReleaseIntegrityError(
                f"SPDX SBOM {path} must use {key}={expected!r}"
            )
    return hash_file(path)


def write_json(path: Path, document: dict[str, Any]) -> None:
    if path.exists() and (path.is_symlink() or not path.is_file()):
        raise ReleaseIntegrityError(f"output must be a regular file: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    serialized = (
        json.dumps(document, indent=2, sort_keys=True, ensure_ascii=True) + "\n"
    ).encode("utf-8")
    try:
        path.write_bytes(serialized)
    except OSError as error:
        raise ReleaseIntegrityError(f"cannot write {path}: {error}") from error


def build_group_manifest(
    *,
    group_id: str,
    family: str,
    subjects_dir: Path,
    subject_glob: str,
    sbom_path: Path,
    source_commit: str,
    release_tag: str,
    output_path: Path,
) -> None:
    group_id = validate_group_id(group_id)
    source_commit = validate_source_commit(source_commit)
    normalized_tag = validate_release_tag(release_tag)
    subject_glob = validate_subject_glob(subject_glob)
    if subjects_dir.is_symlink() or not subjects_dir.is_dir():
        raise ReleaseIntegrityError(
            f"release subjects directory must be a real directory: {subjects_dir}"
        )

    bundle_root = output_path.parent.parent.resolve()
    expected_subjects_dir = bundle_root / "subjects"
    if subjects_dir.resolve() != expected_subjects_dir:
        raise ReleaseIntegrityError(
            f"release subjects must be beneath {expected_subjects_dir}"
        )
    if sbom_path.parent.resolve() != bundle_root / "integrity":
        raise ReleaseIntegrityError(
            f"SPDX SBOM must be beneath {bundle_root / 'integrity'}"
        )

    candidates = sorted(subjects_dir.glob(subject_glob), key=lambda path: path.name)
    if not candidates:
        raise ReleaseIntegrityError(
            f"no release subjects match {subject_glob!r} in {subjects_dir}"
        )
    subjects: list[FileEvidence] = []
    seen: set[str] = set()
    for candidate in candidates:
        validate_basename(candidate.name, "release subject")
        if candidate.name in seen:
            raise ReleaseIntegrityError(f"duplicate release subject: {candidate.name}")
        seen.add(candidate.name)
        subjects.append(hash_file(candidate))

    sbom = validate_spdx(sbom_path)
    manifest = {
        "family": family,
        "group_id": group_id,
        "release_tag": normalized_tag,
        "sbom": sbom.as_dict(),
        "schema_version": 1,
        "source_commit": source_commit,
        "subjects": [subject.as_dict() for subject in subjects],
    }
    write_json(output_path, manifest)
    print(
        f"group={group_id} family={family} subjects={len(subjects)} "
        "outcome=integrity-recorded",
        flush=True,
    )


def require_keys(document: dict[str, Any], expected: set[str], context: str) -> None:
    actual = set(document)
    if actual != expected:
        raise ReleaseIntegrityError(
            f"{context} has invalid keys: "
            f"missing={sorted(expected - actual)}, unexpected={sorted(actual - expected)}"
        )


def parse_file_evidence(raw: object, context: str) -> FileEvidence:
    if not isinstance(raw, dict):
        raise ReleaseIntegrityError(f"{context} must be an object")
    require_keys(raw, {"name", "sha256", "size"}, context)
    name = raw["name"]
    sha256 = raw["sha256"]
    size = raw["size"]
    if not isinstance(name, str):
        raise ReleaseIntegrityError(f"{context} has an invalid name")
    validate_basename(name, context)
    if not isinstance(sha256, str) or not SHA256_PATTERN.fullmatch(sha256):
        raise ReleaseIntegrityError(f"{context} has an invalid SHA-256")
    if type(size) is not int or size <= 0:
        raise ReleaseIntegrityError(f"{context} has an invalid size")
    return FileEvidence(name=name, sha256=sha256, size=size)


def verify_file_evidence(path: Path, expected: FileEvidence, context: str) -> None:
    observed = hash_file(path)
    if observed != expected:
        raise ReleaseIntegrityError(
            f"{context} digest or size drift: expected={expected.as_dict()}, "
            f"observed={observed.as_dict()}"
        )


def validate_sigstore_bundle(path: Path, description: str) -> FileEvidence:
    document = load_json(path, description)
    media_type = document.get("mediaType")
    if not isinstance(media_type, str) or not media_type.startswith(
        SIGSTORE_MEDIA_TYPE_PREFIX
    ):
        raise ReleaseIntegrityError(f"{description} has an invalid Sigstore mediaType")
    for key in ("verificationMaterial", "dsseEnvelope"):
        if not isinstance(document.get(key), dict):
            raise ReleaseIntegrityError(f"{description} omits object {key}")
    return hash_file(path)


def load_group(
    manifest_path: Path,
    bundle_root: Path,
    *,
    source_commit: str,
    release_tag: str | None,
    attestations: str,
) -> GroupEvidence:
    document = load_json(manifest_path, "artifact group manifest")
    require_keys(
        document,
        {
            "schema_version",
            "group_id",
            "family",
            "source_commit",
            "release_tag",
            "subjects",
            "sbom",
        },
        f"artifact group manifest {manifest_path.name}",
    )
    if document["schema_version"] != 1:
        raise ReleaseIntegrityError(f"unsupported group schema in {manifest_path}")
    group_id = document["group_id"]
    family = document["family"]
    if not isinstance(group_id, str):
        raise ReleaseIntegrityError(f"invalid group id in {manifest_path}")
    validate_group_id(group_id)
    if manifest_path.name != f"{group_id}.group.json":
        raise ReleaseIntegrityError(f"group manifest filename mismatch: {manifest_path}")
    if family not in REQUIRED_FAMILIES:
        raise ReleaseIntegrityError(f"invalid artifact family in {manifest_path}: {family}")
    if document["source_commit"] != source_commit:
        raise ReleaseIntegrityError(f"source commit mismatch in {manifest_path}")
    if document["release_tag"] != release_tag:
        raise ReleaseIntegrityError(f"release tag mismatch in {manifest_path}")

    raw_subjects = document["subjects"]
    if not isinstance(raw_subjects, list) or not raw_subjects:
        raise ReleaseIntegrityError(f"group {group_id} has no release subjects")
    subjects = tuple(
        parse_file_evidence(raw, f"group {group_id} subject") for raw in raw_subjects
    )
    if [subject.name for subject in subjects] != sorted(
        subject.name for subject in subjects
    ):
        raise ReleaseIntegrityError(f"group {group_id} subjects are not sorted")
    if len({subject.name for subject in subjects}) != len(subjects):
        raise ReleaseIntegrityError(f"group {group_id} repeats a release subject")
    for subject in subjects:
        verify_file_evidence(
            bundle_root / "subjects" / subject.name,
            subject,
            f"group {group_id} subject",
        )

    sbom = parse_file_evidence(document["sbom"], f"group {group_id} SBOM")
    sbom_path = bundle_root / "integrity" / sbom.name
    observed_sbom = validate_spdx(sbom_path)
    if observed_sbom != sbom:
        raise ReleaseIntegrityError(f"group {group_id} SPDX SBOM digest or size drift")

    provenance_path = (
        bundle_root / "integrity" / f"{group_id}.provenance.sigstore.json"
    )
    sbom_bundle_path = bundle_root / "integrity" / f"{group_id}.sbom.sigstore.json"
    provenance_exists = provenance_path.exists() or provenance_path.is_symlink()
    sbom_bundle_exists = sbom_bundle_path.exists() or sbom_bundle_path.is_symlink()
    if attestations == "required" and not provenance_exists:
        raise ReleaseIntegrityError(
            f"group {group_id} is missing its provenance attestation bundle"
        )
    if attestations == "required" and not sbom_bundle_exists:
        raise ReleaseIntegrityError(
            f"group {group_id} is missing its SBOM attestation bundle"
        )
    if provenance_exists != sbom_bundle_exists:
        raise ReleaseIntegrityError(
            f"group {group_id} must retain both provenance and SBOM attestations"
        )
    provenance = (
        validate_sigstore_bundle(provenance_path, "provenance attestation bundle")
        if provenance_exists
        else None
    )
    sbom_bundle = (
        validate_sigstore_bundle(sbom_bundle_path, "SBOM attestation bundle")
        if sbom_bundle_exists
        else None
    )
    return GroupEvidence(
        group_id=group_id,
        family=family,
        subjects=subjects,
        sbom=sbom,
        manifest=hash_file(manifest_path),
        provenance_bundle=provenance,
        sbom_bundle=sbom_bundle,
    )


def aggregate_release(
    *,
    bundle_dir: Path,
    output_dir: Path,
    source_commit: str,
    release_tag: str,
    attestations: str,
) -> None:
    source_commit = validate_source_commit(source_commit)
    normalized_tag = validate_release_tag(release_tag)
    if bundle_dir.is_symlink() or not bundle_dir.is_dir():
        raise ReleaseIntegrityError(f"release bundle must be a real directory: {bundle_dir}")
    bundle_root = bundle_dir.resolve()
    subjects_dir = bundle_root / "subjects"
    integrity_dir = bundle_root / "integrity"
    if subjects_dir.is_symlink() or not subjects_dir.is_dir():
        raise ReleaseIntegrityError(f"release bundle omits subjects directory: {bundle_dir}")
    if integrity_dir.is_symlink() or not integrity_dir.is_dir():
        raise ReleaseIntegrityError(f"release bundle omits integrity directory: {bundle_dir}")

    manifests = sorted(integrity_dir.glob("*.group.json"), key=lambda path: path.name)
    if not manifests:
        raise ReleaseIntegrityError("release bundle has no artifact group manifests")
    groups = tuple(
        load_group(
            manifest,
            bundle_root,
            source_commit=source_commit,
            release_tag=normalized_tag,
            attestations=attestations,
        )
        for manifest in manifests
    )
    if len({group.group_id for group in groups}) != len(groups):
        raise ReleaseIntegrityError("release bundle repeats an artifact group id")
    families = {group.family for group in groups}
    missing_families = sorted(REQUIRED_FAMILIES - families)
    if missing_families:
        raise ReleaseIntegrityError(
            f"release bundle is missing artifact families: {missing_families}"
        )

    subject_groups: dict[str, str] = {}
    artifacts: list[dict[str, str | int]] = []
    for group in groups:
        for subject in group.subjects:
            prior_group = subject_groups.get(subject.name)
            if prior_group is not None:
                raise ReleaseIntegrityError(
                    f"release subject {subject.name} belongs to both "
                    f"{prior_group} and {group.group_id}"
                )
            subject_groups[subject.name] = group.group_id
            artifacts.append(
                {
                    "family": group.family,
                    "group_id": group.group_id,
                    **subject.as_dict(),
                }
            )

    actual_subjects: set[str] = set()
    for path in subjects_dir.iterdir():
        if path.is_symlink() or not path.is_file():
            raise ReleaseIntegrityError(
                f"release subjects directory contains a non-file: {path}"
            )
        validate_basename(path.name, "release subject")
        actual_subjects.add(path.name)
    registered_subjects = set(subject_groups)
    unregistered = sorted(actual_subjects - registered_subjects)
    missing = sorted(registered_subjects - actual_subjects)
    if unregistered:
        raise ReleaseIntegrityError(f"unregistered release subject(s): {unregistered}")
    if missing:
        raise ReleaseIntegrityError(f"registered release subject(s) missing: {missing}")

    artifacts.sort(key=lambda artifact: str(artifact["name"]))
    checksum_text = "".join(
        f"{artifact['sha256']}  {artifact['name']}\n" for artifact in artifacts
    )
    group_index: list[dict[str, Any]] = []
    for group in groups:
        entry: dict[str, Any] = {
            "family": group.family,
            "group_id": group.group_id,
            "manifest": group.manifest.as_dict(),
            "sbom": group.sbom.as_dict(),
            "subjects": [subject.name for subject in group.subjects],
        }
        if group.provenance_bundle is not None and group.sbom_bundle is not None:
            entry["provenance_attestation"] = group.provenance_bundle.as_dict()
            entry["sbom_attestation"] = group.sbom_bundle.as_dict()
        group_index.append(entry)

    if output_dir.exists() and (output_dir.is_symlink() or not output_dir.is_dir()):
        raise ReleaseIntegrityError(
            f"release integrity output must be a real directory: {output_dir}"
        )
    output_dir.mkdir(parents=True, exist_ok=True)
    try:
        (output_dir / "SHA256SUMS").write_text(checksum_text, encoding="utf-8")
    except OSError as error:
        raise ReleaseIntegrityError(f"cannot write SHA256SUMS: {error}") from error
    write_json(
        output_dir / "release-artifacts.json",
        {
            "artifact_count": len(artifacts),
            "artifacts": artifacts,
            "attestations": attestations,
            "groups": group_index,
            "release_tag": normalized_tag,
            "repository": REPOSITORY,
            "schema_version": 1,
            "source_commit": source_commit,
        },
    )
    print(
        f"groups={len(groups)} artifacts={len(artifacts)} "
        f"attestations={attestations} outcome=release-integrity-complete",
        flush=True,
    )


def main() -> int:
    arguments = parse_args()
    try:
        if arguments.command == "group":
            build_group_manifest(
                group_id=arguments.group_id,
                family=arguments.family,
                subjects_dir=arguments.subjects_dir,
                subject_glob=arguments.subject_glob,
                sbom_path=arguments.sbom,
                source_commit=arguments.source_commit,
                release_tag=arguments.release_tag,
                output_path=arguments.output,
            )
        else:
            aggregate_release(
                bundle_dir=arguments.bundle_dir,
                output_dir=arguments.output_dir,
                source_commit=arguments.source_commit,
                release_tag=arguments.release_tag,
                attestations=arguments.attestations,
            )
    except ReleaseIntegrityError as error:
        print(f"release integrity failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
