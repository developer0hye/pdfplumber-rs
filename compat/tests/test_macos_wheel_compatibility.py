"""Contracts for native macOS wheel deployment and installation (DIST-011)."""

from __future__ import annotations

import hashlib
import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path
from types import ModuleType

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = REPO_ROOT / "python-macos-wheel-targets.toml"
CHECKER_PATH = REPO_ROOT / "scripts" / "check_macos_wheel.py"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release-artifacts.yml"
CI_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "cli-binaries-ci.yml"
GUIDE_PATH = REPO_ROOT / "docs" / "macos-wheels.md"
REFERENCE_PATH = REPO_ROOT / "references" / "python-macos-wheels.md"
REFERENCE_INDEX_PATH = REPO_ROOT / "references" / "INDEX.md"
SUPPORT_PATH = REPO_ROOT / "support-matrix.toml"

EXPECTED_TARGETS = {
    "x86_64": {
        "runner": "macos-15-intel",
        "runner_arch": "X64",
        "machine": "x86_64",
        "python_tag": "cp313",
        "abi_tag": "cp313",
        "platform_tag": "macosx_10_12_x86_64",
        "deployment_target": "10.12",
        "macho_arch": "x86_64",
    },
    "aarch64": {
        "runner": "macos-15",
        "runner_arch": "ARM64",
        "machine": "arm64",
        "python_tag": "cp313",
        "abi_tag": "cp313",
        "platform_tag": "macosx_11_0_arm64",
        "deployment_target": "11.0",
        "macho_arch": "arm64",
    },
}


def load_checker() -> ModuleType | None:
    if not CHECKER_PATH.is_file():
        return None
    spec = importlib.util.spec_from_file_location("check_macos_wheel", CHECKER_PATH)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class MacosWheelCompatibilityTests(unittest.TestCase):
    def require_checker(self) -> ModuleType:
        checker = load_checker()
        self.assertIsNotNone(checker, "missing macOS wheel compatibility checker")
        assert checker is not None
        return checker

    def test_policy_declares_exact_native_runners_tags_and_deployment_floors(
        self,
    ) -> None:
        self.assertTrue(POLICY_PATH.is_file(), "missing macOS wheel target policy")
        if not POLICY_PATH.is_file():
            return

        policy = tomllib.loads(POLICY_PATH.read_text(encoding="utf-8"))
        self.assertEqual(policy.get("schema_version"), 1)
        targets = {
            target["target"]: {
                key: value for key, value in target.items() if key != "target"
            }
            for target in policy.get("targets", [])
        }
        self.assertEqual(targets, EXPECTED_TARGETS)

    def test_release_matrix_builds_installs_and_inspects_on_both_native_hosts(
        self,
    ) -> None:
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        ci_workflow = CI_WORKFLOW_PATH.read_text(encoding="utf-8")

        for phrase in (
            "runner: macos-15-intel",
            "runner: macos-15",
            'deployment_target: "10.12"',
            'deployment_target: "11.0"',
            'maturin-version: "1.14.1"',
            'MACOSX_DEPLOYMENT_TARGET=${{ matrix.deployment_target }}',
            "python -m venv dist/macos-wheel-venv",
            "pip install --disable-pip-version-check --no-deps dist/subjects/*.whl",
            "python scripts/check_macos_wheel.py",
            "--installed-python dist/macos-wheel-venv/bin/python",
            "--fixture tests/fixtures/generated/basic_text.pdf",
            "--expected tests/fixtures/expected/cli-release-basic-text.jsonl",
            "python-wheels-macos-${{ matrix.target }}.macho.json",
        ):
            with self.subTest(workflow_phrase=phrase):
                self.assertIn(phrase, workflow)

        self.assertLess(
            workflow.index("Install the macOS wheel on its native architecture"),
            workflow.index("Verify macOS wheel deployment and installed behavior"),
        )
        self.assertLess(
            workflow.index("Verify macOS wheel deployment and installed behavior"),
            workflow.index("Generate the wheel SPDX document"),
        )
        for path in (
            "python-macos-wheel-targets.toml",
            "scripts/check_macos_wheel.py",
            "compat/tests/test_macos_wheel_compatibility.py",
        ):
            with self.subTest(ci_path=path):
                self.assertIn(path, ci_workflow)

    def test_checker_parses_both_macho_deployment_command_forms(self) -> None:
        checker = self.require_checker()
        legacy = """
Load command 8
      cmd LC_VERSION_MIN_MACOSX
  cmdsize 16
  version 10.12
      sdk 26.5
"""
        modern = """
Load command 8
      cmd LC_BUILD_VERSION
  cmdsize 32
 platform 1
    minos 11.0
      sdk 26.5
   ntools 1
"""
        self.assertEqual(checker.parse_deployment_target(legacy), "10.12")
        self.assertEqual(checker.parse_deployment_target(modern), "11.0")

        for output in ("", legacy + modern):
            with self.subTest(output=output), self.assertRaises(
                checker.MacosWheelError
            ):
                checker.parse_deployment_target(output)

    def test_checker_rejects_host_filename_architecture_and_floor_drift(self) -> None:
        checker = self.require_checker()
        policies = checker.load_policy(POLICY_PATH)
        target = policies["x86_64"]
        wheel_name = "pdfplumber_rs-0.3.0-cp313-cp313-macosx_10_12_x86_64.whl"

        checker.validate_host(target, "Darwin", "x86_64", "X64")
        checker.validate_wheel_name(wheel_name, target)
        checker.validate_macho(
            {"architectures": ["x86_64"], "deployment_target": "10.12"},
            target,
        )

        failures = (
            (checker.validate_host, (target, "Darwin", "arm64", "ARM64")),
            (
                checker.validate_wheel_name,
                (
                    "pdfplumber_rs-0.3.0-cp313-cp313-macosx_11_0_arm64.whl",
                    target,
                ),
            ),
            (
                checker.validate_macho,
                (
                    {"architectures": ["arm64"], "deployment_target": "10.12"},
                    target,
                ),
            ),
            (
                checker.validate_macho,
                (
                    {"architectures": ["x86_64"], "deployment_target": "13.0"},
                    target,
                ),
            ),
        )
        for function, arguments in failures:
            with self.subTest(function=function.__name__), self.assertRaises(
                checker.MacosWheelError
            ):
                function(*arguments)

    def test_evidence_binds_wheel_policy_macho_and_installed_probe(self) -> None:
        checker = self.require_checker()
        target = checker.load_policy(POLICY_PATH)["aarch64"]
        wheel_name = "pdfplumber_rs-0.3.0-cp313-cp313-macosx_11_0_arm64.whl"
        wheel_bytes = b"representative macOS wheel bytes\n"
        inspection = {
            "native_module": "pdfplumber/_native.cpython-313-darwin.so",
            "native_module_sha256": "a" * 64,
            "architectures": ["arm64"],
            "deployment_target": "11.0",
        }
        probe = {
            "distribution_version": "0.3.0",
            "machine": "arm64",
            "page_count": 1,
            "text_sha256": "b" * 64,
        }

        with tempfile.TemporaryDirectory() as temporary_directory:
            wheel_path = Path(temporary_directory) / wheel_name
            wheel_path.write_bytes(wheel_bytes)
            evidence = checker.build_evidence(
                wheel_path,
                POLICY_PATH,
                target,
                inspection,
                probe,
            )

        self.assertEqual(evidence["outcome"], "compatible")
        self.assertEqual(evidence["wheel"], wheel_name)
        self.assertEqual(
            evidence["wheel_sha256"], hashlib.sha256(wheel_bytes).hexdigest()
        )
        self.assertEqual(evidence["macho"], inspection)
        self.assertEqual(evidence["installed_probe"], probe)

    def test_guidance_and_support_record_proof_and_platform_boundaries(self) -> None:
        for path in (GUIDE_PATH, REFERENCE_PATH):
            with self.subTest(path=path.relative_to(REPO_ROOT)):
                self.assertTrue(path.is_file(), f"missing {path.relative_to(REPO_ROOT)}")

        if GUIDE_PATH.is_file():
            guide = GUIDE_PATH.read_text(encoding="utf-8")
            for phrase in (
                "macosx_10_12_x86_64",
                "macosx_11_0_arm64",
                "LC_VERSION_MIN_MACOSX",
                "LC_BUILD_VERSION",
                "native Intel",
                "Apple Silicon",
                "does not prove execution on the minimum operating-system release",
            ):
                with self.subTest(guide_phrase=phrase):
                    self.assertIn(phrase, guide)

        support = tomllib.loads(SUPPORT_PATH.read_text(encoding="utf-8"))
        python = next(
            surface for surface in support["surfaces"] if surface["id"] == "python"
        )
        verified = "\n".join(python["ci_verified_platforms"])
        self.assertIn("macOS 15 Intel", verified)
        self.assertIn("macOS 15 Apple Silicon", verified)
        evidence = set(python["evidence"])
        for path in (
            "docs/macos-wheels.md",
            "compat/tests/test_macos_wheel_compatibility.py",
            "python-macos-wheel-targets.toml",
            "scripts/check_macos_wheel.py",
        ):
            with self.subTest(evidence_path=path):
                self.assertIn(path, evidence)

        index = REFERENCE_INDEX_PATH.read_text(encoding="utf-8")
        self.assertIn("python-macos-wheels.md", index)
        prd = (REPO_ROOT / "PRD.md").read_text(encoding="utf-8")
        self.assertRegex(prd, r"(?m)^- \[ \] \*\*DIST-011\*\*")


if __name__ == "__main__":
    unittest.main()
