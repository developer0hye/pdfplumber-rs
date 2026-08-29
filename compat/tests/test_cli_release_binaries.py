"""Contracts for native, versioned Command-Line Interface release archives."""

from __future__ import annotations

import hashlib
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
ATTRIBUTES_PATH = REPO_ROOT / ".gitattributes"
TARGETS_PATH = REPO_ROOT / "cli-release-targets.toml"
SMOKE_POLICY_PATH = REPO_ROOT / "cli-release-smoke.toml"
PACKAGER_PATH = REPO_ROOT / "scripts" / "build_cli_release.py"
SMOKE_EXPECTED_PATH = (
    REPO_ROOT / "tests" / "fixtures" / "expected" / "cli-release-basic-text.jsonl"
)
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "cli-binaries.yml"
CI_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "cli-binaries-ci.yml"
RELEASE_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
GUIDE_PATH = REPO_ROOT / "docs" / "cli-binaries.md"
REFERENCE_PATH = REPO_ROOT / "references" / "rust-cli-binaries.md"

SMOKE_FIXTURE_SHA256 = (
    "22b6f9bd4aa388d7e6fb116d45bbe15e6da84c8d23fe20582d857e4b05c809ec"
)
SMOKE_EXPECTED_STDOUT = (
    '{"page":1,"text":"The quick brown fox jumps over the lazy dog.\\n'
    'Special chars: \\"quotes\\", copyright ©, registered ®, section §, '
    "degree °, plus-minus ±\\nAccented: café, naïve, résumé, über, piñata, "
    "à la carte\\nNumbers: 0 1 2 3 4 5 6 7 8 9. Price: $1,234.56. "
    'Ratio: 3:1. Percent: 99.9%"}\n'
).encode()


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


def sha256(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def write_smoke_policy(root: Path) -> tuple[Path, Path, Path]:
    fixture_path = root / "fixture.pdf"
    fixture_path.write_bytes(b"%PDF-1.7\nrealistic fixture bytes\n")
    expected_path = root / "expected.jsonl"
    expected_path.write_bytes(SMOKE_EXPECTED_STDOUT)
    policy_path = root / "smoke.toml"
    policy_path.write_text(
        "\n".join(
            (
                "schema_version = 1",
                f'fixture = "{fixture_path.name}"',
                f'fixture_sha256 = "{sha256(fixture_path.read_bytes())}"',
                f'expected_stdout = "{expected_path.name}"',
                f'expected_stdout_sha256 = "{sha256(SMOKE_EXPECTED_STDOUT)}"',
                'args = ["text", "{fixture}", "--format", "json"]',
                "timeout_seconds = 30",
                "",
            )
        ),
        encoding="utf-8",
    )
    return policy_path, fixture_path, expected_path


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

    def test_smoke_policy_binds_the_real_fixture_and_exact_stdout(self) -> None:
        self.assertTrue(SMOKE_POLICY_PATH.is_file(), "missing CLI smoke policy")
        self.assertTrue(SMOKE_EXPECTED_PATH.is_file(), "missing exact smoke output")
        policy = tomllib.loads(SMOKE_POLICY_PATH.read_text(encoding="utf-8"))

        self.assertEqual(
            set(policy),
            {
                "schema_version",
                "fixture",
                "fixture_sha256",
                "expected_stdout",
                "expected_stdout_sha256",
                "args",
                "timeout_seconds",
            },
        )
        self.assertEqual(policy["schema_version"], 1)
        self.assertEqual(policy["fixture"], "tests/fixtures/generated/basic_text.pdf")
        self.assertEqual(policy["fixture_sha256"], SMOKE_FIXTURE_SHA256)
        fixture = REPO_ROOT / policy["fixture"]
        self.assertEqual(sha256(fixture.read_bytes()), SMOKE_FIXTURE_SHA256)
        self.assertEqual(
            policy["expected_stdout"],
            "tests/fixtures/expected/cli-release-basic-text.jsonl",
        )
        self.assertEqual(SMOKE_EXPECTED_PATH.read_bytes(), SMOKE_EXPECTED_STDOUT)
        self.assertEqual(
            policy["expected_stdout_sha256"], sha256(SMOKE_EXPECTED_STDOUT)
        )
        self.assertEqual(policy["args"], ["text", "{fixture}", "--format", "json"])
        self.assertEqual(policy["timeout_seconds"], 30)

    def test_exact_smoke_output_checkout_bytes_are_platform_independent(self) -> None:
        attributes = ATTRIBUTES_PATH.read_text(encoding="utf-8").splitlines()
        self.assertIn(
            "tests/fixtures/expected/cli-release-basic-text.jsonl text eol=lf",
            attributes,
        )

    def test_smoke_executes_the_executable_extracted_from_each_archive_format(
        self,
    ) -> None:
        packager = self.require_packager()
        cases = (
            ("aarch64-apple-darwin", "macOS", "ARM64", "pdfplumber"),
            ("x86_64-pc-windows-msvc", "Windows", "X64", "pdfplumber.exe"),
        )
        for target, runner_os, runner_arch, executable_name in cases:
            with (
                self.subTest(target=target),
                tempfile.TemporaryDirectory() as directory,
            ):
                root = Path(directory)
                binary_content = executable_bytes(target)
                binary_path = root / executable_name
                binary_path.write_bytes(binary_content)
                license_path = root / "LICENSE"
                license_path.write_text("Apache-2.0\n", encoding="utf-8")
                readme_path = root / "README.md"
                readme_path.write_text("# CLI\n", encoding="utf-8")
                archive = packager.package_release(
                    binary_path=binary_path,
                    target=target,
                    runner_os=runner_os,
                    runner_arch=runner_arch,
                    rustc_host=target,
                    version="1.2.3",
                    output_dir=root / "dist",
                    license_path=license_path,
                    readme_path=readme_path,
                )
                policy_path, fixture_path, _expected_path = write_smoke_policy(root)
                manifest_path = root / "Cargo.toml"
                manifest_path.write_text(
                    '[package]\nname = "pdfplumber-cli"\nversion = "1.2.3"\n',
                    encoding="utf-8",
                )
                observed: dict[str, object] = {}

                def runner(
                    command: tuple[str, ...],
                    observed_result: dict[str, object] = observed,
                    **options: object,
                ) -> subprocess.CompletedProcess[bytes]:
                    extracted_executable = Path(command[0])
                    observed_result["binary"] = extracted_executable.read_bytes()
                    observed_result["name"] = extracted_executable.name
                    observed_result["args"] = command[1:]
                    observed_result["options"] = options
                    return subprocess.CompletedProcess(
                        command, 0, stdout=SMOKE_EXPECTED_STDOUT, stderr=b""
                    )

                original_runner = packager.subprocess.run
                packager.subprocess.run = runner
                try:
                    result = packager.main(
                        [
                            "smoke",
                            "--target",
                            target,
                            "--archive",
                            str(archive),
                            "--policy",
                            str(policy_path),
                            "--manifest",
                            str(manifest_path),
                        ]
                    )
                finally:
                    packager.subprocess.run = original_runner

                self.assertEqual(result, 0)
                self.assertEqual(observed["binary"], binary_content)
                self.assertEqual(observed["name"], executable_name)
                self.assertEqual(
                    observed["args"],
                    ("text", str(fixture_path.resolve()), "--format", "json"),
                )
                self.assertEqual(
                    observed["options"],
                    {"check": False, "capture_output": True, "timeout": 30},
                )

    def test_smoke_fails_closed_on_input_or_process_drift(self) -> None:
        packager = self.require_packager()
        target = "aarch64-apple-darwin"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "pdfplumber"
            binary.write_bytes(executable_bytes(target))
            license_path = root / "LICENSE"
            license_path.write_text("Apache-2.0\n", encoding="utf-8")
            readme_path = root / "README.md"
            readme_path.write_text("# CLI\n", encoding="utf-8")
            archive = packager.package_release(
                binary_path=binary,
                target=target,
                runner_os="macOS",
                runner_arch="ARM64",
                rustc_host=target,
                version="1.2.3",
                output_dir=root / "dist",
                license_path=license_path,
                readme_path=readme_path,
            )
            policy_path, fixture_path, _expected_path = write_smoke_policy(root)
            policy = packager.load_smoke_policy(policy_path)
            fixture_content = fixture_path.read_bytes()
            fixture_path.write_bytes(fixture_content + b"drift")

            with self.assertRaisesRegex(packager.CliReleaseError, "fixture SHA-256"):
                packager.smoke_release(
                    archive_path=archive,
                    target=target,
                    version="1.2.3",
                    policy=policy,
                )

            fixture_path.write_bytes(fixture_content)
            process_cases = (
                (
                    subprocess.CompletedProcess(
                        (), 1, stdout=SMOKE_EXPECTED_STDOUT, stderr=b""
                    ),
                    "exit code 1",
                ),
                (
                    subprocess.CompletedProcess(
                        (), 0, stdout=SMOKE_EXPECTED_STDOUT, stderr=b"warning"
                    ),
                    "standard error",
                ),
                (
                    subprocess.CompletedProcess(
                        (), 0, stdout=SMOKE_EXPECTED_STDOUT + b" ", stderr=b""
                    ),
                    "standard output",
                ),
            )
            for completed, expected_error in process_cases:
                with self.subTest(expected_error=expected_error):

                    def runner(
                        _command: tuple[str, ...],
                        completed_result: subprocess.CompletedProcess[
                            bytes
                        ] = completed,
                        **_options: object,
                    ) -> subprocess.CompletedProcess[bytes]:
                        return completed_result

                    with self.assertRaisesRegex(
                        packager.CliReleaseError, expected_error
                    ) as caught:
                        packager.smoke_release(
                            archive_path=archive,
                            target=target,
                            version="1.2.3",
                            policy=policy,
                            runner=runner,
                        )
                    self.assertNotIn("warning", str(caught.exception))

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
            "python scripts/build_cli_release.py smoke",
            '--archive-dir "dist/subjects"',
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
        for smoke_input in (
            "cli-release-smoke.toml",
            "tests/fixtures/generated/basic_text.pdf",
            "tests/fixtures/expected/cli-release-basic-text.jsonl",
        ):
            self.assertIn(smoke_input, ci_workflow)

        release = RELEASE_PATH.read_text(encoding="utf-8")
        self.assertIn("uses: ./.github/workflows/cli-binaries.yml", release)
        self.assertIn("release_tag: ${{ github.ref_name }}", release)
        self.assertIn("publish-pypi,", release)
        self.assertIn("cli-binaries,", release)
        self.assertIn("release-artifacts,", release)
        self.assertIn("integrity,", release)
        self.assertIn("pattern: cli-binary-*", release)
        self.assertIn("path: release-cli-binaries", release)
        self.assertIn("release-cli-binaries/subjects/*", release)
        self.assertIn("release-cli-binaries/integrity/*", release)

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
            "basic_text.pdf",
            "exact standard output",
            "target operating system",
            "release integrity gate",
            "SHA256SUMS",
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
                self.assertIn(target, verified)
        self.assertNotIn("DIST-004", limitations)
        self.assertNotIn("not runtime-smoke-tested", limitations)
        self.assertIn("cli-release-targets.toml", evidence)
        self.assertIn("cli-release-smoke.toml", evidence)
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
