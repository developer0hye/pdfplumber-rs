"""Contracts for native Windows wheel dependencies and path behavior (DIST-012)."""

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
POLICY_PATH = REPO_ROOT / "python-windows-wheel-targets.toml"
ATTRIBUTES_PATH = REPO_ROOT / ".gitattributes"
CHECKER_PATH = REPO_ROOT / "scripts" / "check_windows_wheel.py"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release-artifacts.yml"
CI_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "cli-binaries-ci.yml"
GUIDE_PATH = REPO_ROOT / "docs" / "windows-wheels.md"
REFERENCE_PATH = REPO_ROOT / "references" / "python-windows-wheels.md"
REFERENCE_INDEX_PATH = REPO_ROOT / "references" / "INDEX.md"
SUPPORT_PATH = REPO_ROOT / "support-matrix.toml"

EXPECTED_DEPENDENCIES = [
    "api-ms-win-core-synch-l1-2-0.dll",
    "api-ms-win-crt-heap-l1-1-0.dll",
    "api-ms-win-crt-math-l1-1-0.dll",
    "api-ms-win-crt-runtime-l1-1-0.dll",
    "bcryptprimitives.dll",
    "kernel32.dll",
    "ntdll.dll",
    "python313.dll",
    "vcruntime140.dll",
]
EXPECTED_TARGET = {
    "runner": "windows-2025",
    "runner_arch": "X64",
    "machine": "AMD64",
    "python_tag": "cp313",
    "abi_tag": "cp313",
    "platform_tag": "win_amd64",
    "pe_machine": "8664",
    "pe_format": "PE32+",
    "long_paths_enabled": 1,
    "minimum_long_path_characters": 280,
    "required_dependencies": EXPECTED_DEPENDENCIES,
}


def load_checker() -> ModuleType | None:
    if not CHECKER_PATH.is_file():
        return None
    spec = importlib.util.spec_from_file_location("check_windows_wheel", CHECKER_PATH)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def sample_probe(native_module_sha256: str) -> dict[str, object]:
    common = {
        "contains_non_ascii": True,
        "normal_win32_path": True,
        "page_count": 1,
        "text_sha256": "b" * 64,
        "fixture_sha256": "c" * 64,
    }
    return {
        "distribution_version": "0.3.0",
        "python_version": "3.13.12",
        "machine": "AMD64",
        "package_file": "Lib/site-packages/pdfplumber/__init__.py",
        "native_module_file": (
            "Lib/site-packages/pdfplumber/_native.cp313-win_amd64.pyd"
        ),
        "native_module_sha256": native_module_sha256,
        "expected_sha256": "d" * 64,
        "path_probes": [
            {"case": "unicode", "path_characters": 126, **common},
            {"case": "long_unicode", "path_characters": 304, **common},
        ],
    }


class WindowsWheelCompatibilityTests(unittest.TestCase):
    def require_checker(self) -> ModuleType:
        checker = load_checker()
        self.assertIsNotNone(checker, "missing Windows wheel compatibility checker")
        assert checker is not None
        return checker

    def test_policy_declares_exact_native_runner_pe_and_dependency_contract(
        self,
    ) -> None:
        self.assertTrue(POLICY_PATH.is_file(), "missing Windows wheel target policy")
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
        self.assertEqual(targets, {"x86_64": EXPECTED_TARGET})

    def test_policy_keeps_lf_bytes_on_windows_for_source_digest_binding(self) -> None:
        attributes = ATTRIBUTES_PATH.read_text(encoding="utf-8").splitlines()
        self.assertIn(
            "python-windows-wheel-targets.toml text eol=lf",
            attributes,
        )

    def test_release_matrix_installs_inspects_and_runs_paths_on_native_windows(
        self,
    ) -> None:
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        ci_workflow = CI_WORKFLOW_PATH.read_text(encoding="utf-8")

        for phrase in (
            "runner: windows-2025",
            "Locate DUMPBIN for Windows wheel inspection",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "WINDOWS_DUMPBIN",
            "Install the Windows wheel on its native architecture",
            "dist/windows-wheel-venv/Scripts/python.exe",
            "Verify Windows wheel dependencies and installed path behavior",
            "python scripts/check_windows_wheel.py",
            "--policy python-windows-wheel-targets.toml",
            '--dumpbin "$env:WINDOWS_DUMPBIN"',
            "--installed-python dist/windows-wheel-venv/Scripts/python.exe",
            "--fixture tests/fixtures/generated/basic_text.pdf",
            "--expected tests/fixtures/expected/cli-release-basic-text.jsonl",
            "python-wheels-windows-${{ matrix.target }}.pe.json",
        ):
            with self.subTest(workflow_phrase=phrase):
                self.assertIn(phrase, workflow)

        self.assertLess(
            workflow.index("Install the Windows wheel on its native architecture"),
            workflow.index(
                "Verify Windows wheel dependencies and installed path behavior"
            ),
        )
        self.assertLess(
            workflow.index(
                "Verify Windows wheel dependencies and installed path behavior"
            ),
            workflow.index("Generate the wheel SPDX document"),
        )
        for path in (
            "python-windows-wheel-targets.toml",
            "scripts/check_windows_wheel.py",
            "compat/tests/test_windows_wheel_compatibility.py",
        ):
            with self.subTest(ci_path=path):
                self.assertEqual(ci_workflow.count(path), 2)

    def test_checker_parses_exact_pe_format_machine_and_dependencies(self) -> None:
        checker = self.require_checker()
        dumpbin = """
FILE HEADER VALUES
            8664 machine (x64)
OPTIONAL HEADER VALUES
             20B magic # (PE32+)
  Image has the following dependencies:

    KERNEL32.dll
    python313.dll
    VCRUNTIME140.dll
    bcryptprimitives.dll
    ntdll.dll
    api-ms-win-core-synch-l1-2-0.dll
    api-ms-win-crt-math-l1-1-0.dll
    api-ms-win-crt-runtime-l1-1-0.dll
    api-ms-win-crt-heap-l1-1-0.dll
"""
        self.assertEqual(
            checker.parse_dumpbin(dumpbin),
            {
                "pe_machine": "8664",
                "pe_format": "PE32+",
                "dependencies": EXPECTED_DEPENDENCIES,
            },
        )
        for output in ("", dumpbin.replace("8664 machine (x64)", "14C machine (x86)")):
            with self.subTest(output=output), self.assertRaises(
                checker.WindowsWheelError
            ):
                checker.parse_dumpbin(output)

    def test_checker_rejects_host_tag_pe_dependency_and_long_path_drift(self) -> None:
        checker = self.require_checker()
        target = checker.load_policy(POLICY_PATH)["x86_64"]
        wheel_name = "pdfplumber_rs-0.3.0-cp313-cp313-win_amd64.whl"
        inspection = {
            "native_module": "pdfplumber/_native.cp313-win_amd64.pyd",
            "native_module_sha256": "a" * 64,
            "pe_machine": "8664",
            "pe_format": "PE32+",
            "dependencies": EXPECTED_DEPENDENCIES,
        }

        checker.validate_host(target, "Windows", "AMD64", "X64")
        checker.validate_wheel_name(wheel_name, target)
        checker.validate_pe(inspection, target)
        checker.validate_long_paths_enabled(1, target)
        checker.validate_installed_probe(
            sample_probe(inspection["native_module_sha256"]),
            target,
            inspection["native_module_sha256"],
        )

        failures = (
            (checker.validate_host, (target, "Windows", "ARM64", "ARM64")),
            (
                checker.validate_wheel_name,
                ("pdfplumber_rs-0.3.0-cp313-cp313-win32.whl", target),
            ),
            (
                checker.validate_pe,
                ({**inspection, "pe_machine": "14C"}, target),
            ),
            (
                checker.validate_pe,
                ({**inspection, "dependencies": ["kernel32.dll"]}, target),
            ),
            (checker.validate_long_paths_enabled, (0, target)),
        )
        for function, arguments in failures:
            with self.subTest(function=function.__name__), self.assertRaises(
                checker.WindowsWheelError
            ):
                function(*arguments)

        short_probe = sample_probe(inspection["native_module_sha256"])
        short_probe["path_probes"][1]["path_characters"] = 260
        with self.assertRaises(checker.WindowsWheelError):
            checker.validate_installed_probe(
                short_probe,
                target,
                inspection["native_module_sha256"],
            )

    def test_evidence_binds_wheel_policy_pe_registry_and_installed_probe(
        self,
    ) -> None:
        checker = self.require_checker()
        target = checker.load_policy(POLICY_PATH)["x86_64"]
        wheel_name = "pdfplumber_rs-0.3.0-cp313-cp313-win_amd64.whl"
        wheel_bytes = b"representative Windows wheel bytes\n"
        inspection = {
            "native_module": "pdfplumber/_native.cp313-win_amd64.pyd",
            "native_module_sha256": "a" * 64,
            "pe_machine": "8664",
            "pe_format": "PE32+",
            "dependencies": EXPECTED_DEPENDENCIES,
        }
        probe = sample_probe(inspection["native_module_sha256"])

        with tempfile.TemporaryDirectory() as temporary_directory:
            wheel_path = Path(temporary_directory) / wheel_name
            wheel_path.write_bytes(wheel_bytes)
            evidence = checker.build_evidence(
                wheel_path,
                POLICY_PATH,
                target,
                inspection,
                1,
                probe,
            )

        self.assertEqual(evidence["outcome"], "compatible")
        self.assertEqual(evidence["wheel"], wheel_name)
        self.assertEqual(
            evidence["wheel_sha256"], hashlib.sha256(wheel_bytes).hexdigest()
        )
        self.assertEqual(evidence["pe"], inspection)
        self.assertEqual(evidence["long_paths_enabled"], 1)
        self.assertEqual(evidence["installed_probe"], probe)

    def test_guidance_and_support_record_proof_and_platform_boundaries(self) -> None:
        for path in (GUIDE_PATH, REFERENCE_PATH):
            with self.subTest(path=path.relative_to(REPO_ROOT)):
                self.assertTrue(
                    path.is_file(), f"missing {path.relative_to(REPO_ROOT)}"
                )

        if GUIDE_PATH.is_file():
            guide = GUIDE_PATH.read_text(encoding="utf-8")
            for phrase in (
                "windows-2025",
                "DUMPBIN /DEPENDENTS",
                "PE32+",
                "LongPathsEnabled=1",
                "more than 260 characters",
                "normal Win32 path",
                "does not prove behavior when the Windows long-path policy is disabled",
            ):
                with self.subTest(guide_phrase=phrase):
                    self.assertIn(phrase, guide)

        support = tomllib.loads(SUPPORT_PATH.read_text(encoding="utf-8"))
        python = next(
            surface for surface in support["surfaces"] if surface["id"] == "python"
        )
        verified = "\n".join(python["ci_verified_platforms"])
        self.assertIn("Windows Server 2025 x86-64", verified)
        evidence = set(python["evidence"])
        for path in (
            "docs/windows-wheels.md",
            "compat/tests/test_windows_wheel_compatibility.py",
            "python-windows-wheel-targets.toml",
            "scripts/check_windows_wheel.py",
        ):
            with self.subTest(evidence_path=path):
                self.assertIn(path, evidence)

        index = REFERENCE_INDEX_PATH.read_text(encoding="utf-8")
        self.assertIn("python-windows-wheels.md", index)
        prd = (REPO_ROOT / "PRD.md").read_text(encoding="utf-8")
        self.assertIn("- [ ] **DIST-012**", prd)
