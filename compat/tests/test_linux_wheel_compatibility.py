"""Contracts for Linux wheel tags and shared-library independence (DIST-010)."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from types import ModuleType
from unittest import mock

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = REPO_ROOT / "python-wheel-targets.toml"
CHECKER_PATH = REPO_ROOT / "scripts" / "check_linux_wheel.py"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release-artifacts.yml"
CI_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "cli-binaries-ci.yml"
GUIDE_PATH = REPO_ROOT / "docs" / "linux-wheels.md"
REFERENCE_PATH = REPO_ROOT / "references" / "python-linux-wheels.md"
REFERENCE_INDEX_PATH = REPO_ROOT / "references" / "INDEX.md"

EXPECTED_AUDITOR = {
    "auditwheel_version": "6.8.1",
    "packaging_version": "26.3",
    "pyelftools_version": "0.33",
}
EXPECTED_TARGETS = {
    "x86_64": {
        "python_tag": "cp313",
        "abi_tag": "cp313",
        "manylinux": "2014",
        "auditwheel_tag": "manylinux_2_17_x86_64",
        "filename_platform_tag": (
            "manylinux_2_17_x86_64.manylinux2014_x86_64"
        ),
    },
    "aarch64": {
        "python_tag": "cp313",
        "abi_tag": "cp313",
        "manylinux": "2014",
        "auditwheel_tag": "manylinux_2_17_aarch64",
        "filename_platform_tag": (
            "manylinux_2_17_aarch64.manylinux2014_aarch64"
        ),
    },
}


def load_checker() -> ModuleType | None:
    if not CHECKER_PATH.is_file():
        return None
    spec = importlib.util.spec_from_file_location("check_linux_wheel", CHECKER_PATH)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def sample_report(target: str, wheel_name: str) -> dict[str, object]:
    return {
        "version": 1,
        "wheel": wheel_name,
        "pure": False,
        "overall_tag": EXPECTED_TARGETS[target]["auditwheel_tag"],
        "sym_tag": EXPECTED_TARGETS[target]["auditwheel_tag"],
        "pyfpe": False,
        "ucs2": False,
        "unsupported_isa": False,
        "versioned_symbols": {
            "libc.so.6": ["GLIBC_2.17"],
            "libgcc_s.so.1": ["GCC_3.0"],
        },
        "external_libs": {},
        "policy_upgrades": {},
    }


class LinuxWheelCompatibilityTests(unittest.TestCase):
    def require_checker(self) -> ModuleType:
        checker = load_checker()
        self.assertIsNotNone(checker, "missing Linux wheel compatibility checker")
        assert checker is not None
        return checker

    def test_policy_pins_the_auditor_and_exact_manylinux_targets(self) -> None:
        self.assertTrue(POLICY_PATH.is_file(), "missing Linux wheel target policy")
        if not POLICY_PATH.is_file():
            return

        policy = tomllib.loads(POLICY_PATH.read_text(encoding="utf-8"))
        self.assertEqual(policy.get("schema_version"), 1)
        self.assertEqual(policy.get("auditor"), EXPECTED_AUDITOR)
        targets = {
            target["target"]: {
                key: value for key, value in target.items() if key != "target"
            }
            for target in policy.get("targets", [])
        }
        self.assertEqual(targets, EXPECTED_TARGETS)

    def test_release_matrix_audits_both_linux_wheels_before_integrity(self) -> None:
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        ci_workflow = CI_WORKFLOW_PATH.read_text(encoding="utf-8")

        self.assertEqual(workflow.count('manylinux: "2014"'), 2)
        for phrase in (
            "auditwheel==6.8.1",
            "packaging==26.3",
            "pyelftools==0.33",
            "python scripts/check_linux_wheel.py",
            '--target "${{ matrix.target }}"',
            "python-wheels-linux-${{ matrix.target }}.auditwheel.json",
        ):
            with self.subTest(workflow_phrase=phrase):
                self.assertIn(phrase, workflow)

        self.assertLess(
            workflow.index("Audit Linux wheel compatibility"),
            workflow.index("Generate the wheel SPDX document"),
        )
        for path in (
            "python-wheel-targets.toml",
            "scripts/check_linux_wheel.py",
            "compat/tests/test_linux_wheel_compatibility.py",
        ):
            with self.subTest(ci_path=path):
                self.assertIn(path, ci_workflow)

    def test_matching_auditwheel_report_produces_digest_bound_evidence(self) -> None:
        checker = self.require_checker()
        policy = checker.load_policy(POLICY_PATH)

        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            for target, expected in EXPECTED_TARGETS.items():
                wheel_name = (
                    "pdfplumber_rs-0.3.0-"
                    f"{expected['python_tag']}-{expected['abi_tag']}-"
                    f"{expected['filename_platform_tag']}.whl"
                )
                wheel_path = root / wheel_name
                wheel_bytes = f"realistic {target} wheel bytes\n".encode()
                wheel_path.write_bytes(wheel_bytes)
                report = sample_report(target, wheel_name)

                evidence = checker.build_evidence(
                    wheel_path,
                    POLICY_PATH,
                    policy[target],
                    report,
                )

                with self.subTest(target=target):
                    self.assertEqual(evidence["outcome"], "compatible")
                    self.assertEqual(evidence["target"], target)
                    self.assertEqual(evidence["wheel"], wheel_name)
                    self.assertEqual(
                        evidence["wheel_sha256"], hashlib.sha256(wheel_bytes).hexdigest()
                    )
                    self.assertEqual(evidence["auditwheel"], report)

    def test_checker_rejects_incompatible_tags_libraries_and_instruction_sets(
        self,
    ) -> None:
        checker = self.require_checker()
        policy = checker.load_policy(POLICY_PATH)
        target_policy = policy["x86_64"]
        wheel_name = (
            "pdfplumber_rs-0.3.0-cp313-cp313-"
            "manylinux_2_17_x86_64.manylinux2014_x86_64.whl"
        )
        mutations = (
            ("overall_tag", "manylinux_2_28_x86_64", "overall tag"),
            ("sym_tag", "manylinux_2_28_x86_64", "symbol tag"),
            ("external_libs", {"libfoo.so.1": {}}, "external shared libraries"),
            ("unsupported_isa", True, "instruction-set"),
            ("pure", True, "native wheel"),
        )

        for field, value, message in mutations:
            report = sample_report("x86_64", wheel_name)
            report[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(
                checker.LinuxWheelError, message
            ):
                checker.validate_auditwheel_report(
                    report,
                    target_policy,
                    wheel_name,
                )

    def test_checker_rejects_auditor_dependency_version_drift(self) -> None:
        checker = self.require_checker()
        auditor_policy = tomllib.loads(POLICY_PATH.read_text(encoding="utf-8"))[
            "auditor"
        ]
        installed_versions = {
            "auditwheel": "6.8.1",
            "packaging": "0.0.0",
            "pyelftools": "0.33",
        }

        with mock.patch.object(
            checker.importlib.metadata,
            "version",
            side_effect=installed_versions.__getitem__,
        ), self.assertRaisesRegex(
            checker.LinuxWheelError,
            "packaging version '0.0.0' does not equal '26.3'",
        ):
            checker.verify_auditor_environment(auditor_policy)

    def test_cli_retains_the_verified_auditwheel_report(self) -> None:
        checker = self.require_checker()
        wheel_name = (
            "pdfplumber_rs-0.3.0-cp313-cp313-"
            "manylinux_2_17_x86_64.manylinux2014_x86_64.whl"
        )
        report = sample_report("x86_64", wheel_name)

        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            wheel_directory = root / "subjects"
            wheel_directory.mkdir()
            (wheel_directory / wheel_name).write_bytes(b"real wheel archive bytes\n")
            output_path = root / "integrity" / "auditwheel.json"

            with mock.patch.object(checker, "run_auditwheel", return_value=report):
                result = checker.main(
                    [
                        "--policy",
                        str(POLICY_PATH),
                        "--target",
                        "x86_64",
                        "--wheel-dir",
                        str(wheel_directory),
                        "--output",
                        str(output_path),
                    ]
                )

            self.assertEqual(result, 0)
            retained = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(retained["auditwheel"], report)
            self.assertEqual(retained["outcome"], "compatible")

    def test_guidance_distinguishes_manylinux_policy_from_installation_proof(
        self,
    ) -> None:
        for path in (GUIDE_PATH, REFERENCE_PATH):
            with self.subTest(path=path.relative_to(REPO_ROOT)):
                self.assertTrue(path.is_file(), f"missing {path.relative_to(REPO_ROOT)}")

        if GUIDE_PATH.is_file():
            guide = GUIDE_PATH.read_text(encoding="utf-8")
            for phrase in (
                "manylinux_2_17",
                "manylinux2014",
                "external_libs",
                "unsupported_isa",
                "does not prove installation",
            ):
                with self.subTest(guide_phrase=phrase):
                    self.assertIn(phrase, guide)

        index = REFERENCE_INDEX_PATH.read_text(encoding="utf-8")
        self.assertIn("python-linux-wheels.md", index)
        prd = (REPO_ROOT / "PRD.md").read_text(encoding="utf-8")
        self.assertRegex(prd, r"(?m)^- \[[ x]\] \*\*DIST-010\*\*")


if __name__ == "__main__":
    unittest.main()
