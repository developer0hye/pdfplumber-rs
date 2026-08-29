"""Contracts for native, versioned Command-Line Interface release archives."""

from __future__ import annotations

import importlib.util
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path
from types import ModuleType

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
TARGETS_PATH = REPO_ROOT / "cli-release-targets.toml"
PACKAGER_PATH = REPO_ROOT / "scripts" / "build_cli_release.py"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "cli-binaries.yml"
CI_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "cli-binaries-ci.yml"
RELEASE_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
GUIDE_PATH = REPO_ROOT / "docs" / "cli-binaries.md"
REFERENCE_PATH = REPO_ROOT / "references" / "rust-cli-binaries.md"


EXPECTED_TARGETS = {
    "x86_64-unknown-linux-gnu": {
        "runner": "ubuntu-22.04",
        "runner_os": "Linux",
        "runner_arch": "X64",
        "archive_format": "tar.gz",
        "rust_tier": 1,
    },
    "aarch64-unknown-linux-gnu": {
        "runner": "ubuntu-22.04-arm",
        "runner_os": "Linux",
        "runner_arch": "ARM64",
        "archive_format": "tar.gz",
        "rust_tier": 1,
    },
    "x86_64-apple-darwin": {
        "runner": "macos-15-intel",
        "runner_os": "macOS",
        "runner_arch": "X64",
        "archive_format": "tar.gz",
        "rust_tier": 2,
    },
    "aarch64-apple-darwin": {
        "runner": "macos-15",
        "runner_os": "macOS",
        "runner_arch": "ARM64",
        "archive_format": "tar.gz",
        "rust_tier": 1,
    },
    "x86_64-pc-windows-msvc": {
        "runner": "windows-2025",
        "runner_os": "Windows",
        "runner_arch": "X64",
        "archive_format": "zip",
        "rust_tier": 1,
    },
}


def load_packager() -> ModuleType | None:
    if not PACKAGER_PATH.is_file():
        return None
    spec = importlib.util.spec_from_file_location("build_cli_release", PACKAGER_PATH)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def executable_bytes(target: str) -> bytes:
    binary = bytearray(256)
    if target.endswith("linux-gnu"):
        binary[:4] = b"\x7fELF"
        binary[4] = 2
        binary[5] = 1
        machine = 62 if target.startswith("x86_64") else 183
        binary[18:20] = struct.pack("<H", machine)
    elif target.endswith("apple-darwin"):
        binary[:4] = b"\xcf\xfa\xed\xfe"
        cpu_type = 0x01000007 if target.startswith("x86_64") else 0x0100000C
        binary[4:8] = struct.pack("<I", cpu_type)
    elif target.endswith("windows-msvc"):
        binary[:2] = b"MZ"
        binary[0x3C:0x40] = struct.pack("<I", 0x80)
        binary[0x80:0x84] = b"PE\0\0"
        binary[0x84:0x86] = struct.pack("<H", 0x8664)
    else:
        raise AssertionError(f"no fixture format for {target}")
    return bytes(binary)


class CliReleaseBinaryTests(unittest.TestCase):
    def require_packager(self) -> ModuleType:
        packager = load_packager()
        self.assertIsNotNone(packager, "missing CLI release archive builder")
        assert packager is not None
        return packager

    def test_target_policy_declares_exact_required_native_matrix(self) -> None:
        self.assertTrue(TARGETS_PATH.is_file(), "missing CLI target policy")
        policy = tomllib.loads(TARGETS_PATH.read_text(encoding="utf-8"))
        self.assertEqual(policy["schema_version"], 1)
        actual = {target.pop("triple"): target for target in policy["targets"]}
        self.assertEqual(actual, EXPECTED_TARGETS)

    def test_matrix_export_is_complete_and_contains_no_optional_platform(self) -> None:
        packager = self.require_packager()
        matrix = packager.github_matrix(TARGETS_PATH)

        self.assertEqual(
            {entry["target"] for entry in matrix["include"]},
            set(EXPECTED_TARGETS),
        )
        self.assertEqual(len(matrix["include"]), 5)
        for entry in matrix["include"]:
            expected = EXPECTED_TARGETS[entry["target"]]
            with self.subTest(target=entry["target"]):
                self.assertEqual(entry["runner"], expected["runner"])
                self.assertEqual(entry["runner_os"], expected["runner_os"])
                self.assertEqual(entry["runner_arch"], expected["runner_arch"])

    def test_linux_archive_is_versioned_and_contains_only_public_files(self) -> None:
        packager = self.require_packager()
        target = "x86_64-unknown-linux-gnu"
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            binary = root / "pdfplumber"
            binary.write_bytes(executable_bytes(target))
            license_path = root / "LICENSE"
            license_path.write_text("Apache-2.0\n", encoding="utf-8")
            readme_path = root / "README.md"
            readme_path.write_text("# CLI\n", encoding="utf-8")

            archive = packager.package_release(
                binary_path=binary,
                target=target,
                runner_os="Linux",
                runner_arch="X64",
                rustc_host=target,
                version="1.2.3",
                output_dir=root / "dist",
                license_path=license_path,
                readme_path=readme_path,
            )

            self.assertEqual(
                archive.name,
                "pdfplumber-cli-1.2.3-x86_64-unknown-linux-gnu.tar.gz",
            )
            archive_root = archive.name.removesuffix(".tar.gz")
            with tarfile.open(archive, "r:gz") as package:
                self.assertEqual(
                    package.getnames(),
                    [
                        f"{archive_root}/pdfplumber",
                        f"{archive_root}/README.md",
                        f"{archive_root}/LICENSE",
                    ],
                )
                executable = package.getmember(f"{archive_root}/pdfplumber")
                self.assertEqual(executable.mode, 0o755)
                self.assertTrue(executable.isfile())
                extracted = package.extractfile(executable)
                self.assertIsNotNone(extracted)
                assert extracted is not None
                self.assertEqual(extracted.read(), executable_bytes(target))

    def test_windows_archive_uses_exe_and_zip_permissions(self) -> None:
        packager = self.require_packager()
        target = "x86_64-pc-windows-msvc"
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            binary = root / "pdfplumber.exe"
            binary.write_bytes(executable_bytes(target))
            license_path = root / "LICENSE"
            license_path.write_text("Apache-2.0\n", encoding="utf-8")
            readme_path = root / "README.md"
            readme_path.write_text("# CLI\n", encoding="utf-8")

            archive = packager.package_release(
                binary_path=binary,
                target=target,
                runner_os="Windows",
                runner_arch="X64",
                rustc_host=target,
                version="1.2.3",
                output_dir=root / "dist",
                license_path=license_path,
                readme_path=readme_path,
            )

            self.assertEqual(
                archive.name,
                "pdfplumber-cli-1.2.3-x86_64-pc-windows-msvc.zip",
            )
            archive_root = archive.name.removesuffix(".zip")
            with zipfile.ZipFile(archive) as package:
                self.assertEqual(
                    package.namelist(),
                    [
                        f"{archive_root}/pdfplumber.exe",
                        f"{archive_root}/README.md",
                        f"{archive_root}/LICENSE",
                    ],
                )
                executable = package.getinfo(f"{archive_root}/pdfplumber.exe")
                self.assertEqual(executable.external_attr >> 16, 0o100755)
                self.assertEqual(package.read(executable), executable_bytes(target))

    def test_packaging_rejects_unknown_or_misrouted_targets(self) -> None:
        packager = self.require_packager()
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            binary = root / "pdfplumber"
            binary.write_bytes(executable_bytes("x86_64-unknown-linux-gnu"))
            license_path = root / "LICENSE"
            readme_path = root / "README.md"
            license_path.write_text("license", encoding="utf-8")
            readme_path.write_text("readme", encoding="utf-8")
            common = {
                "binary_path": binary,
                "version": "1.2.3",
                "output_dir": root / "dist",
                "license_path": license_path,
                "readme_path": readme_path,
            }

            invalid_cases = (
                {
                    "target": "powerpc64le-unknown-linux-gnu",
                    "runner_os": "Linux",
                    "runner_arch": "X64",
                    "rustc_host": "x86_64-unknown-linux-gnu",
                },
                {
                    "target": "x86_64-unknown-linux-gnu",
                    "runner_os": "Linux",
                    "runner_arch": "ARM64",
                    "rustc_host": "x86_64-unknown-linux-gnu",
                },
                {
                    "target": "x86_64-unknown-linux-gnu",
                    "runner_os": "Linux",
                    "runner_arch": "X64",
                    "rustc_host": "aarch64-unknown-linux-gnu",
                },
            )
            for invalid in invalid_cases:
                with (
                    self.subTest(invalid=invalid),
                    self.assertRaises(packager.CliReleaseError),
                ):
                    packager.package_release(**common, **invalid)

    def test_packaging_rejects_wrong_executable_format_for_every_target(
        self,
    ) -> None:
        packager = self.require_packager()
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            license_path = root / "LICENSE"
            readme_path = root / "README.md"
            license_path.write_text("license", encoding="utf-8")
            readme_path.write_text("readme", encoding="utf-8")

            for target, expected in EXPECTED_TARGETS.items():
                binary_name = (
                    "pdfplumber.exe" if target.endswith("msvc") else "pdfplumber"
                )
                binary = root / binary_name
                binary.write_bytes(b"not a native executable")
                with (
                    self.subTest(target=target),
                    self.assertRaises(packager.CliReleaseError),
                ):
                    packager.package_release(
                        binary_path=binary,
                        target=target,
                        runner_os=expected["runner_os"],
                        runner_arch=expected["runner_arch"],
                        rustc_host=target,
                        version="1.2.3",
                        output_dir=root / "dist",
                        license_path=license_path,
                        readme_path=readme_path,
                    )

    def test_release_tag_must_match_exact_cli_semver(self) -> None:
        packager = self.require_packager()
        self.assertEqual(packager.validate_release_tag("", "1.2.3"), "1.2.3")
        self.assertEqual(packager.validate_release_tag("v1.2.3", "1.2.3"), "1.2.3")
        for invalid_tag in ("1.2.3", "v1.2.4", "v1.2", "v1.2.3-rc.1"):
            with (
                self.subTest(invalid_tag=invalid_tag),
                self.assertRaises(packager.CliReleaseError),
            ):
                packager.validate_release_tag(invalid_tag, "1.2.3")

    def test_binary_lookup_uses_cargo_metadata_target_directory(self) -> None:
        packager = self.require_packager()
        with tempfile.TemporaryDirectory() as temporary_directory:
            target_directory = Path(temporary_directory) / "shared-cargo-target"
            commands: list[tuple[str, ...]] = []

            def runner(
                command: tuple[str, ...], **options: object
            ) -> subprocess.CompletedProcess[str]:
                commands.append(command)
                return subprocess.CompletedProcess(
                    command,
                    0,
                    stdout=f'{{"target_directory":"{target_directory}"}}',
                    stderr="",
                )

            discovered = packager.cargo_target_directory(runner=runner)

            self.assertEqual(
                commands,
                [("cargo", "metadata", "--no-deps", "--format-version", "1")],
            )
            self.assertEqual(discovered, target_directory)
            self.assertEqual(
                packager.default_binary_path(
                    "aarch64-apple-darwin", target_directory=discovered
                ),
                target_directory / "aarch64-apple-darwin" / "release" / "pdfplumber",
            )

    def test_workflows_build_locked_native_targets_and_gate_the_release(
        self,
    ) -> None:
        for path in (WORKFLOW_PATH, CI_WORKFLOW_PATH, RELEASE_PATH):
            with self.subTest(path=path.name):
                self.assertTrue(
                    path.is_file(), f"missing {path.relative_to(REPO_ROOT)}"
                )

        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        for phrase in (
            "workflow_call:",
            "python scripts/build_cli_release.py matrix",
            "runs-on: ${{ matrix.runner }}",
            "fail-fast: false",
            "targets: ${{ matrix.target }}",
            "cargo build --locked --release --package pdfplumber-cli --target",
            "python scripts/build_cli_release.py package",
            '--runner-os "${{ runner.os }}"',
            '--runner-arch "${{ runner.arch }}"',
            "actions/upload-artifact@v4",
            "name: cli-binary-${{ matrix.target }}",
            "if-no-files-found: error",
        ):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, workflow)

        ci_workflow = CI_WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertIn("pull_request:", ci_workflow)
        self.assertIn("branches: [main]", ci_workflow)
        self.assertIn("uses: ./.github/workflows/cli-binaries.yml", ci_workflow)

        release = RELEASE_PATH.read_text(encoding="utf-8")
        self.assertIn("uses: ./.github/workflows/cli-binaries.yml", release)
        self.assertIn("release_tag: ${{ github.ref_name }}", release)
        self.assertIn("needs: [publish, metadata, scorecards, cli-binaries]", release)
        self.assertIn("pattern: cli-binary-*", release)
        self.assertIn("path: release-cli-binaries", release)
        self.assertIn("release-cli-binaries/*", release)

    def test_public_guide_states_platform_and_verification_boundaries(self) -> None:
        self.assertTrue(GUIDE_PATH.is_file(), "missing prebuilt CLI guide")
        guide = GUIDE_PATH.read_text(encoding="utf-8")
        for target in EXPECTED_TARGETS:
            with self.subTest(target=target):
                self.assertIn(target, guide)
        for phrase in (
            "GitHub Release",
            "pdfplumber-cli-<version>-<target>",
            "Linux GNU",
            "macOS 10.12",
            "macOS 11",
            "Windows 10",
            "native runner",
            "executable format",
            "DIST-004",
            "not runtime-smoke-tested",
            "DIST-005",
        ):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, guide)

        cli_readme = (REPO_ROOT / "crates" / "pdfplumber-cli" / "README.md").read_text(
            encoding="utf-8"
        )
        root_readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn("docs/cli-binaries.md", root_readme)
        self.assertIn("../../docs/cli-binaries.md", cli_readme)

    def test_support_source_and_reference_index_record_the_new_boundary(self) -> None:
        support = tomllib.loads(
            (REPO_ROOT / "support-matrix.toml").read_text(encoding="utf-8")
        )
        cli = next(surface for surface in support["surfaces"] if surface["id"] == "cli")
        configured = "\n".join(cli["release_configured_targets"])
        verified = "\n".join(cli["ci_verified_platforms"])
        limitations = "\n".join(cli["known_limitations"])
        evidence = set(cli["evidence"])

        for target in EXPECTED_TARGETS:
            with self.subTest(target=target):
                self.assertIn(target, configured)
                self.assertNotIn(target, verified)
        self.assertIn("DIST-004", limitations)
        self.assertIn("not runtime-smoke-tested", limitations)
        self.assertIn("cli-release-targets.toml", evidence)
        self.assertIn("scripts/build_cli_release.py", evidence)
        self.assertIn("docs/cli-binaries.md", evidence)

        self.assertTrue(REFERENCE_PATH.is_file(), "missing CLI binary reference note")
        reference = REFERENCE_PATH.read_text(encoding="utf-8")
        self.assertLessEqual(len(reference.splitlines()), 50)
        self.assertIn("Typst", reference)
        self.assertIn("GitHub-hosted runners", reference)
        index = (REPO_ROOT / "references" / "INDEX.md").read_text(encoding="utf-8")
        self.assertIn("rust-cli-binaries.md", index)


if __name__ == "__main__":
    unittest.main()
