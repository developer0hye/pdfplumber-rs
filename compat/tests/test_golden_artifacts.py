"""Contract tests for provenance in the committed golden corpus (PARITY-002)."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from compat.harness import lockfile, provenance, upstream


GOLDEN_ROOT: Path = (
    upstream.REPO_ROOT / "crates" / "pdfplumber" / "tests" / "fixtures" / "golden"
)
PDF_ROOT: Path = (
    upstream.REPO_ROOT / "crates" / "pdfplumber" / "tests" / "fixtures" / "pdfs"
)
REQUIRED_FIELDS: tuple[str, ...] = (
    "upstream_project",
    "upstream_version",
    "upstream_tag",
    "upstream_commit",
    "lockfile_sha256",
    "fixture_path",
    "fixture_sha256",
    "generated_by",
    "python_version",
    "platform_system",
    "platform_machine",
)


class GoldenArtifactProvenanceTest(unittest.TestCase):
    def test_every_committed_artifact_has_valid_provenance(self) -> None:
        artifacts: list[Path] = sorted(GOLDEN_ROOT.rglob("*.json"))
        self.assertGreater(
            len(artifacts),
            1,
            "the corpus-wide contract requires materially different fixtures",
        )

        target: upstream.Target = upstream.load_target()
        environment: upstream.Environment = upstream.load_environment()
        expected_lock_hash: str = lockfile.digest()
        errors: list[str] = []

        for artifact in artifacts:
            relative_artifact: Path = artifact.relative_to(GOLDEN_ROOT)
            fixture: Path = PDF_ROOT / relative_artifact.with_suffix(".pdf")
            artifact_name: str = relative_artifact.as_posix()

            if not fixture.is_file():
                errors.append(f"{artifact_name}: matching fixture is missing")
                continue

            payload: object = json.loads(artifact.read_text(encoding="utf-8"))
            if not isinstance(payload, dict):
                errors.append(f"{artifact_name}: root is not an object")
                continue

            record: object = payload.get("provenance")
            if not isinstance(record, dict):
                errors.append(f"{artifact_name}: provenance is missing")
                continue

            for field in REQUIRED_FIELDS:
                if not record.get(field):
                    errors.append(f"{artifact_name}: {field} is missing or empty")

            expected_values: dict[str, str] = {
                "upstream_project": target.project,
                "upstream_version": target.version,
                "upstream_tag": target.tag,
                "upstream_commit": target.commit,
                "lockfile_sha256": expected_lock_hash,
                "fixture_path": fixture.relative_to(upstream.REPO_ROOT).as_posix(),
                "fixture_sha256": provenance.file_sha256(fixture),
                "generated_by": provenance.GENERATION_COMMAND,
            }
            for field, expected in expected_values.items():
                actual: object = record.get(field)
                if actual != expected:
                    errors.append(
                        f"{artifact_name}: {field} is {actual!r}, expected {expected!r}"
                    )

            python_version: object = record.get("python_version")
            if not isinstance(python_version, str) or not python_version.startswith(
                f"{environment.python_version}."
            ):
                errors.append(
                    f"{artifact_name}: python_version is {python_version!r}, "
                    f"expected {environment.python_version}.x"
                )

        if errors:
            preview: str = "\n".join(errors[:20])
            self.fail(
                f"{len(errors)} golden-artifact provenance violation(s):\n{preview}"
            )


if __name__ == "__main__":
    unittest.main()
