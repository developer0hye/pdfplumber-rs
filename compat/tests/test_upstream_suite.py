"""Reproducible upstream-suite runner contracts (PARITY-014)."""

from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from compat.harness import lockfile, upstream, upstream_suite


SOURCE_MANIFEST: Path = upstream.REPO_ROOT / "compat" / "upstream-suite.toml"
UNSUPPORTED_MANIFEST: Path = (
    upstream.REPO_ROOT / "compat" / "upstream-unsupported.toml"
)


class UpstreamSuiteContractTests(unittest.TestCase):
    def test_source_manifest_pins_the_exact_upstream_test_tree(self) -> None:
        config = upstream_suite.load_source_manifest(SOURCE_MANIFEST)
        target = upstream.load_target()
        self.assertEqual(config.project, target.project)
        self.assertEqual(config.version, target.version)
        self.assertEqual(config.tag, target.tag)
        self.assertEqual(config.commit, target.commit)
        self.assertEqual(config.repository, target.repository)
        self.assertRegex(config.tests_tree, r"^[0-9a-f]{40}$")
        self.assertEqual(
            config.suite_paths,
            (
                Path("tests"),
                Path("examples/pdfs/ag-energy-round-up-2017-02-24.pdf"),
            ),
        )
        self.assertRegex(config.tests_sha256, r"^[0-9a-f]{64}$")
        self.assertEqual(config.tests_file_count, 102)
        self.assertEqual(
            config.requirements_path,
            Path("compat/requirements-upstream-tests.txt"),
        )
        self.assertEqual(config.external_commands, ("gs",))
        requirements = upstream.REPO_ROOT / config.requirements_path
        self.assertEqual(
            config.requirements_sha256,
            hashlib.sha256(requirements.read_bytes()).hexdigest(),
        )
        self.assertNotEqual(config.requirements_sha256, lockfile.digest())

    def test_unsupported_manifest_is_machine_readable_and_target_bound(self) -> None:
        manifest = upstream_suite.load_unsupported_manifest(UNSUPPORTED_MANIFEST)
        target = upstream.load_target()
        self.assertEqual(manifest.version, target.version)
        self.assertEqual(manifest.commit, target.commit)
        self.assertEqual(manifest.tests, ())

    def test_tree_digest_is_order_independent_and_detects_changes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "nested").mkdir()
            (root / "b.py").write_text("b\n", encoding="utf-8")
            (root / "nested" / "a.py").write_text("a\n", encoding="utf-8")
            first = upstream_suite.tree_fingerprint(root)
            self.assertEqual(first.file_count, 2)
            self.assertEqual(first, upstream_suite.tree_fingerprint(root))
            (root / "nested" / "a.py").write_text("changed\n", encoding="utf-8")
            self.assertNotEqual(first, upstream_suite.tree_fingerprint(root))

    def test_materialized_suite_requires_verified_source_metadata(self) -> None:
        config = upstream_suite.load_source_manifest(SOURCE_MANIFEST)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(
                upstream_suite.SuiteSourceMismatch,
                "source metadata is missing",
            ):
                upstream_suite.verify_materialized_suite(root, config)

            (root / upstream_suite.SOURCE_METADATA_NAME).write_text(
                json.dumps(
                    {
                        "commit": config.commit,
                        "tests_tree": config.tests_tree,
                        "tests_sha256": config.tests_sha256,
                        "tests_file_count": config.tests_file_count,
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                upstream_suite.SuiteSourceMismatch,
                "content fingerprint",
            ):
                upstream_suite.verify_materialized_suite(root, config)

    def test_result_classification_never_turns_failures_into_success(self) -> None:
        manifest = upstream_suite.UnsupportedManifest(
            version="0.11.10",
            commit="7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
            tests=(
                upstream_suite.UnsupportedTest(
                    nodeid="test_module.py::test_known",
                    task_id="PYAPI-001",
                    reason="temporary compatibility gap",
                ),
                upstream_suite.UnsupportedTest(
                    nodeid="test_module.py::test_stale",
                    task_id="PYAPI-002",
                    reason="expected to remain unsupported",
                ),
            ),
        )
        result = upstream_suite.classify_results(
            collected=(
                "test_module.py::test_known",
                "test_module.py::test_new",
                "test_module.py::test_stale",
            ),
            failed=("test_module.py::test_known", "test_module.py::test_new"),
            manifest=manifest,
            pytest_exit_code=1,
        )
        self.assertEqual(result.known_unsupported, ("test_module.py::test_known",))
        self.assertEqual(result.unlisted_failures, ("test_module.py::test_new",))
        self.assertEqual(result.stale_unsupported, ("test_module.py::test_stale",))
        self.assertEqual(result.exit_code, 1)

    def test_unsupported_entries_require_existing_unchecked_prd_tasks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            prd = Path(directory) / "PRD.md"
            prd.write_text(
                "- [x] **PYAPI-002** Outside the master checklist.\n"
                "## 8. Master Implementation Checklist\n"
                "- [x] **PARITY-001** Completed task.\n"
                "- [ ] **PYAPI-002** Open task.\n"
                "## 9. Known Open-Issue Mapping\n"
                "- [x] **PYAPI-002** Also outside the master checklist.\n",
                encoding="utf-8",
            )
            valid = upstream_suite.UnsupportedManifest(
                version="0.11.10",
                commit="7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
                tests=(
                    upstream_suite.UnsupportedTest(
                        nodeid="test_module.py::test_open_gap",
                        task_id="PYAPI-002",
                        reason="candidate package is not available",
                    ),
                ),
            )
            upstream_suite.validate_unsupported_task_links(valid, prd)

            for task_id, message in (
                ("PARITY-001", "references checked task PARITY-001"),
                ("UNKNOWN-999", "references unknown task UNKNOWN-999"),
            ):
                invalid = upstream_suite.UnsupportedManifest(
                    version=valid.version,
                    commit=valid.commit,
                    tests=(
                        upstream_suite.UnsupportedTest(
                            nodeid="test_module.py::test_invalid_gap",
                            task_id=task_id,
                            reason="invalid task link",
                        ),
                    ),
                )
                with self.assertRaisesRegex(
                    upstream_suite.UnsupportedManifestError,
                    message,
                ):
                    upstream_suite.validate_unsupported_task_links(invalid, prd)


if __name__ == "__main__":
    unittest.main()
