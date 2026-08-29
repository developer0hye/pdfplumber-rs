"""Release checksum, SBOM, provenance, and attestation contracts (DIST-005)."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CHECKER_PATH = REPO_ROOT / "scripts" / "build_release_integrity.py"
RELEASE_ARTIFACTS_PATH = REPO_ROOT / ".github" / "workflows" / "release-artifacts.yml"
CLI_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "cli-binaries.yml"
CI_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "cli-binaries-ci.yml"
RELEASE_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
GUIDE_PATH = REPO_ROOT / "docs" / "release-integrity.md"
REFERENCE_PATH = REPO_ROOT / "references" / "release-artifact-integrity.md"
SOURCE_COMMIT = "1" * 40


class ReleaseArtifactIntegrityTests(unittest.TestCase):
    def checker(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        self.assertTrue(CHECKER_PATH.is_file(), "missing release integrity checker")
        return subprocess.run(
            (sys.executable, str(CHECKER_PATH), *arguments),
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    @staticmethod
    def write_subject(path: Path, content: bytes) -> str:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        return hashlib.sha256(content).hexdigest()

    @staticmethod
    def write_sbom(path: Path, group_id: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(
                {
                    "SPDXID": "SPDXRef-DOCUMENT",
                    "creationInfo": {"creators": ["Tool: syft"]},
                    "dataLicense": "CC0-1.0",
                    "documentNamespace": f"https://example.invalid/{group_id}",
                    "name": group_id,
                    "packages": [],
                    "spdxVersion": "SPDX-2.3",
                },
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )

    def record_group(
        self,
        bundle: Path,
        *,
        group_id: str,
        family: str,
        subject_name: str,
    ) -> subprocess.CompletedProcess[str]:
        subjects = bundle / "subjects"
        integrity = bundle / "integrity"
        sbom = integrity / f"{group_id}.spdx.json"
        self.write_sbom(sbom, group_id)
        return self.checker(
            "group",
            "--group-id",
            group_id,
            "--family",
            family,
            "--subjects-dir",
            str(subjects),
            "--subject-glob",
            subject_name,
            "--sbom",
            str(sbom),
            "--source-commit",
            SOURCE_COMMIT,
            "--release-tag",
            "v1.2.3",
            "--output",
            str(integrity / f"{group_id}.group.json"),
        )

    def complete_bundle(self, root: Path) -> tuple[Path, dict[str, str]]:
        bundle = root / "bundle"
        subjects = bundle / "subjects"
        fixtures = {
            "rust-crates": (
                "rust-crate",
                "pdfplumber-core-1.2.3.crate",
            ),
            "python-wheels-linux-x86_64": (
                "python-wheel",
                "pdfplumber_rs-1.2.3-cp313-cp313-manylinux.whl",
            ),
            "python-sdist": (
                "python-sdist",
                "pdfplumber_rs-1.2.3.tar.gz",
            ),
            "cli-x86_64-pc-windows-msvc": (
                "cli-binary",
                "pdfplumber-cli-1.2.3-x86_64-pc-windows-msvc.zip",
            ),
        }
        digests: dict[str, str] = {}
        for group_id, (family, subject_name) in fixtures.items():
            digests[subject_name] = self.write_subject(
                subjects / subject_name,
                f"{group_id}\n".encode(),
            )
            result = self.record_group(
                bundle,
                group_id=group_id,
                family=family,
                subject_name=subject_name,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
        return bundle, digests

    def test_group_manifest_binds_every_subject_and_spdx_document(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            bundle = root / "bundle"
            subjects = bundle / "subjects"
            first_digest = self.write_subject(subjects / "one.whl", b"wheel-one")
            second_digest = self.write_subject(subjects / "two.whl", b"wheel-two")
            integrity = bundle / "integrity"
            sbom = integrity / "python-wheels.spdx.json"
            self.write_sbom(sbom, "python-wheels")
            output = integrity / "python-wheels.group.json"

            result = self.checker(
                "group",
                "--group-id",
                "python-wheels",
                "--family",
                "python-wheel",
                "--subjects-dir",
                str(subjects),
                "--subject-glob",
                "*.whl",
                "--sbom",
                str(sbom),
                "--source-commit",
                SOURCE_COMMIT,
                "--release-tag",
                "v1.2.3",
                "--output",
                str(output),
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            manifest = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(manifest["schema_version"], 1)
            self.assertEqual(manifest["group_id"], "python-wheels")
            self.assertEqual(manifest["family"], "python-wheel")
            self.assertEqual(manifest["source_commit"], SOURCE_COMMIT)
            self.assertEqual(manifest["release_tag"], "v1.2.3")
            self.assertEqual(
                manifest["subjects"],
                [
                    {
                        "name": "one.whl",
                        "sha256": first_digest,
                        "size": len(b"wheel-one"),
                    },
                    {
                        "name": "two.whl",
                        "sha256": second_digest,
                        "size": len(b"wheel-two"),
                    },
                ],
            )
            self.assertEqual(manifest["sbom"]["name"], "python-wheels.spdx.json")
            self.assertEqual(
                manifest["sbom"]["sha256"],
                hashlib.sha256(sbom.read_bytes()).hexdigest(),
            )

    def test_group_manifest_rejects_empty_or_unsafe_subject_selection(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            subjects = root / "bundle" / "subjects"
            subjects.mkdir(parents=True)
            sbom = root / "bundle" / "integrity" / "empty.spdx.json"
            self.write_sbom(sbom, "empty")
            common = (
                "group",
                "--group-id",
                "empty",
                "--family",
                "python-wheel",
                "--subjects-dir",
                str(subjects),
                "--sbom",
                str(sbom),
                "--source-commit",
                SOURCE_COMMIT,
                "--output",
                str(sbom.with_name("empty.group.json")),
            )

            empty = self.checker(*common, "--subject-glob", "*.whl")
            self.assertNotEqual(empty.returncode, 0)
            self.assertIn("no release subjects", empty.stderr)

            unsafe = self.checker(*common, "--subject-glob", "../*")
            self.assertNotEqual(unsafe.returncode, 0)
            self.assertIn("safe basename glob", unsafe.stderr)

    def test_aggregate_rejects_missing_family_and_unregistered_subjects(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            bundle, _ = self.complete_bundle(root)
            output = root / "output"

            passing = self.checker(
                "aggregate",
                "--bundle-dir",
                str(bundle),
                "--output-dir",
                str(output),
                "--source-commit",
                SOURCE_COMMIT,
                "--release-tag",
                "v1.2.3",
                "--attestations",
                "optional",
            )
            self.assertEqual(passing.returncode, 0, passing.stderr)

            extra = bundle / "subjects" / "unregistered.whl"
            self.write_subject(extra, b"not in a group")
            unregistered = self.checker(
                "aggregate",
                "--bundle-dir",
                str(bundle),
                "--output-dir",
                str(output),
                "--source-commit",
                SOURCE_COMMIT,
                "--release-tag",
                "v1.2.3",
                "--attestations",
                "optional",
            )
            self.assertNotEqual(unregistered.returncode, 0)
            self.assertIn("unregistered release subject", unregistered.stderr)

            (bundle / "integrity" / "python-sdist.group.json").unlink()
            missing_family = self.checker(
                "aggregate",
                "--bundle-dir",
                str(bundle),
                "--output-dir",
                str(output),
                "--source-commit",
                SOURCE_COMMIT,
                "--release-tag",
                "v1.2.3",
                "--attestations",
                "optional",
            )
            self.assertNotEqual(missing_family.returncode, 0)
            self.assertIn("missing artifact families", missing_family.stderr)

    def test_required_attestations_need_provenance_and_sbom_bundles(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            bundle, _ = self.complete_bundle(root)
            integrity = bundle / "integrity"
            output = root / "output"
            arguments = (
                "aggregate",
                "--bundle-dir",
                str(bundle),
                "--output-dir",
                str(output),
                "--source-commit",
                SOURCE_COMMIT,
                "--release-tag",
                "v1.2.3",
                "--attestations",
                "required",
            )

            absent = self.checker(*arguments)
            self.assertNotEqual(absent.returncode, 0)
            self.assertIn("provenance attestation bundle", absent.stderr)

            bundle_document = json.dumps(
                {
                    "dsseEnvelope": {},
                    "mediaType": "application/vnd.dev.sigstore.bundle.v0.3+json",
                    "verificationMaterial": {},
                },
                sort_keys=True,
            ).encode()
            for manifest in integrity.glob("*.group.json"):
                group_id = manifest.name.removesuffix(".group.json")
                (integrity / f"{group_id}.provenance.sigstore.json").write_bytes(
                    bundle_document
                )
                (integrity / f"{group_id}.sbom.sigstore.json").write_bytes(
                    bundle_document
                )

            present = self.checker(*arguments)
            self.assertEqual(present.returncode, 0, present.stderr)

    def test_checksum_and_release_index_are_deterministic(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            bundle, digests = self.complete_bundle(root)
            output = root / "output"
            arguments = (
                "aggregate",
                "--bundle-dir",
                str(bundle),
                "--output-dir",
                str(output),
                "--source-commit",
                SOURCE_COMMIT,
                "--release-tag",
                "v1.2.3",
                "--attestations",
                "optional",
            )

            first = self.checker(*arguments)
            self.assertEqual(first.returncode, 0, first.stderr)
            checksum_bytes = (output / "SHA256SUMS").read_bytes()
            index_bytes = (output / "release-artifacts.json").read_bytes()
            second = self.checker(*arguments)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assertEqual((output / "SHA256SUMS").read_bytes(), checksum_bytes)
            self.assertEqual(
                (output / "release-artifacts.json").read_bytes(), index_bytes
            )

            checksum_lines = checksum_bytes.decode().splitlines()
            expected_lines = [
                f"{digest}  {name}" for name, digest in sorted(digests.items())
            ]
            self.assertEqual(checksum_lines, expected_lines)
            index = json.loads(index_bytes)
            self.assertEqual(index["source_commit"], SOURCE_COMMIT)
            self.assertEqual(index["release_tag"], "v1.2.3")
            self.assertEqual(
                {artifact["family"] for artifact in index["artifacts"]},
                {"cli-binary", "python-sdist", "python-wheel", "rust-crate"},
            )
            self.assertNotIn("spdx.json", checksum_bytes.decode())
            self.assertNotIn("sigstore.json", checksum_bytes.decode())

    def test_workflows_attest_every_release_artifact_before_publication(self) -> None:
        for path in (
            RELEASE_ARTIFACTS_PATH,
            CLI_WORKFLOW_PATH,
            CI_WORKFLOW_PATH,
            RELEASE_WORKFLOW_PATH,
        ):
            with self.subTest(path=path.name):
                self.assertTrue(
                    path.is_file(), f"missing {path.relative_to(REPO_ROOT)}"
                )

        package_workflow = RELEASE_ARTIFACTS_PATH.read_text(encoding="utf-8")
        for phrase in (
            "workflow_call:",
            "Build verified crates.io archives",
            "Build wheel (${{ matrix.os }}, ${{ matrix.target }})",
            "Build source distribution",
            "anchore/sbom-action@v0",
            "actions/attest@v4",
            "sbom-path:",
            "python scripts/build_release_integrity.py group",
            "actions/upload-artifact@v4",
        ):
            with self.subTest(package_workflow=phrase):
                self.assertIn(phrase, package_workflow)
        self.assertGreaterEqual(package_workflow.count("uses: actions/attest@v4"), 6)

        cli_workflow = CLI_WORKFLOW_PATH.read_text(encoding="utf-8")
        for phrase in (
            "attest:",
            "anchore/sbom-action@v0",
            "python scripts/build_release_integrity.py group",
            "subject-path:",
            "sbom-path:",
        ):
            with self.subTest(cli_workflow=phrase):
                self.assertIn(phrase, cli_workflow)
        self.assertEqual(cli_workflow.count("uses: actions/attest@v4"), 2)

        ci_workflow = CI_WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertIn("uses: ./.github/workflows/release-artifacts.yml", ci_workflow)
        self.assertEqual(
            ci_workflow.count("attest: ${{ github.event_name == 'push' }}"),
            2,
        )
        self.assertIn(
            "python scripts/build_release_integrity.py aggregate", ci_workflow
        )
        self.assertIn("--attestations", ci_workflow)

        release_workflow = RELEASE_WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertIn(
            "uses: ./.github/workflows/release-artifacts.yml", release_workflow
        )
        self.assertEqual(release_workflow.count("attest: true"), 2)
        self.assertIn(
            "python scripts/build_release_integrity.py aggregate", release_workflow
        )
        self.assertIn("--attestations required", release_workflow)
        integrity = release_workflow.index("\n  integrity:")
        rust_publish = release_workflow.index("\n  publish:")
        pypi_publish = release_workflow.index("\n  publish-pypi:")
        github_release = release_workflow.index("\n  release:")
        self.assertLess(integrity, rust_publish)
        self.assertLess(integrity, pypi_publish)
        self.assertLess(integrity, github_release)
        self.assertIn("release-integrity/**", release_workflow)

    def test_public_verification_guide_and_reference_are_source_backed(self) -> None:
        self.assertTrue(GUIDE_PATH.is_file(), "missing release integrity guide")
        guide = GUIDE_PATH.read_text(encoding="utf-8")
        for phrase in (
            "SHA256SUMS",
            "release-artifacts.json",
            "SPDX 2.3",
            "gh attestation verify",
            "https://spdx.dev/Document/v2.3",
            "release archives",
            "wheels",
            "source distribution",
            "native Command-Line Interface binaries",
            "DIST-007",
        ):
            with self.subTest(guide=phrase):
                self.assertIn(phrase, guide)

        root_readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        cli_guide = (REPO_ROOT / "docs" / "cli-binaries.md").read_text(encoding="utf-8")
        crates_guide = (REPO_ROOT / "docs" / "crates-release.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("docs/release-integrity.md", root_readme)
        self.assertIn("release-integrity.md", cli_guide)
        self.assertIn("release-integrity.md", crates_guide)
        self.assertNotIn(
            "Integrity and provenance assets remain a separate task", cli_guide
        )

        self.assertTrue(REFERENCE_PATH.is_file(), "missing integrity reference note")
        reference = REFERENCE_PATH.read_text(encoding="utf-8")
        self.assertLessEqual(len(reference.splitlines()), 50)
        for phrase in (
            "actions/attest",
            "Anchore",
            "Sigstore",
            "SPDX",
        ):
            with self.subTest(reference=phrase):
                self.assertIn(phrase, reference)
        index = (REPO_ROOT / "references" / "INDEX.md").read_text(encoding="utf-8")
        self.assertIn("release-artifact-integrity.md", index)


if __name__ == "__main__":
    unittest.main()
