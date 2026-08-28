"""Rolling-stable Rust and current lopdf dependency contracts (DX-013)."""

from __future__ import annotations

import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]


def load_toml(path: Path) -> dict:
    return tomllib.loads(path.read_text(encoding="utf-8"))


class RustStablePolicyContractTests(unittest.TestCase):
    def test_packages_do_not_publish_a_minimum_rust_version(self) -> None:
        workspace = load_toml(REPO_ROOT / "Cargo.toml")
        self.assertNotIn("rust-version", workspace["workspace"]["package"])

        for manifest_path in sorted((REPO_ROOT / "crates").glob("*/Cargo.toml")):
            with self.subTest(manifest=manifest_path.parent.name):
                package = load_toml(manifest_path)["package"]
                self.assertNotIn("rust-version", package)

    def test_ci_tracks_only_the_current_stable_channel(self) -> None:
        workflow = (REPO_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("name: Test (stable)", workflow)
        self.assertIn("dtolnay/rust-toolchain@stable", workflow)
        self.assertNotIn("matrix.rust", workflow)
        self.assertNotIn('"1.85"', workflow)

    def test_support_policy_is_rolling_stable_without_an_msrv_claim(self) -> None:
        matrix = load_toml(REPO_ROOT / "support-matrix.toml")
        self.assertEqual(matrix["rust_policy"], "rolling-stable")
        self.assertNotIn("rust_version", matrix)

        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        support = (REPO_ROOT / "docs/support.md").read_text(encoding="utf-8")
        self.assertNotIn("MSRV", readme)
        self.assertNotIn("Rust 1.85", readme)
        self.assertIn("latest stable Rust", readme)
        self.assertNotIn("Rust 1.85", support)
        self.assertIn("rolling stable", support.lower())

    def test_parser_uses_one_current_lopdf_release(self) -> None:
        parser_manifest = load_toml(
            REPO_ROOT / "crates/pdfplumber-parse/Cargo.toml"
        )
        dependencies = parser_manifest["dependencies"]
        self.assertEqual(dependencies["lopdf"]["version"], "0.44")
        self.assertNotIn("lopdf_pre_nom_locate", dependencies)

        lockfile = load_toml(REPO_ROOT / "Cargo.lock")
        parser_lock = next(
            package
            for package in lockfile["package"]
            if package["name"] == "pdfplumber-parse"
        )
        parser_lopdf_dependencies = [
            dependency
            for dependency in parser_lock["dependencies"]
            if dependency.startswith("lopdf")
        ]
        self.assertEqual(parser_lopdf_dependencies, ["lopdf 0.44.0"])

        backend = (
            REPO_ROOT / "crates/pdfplumber-parse/src/lopdf_backend.rs"
        ).read_text(encoding="utf-8")
        self.assertNotIn("lopdf_pre_nom_locate", backend)
        self.assertNotIn("try_load_object_dense_unencrypted", backend)

    def test_wasm_bridge_tracks_the_current_parser_rng(self) -> None:
        wasm_manifest = load_toml(
            REPO_ROOT / "crates/pdfplumber-wasm/Cargo.toml"
        )
        wasm_dependencies = wasm_manifest["target"][
            'cfg(all(target_arch = "wasm32", target_os = "unknown"))'
        ]["dependencies"]
        self.assertEqual(wasm_dependencies["getrandom"]["version"], "0.4")
        self.assertEqual(wasm_dependencies["getrandom"]["features"], ["wasm_js"])

    def test_policy_is_publicly_traceable(self) -> None:
        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        index = (REPO_ROOT / "references/INDEX.md").read_text(encoding="utf-8")
        reference_path = REPO_ROOT / "references/rust-toolchain-policy.md"
        prd = (REPO_ROOT / "PRD.md").read_text(encoding="utf-8")

        self.assertRegex(changelog, r"(?i)rolling stable Rust")
        self.assertIn("rust-toolchain-policy.md", index)
        self.assertTrue(reference_path.is_file())
        self.assertIn("channel-rust-stable", reference_path.read_text(encoding="utf-8"))
        self.assertRegex(prd, r"\[x\] \*\*DX-013\*\*.*rolling stable")


if __name__ == "__main__":
    unittest.main()
