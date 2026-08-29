"""Prepublication WebAssembly/TypeScript consumer contracts (DIST-013)."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from types import ModuleType

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = REPO_ROOT / "wasm-package-test-policy.toml"
CHECKER_PATH = REPO_ROOT / "scripts" / "check_wasm_package.py"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
RELEASE_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
HARNESS_PATH = REPO_ROOT / "compat" / "wasm-package-tests"
GUIDE_PATH = REPO_ROOT / "docs" / "wasm-package-testing.md"
REFERENCE_PATH = REPO_ROOT / "references" / "wasm-package-testing.md"
REFERENCE_INDEX_PATH = REPO_ROOT / "references" / "INDEX.md"
SUPPORT_PATH = REPO_ROOT / "support-matrix.toml"

EXPECTED_POLICY = {
    "schema_version": 1,
    "node_version": "24.20.0",
    "wasm_pack_version": "0.14.0",
    "typescript_version": "7.0.2",
    "vite_version": "8.2.2",
    "playwright_version": "1.62.1",
    "browser": "chromium",
    "fixture": "tests/fixtures/generated/basic_text.pdf",
    "expected": "tests/fixtures/expected/cli-release-basic-text.jsonl",
}


def load_checker() -> ModuleType | None:
    if not CHECKER_PATH.is_file():
        return None
    spec = importlib.util.spec_from_file_location("check_wasm_package", CHECKER_PATH)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class WasmPackageReleaseTests(unittest.TestCase):
    def require_checker(self) -> ModuleType:
        checker = load_checker()
        self.assertIsNotNone(checker, "missing WebAssembly package checker")
        assert checker is not None
        return checker

    def test_policy_pins_supported_node_and_browser_toolchain(self) -> None:
        self.assertTrue(POLICY_PATH.is_file(), "missing WebAssembly package policy")
        if not POLICY_PATH.is_file():
            return

        policy = tomllib.loads(POLICY_PATH.read_text(encoding="utf-8"))
        self.assertEqual(policy, EXPECTED_POLICY)

    def test_consumer_harness_locks_typescript_vite_playwright_and_real_fixture(
        self,
    ) -> None:
        required = (
            "package.json",
            "package-lock.json",
            "tsconfig.node.json",
            "tsconfig.browser.json",
            "node-consumer.ts",
            "browser-consumer.ts",
            "index.html",
            "run-browser.mjs",
        )
        for relative in required:
            with self.subTest(relative=relative):
                self.assertTrue((HARNESS_PATH / relative).is_file())
        if not all((HARNESS_PATH / relative).is_file() for relative in required):
            return

        package = json.loads((HARNESS_PATH / "package.json").read_text(encoding="utf-8"))
        self.assertEqual(
            package.get("devDependencies"),
            {
                "@playwright/test": "1.62.1",
                "@types/node": "24.13.3",
                "typescript": "7.0.2",
                "vite": "8.2.2",
            },
        )
        lock = json.loads(
            (HARNESS_PATH / "package-lock.json").read_text(encoding="utf-8")
        )
        self.assertEqual(lock.get("lockfileVersion"), 3)
        self.assertTrue(lock.get("packages", {}).get("", {}).get("devDependencies"))

        node_source = (HARNESS_PATH / "node-consumer.ts").read_text(encoding="utf-8")
        browser_source = (HARNESS_PATH / "browser-consumer.ts").read_text(
            encoding="utf-8"
        )
        browser_runner = (HARNESS_PATH / "run-browser.mjs").read_text(
            encoding="utf-8"
        )
        for source in (node_source, browser_source):
            self.assertIn('from "pdfplumber-wasm"', source)
            self.assertIn("WasmPdf.open", source)
            self.assertIn("pageCount", source)
            self.assertIn("extractText", source)
        self.assertIn("chromium.launch", browser_runner)
        self.assertIn("data-wasm-status", browser_runner)
        self.assertNotIn("SKIP", node_source + browser_source + browser_runner)

    def test_ci_builds_packs_typechecks_installs_and_executes_both_consumers(
        self,
    ) -> None:
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        phrases = (
            "name: WASM package consumers",
            'node-version: "24.20.0"',
            "cache-dependency-path: compat/wasm-package-tests/package-lock.json",
            "version: v0.14.0",
            "npm ci --prefix compat/wasm-package-tests",
            "npx --prefix compat/wasm-package-tests playwright install --with-deps chromium",
            "wasm-pack build --target bundler --out-dir pkg-browser crates/pdfplumber-wasm",
            "wasm-pack build --target nodejs --out-dir pkg-node crates/pdfplumber-wasm",
            "Copy checked TypeScript declarations into candidate packages",
            "python scripts/check_wasm_package.py",
            "--node-package crates/pdfplumber-wasm/pkg-node",
            "--browser-package crates/pdfplumber-wasm/pkg-browser",
            "--output dist/wasm-package-report.json",
            "name: wasm-package-evidence-${{ github.sha }}",
        )
        for phrase in phrases:
            with self.subTest(workflow_phrase=phrase):
                self.assertIn(phrase, workflow)
        for earlier, later in (
            ("npm ci --prefix", "playwright install --with-deps chromium"),
            ("wasm-pack build --target bundler", "Copy checked TypeScript declarations"),
            ("Copy checked TypeScript declarations", "python scripts/check_wasm_package.py"),
        ):
            self.assertLess(workflow.index(earlier), workflow.index(later))

    def test_tagged_npm_publication_is_blocked_on_the_reusable_ci_gate(self) -> None:
        release = RELEASE_WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertIn("ci:\n    uses: ./.github/workflows/ci.yml", release)
        publish = release[release.index("  publish-npm:") :]
        self.assertIn("needs: [ci, metadata, scorecards]", publish)
        self.assertIn('node-version: "24.20.0"', publish)
        self.assertIn("version: v0.14.0", publish)
        self.assertLess(
            publish.index("Copy hand-authored type definitions"),
            publish.index("npm publish crates/pdfplumber-wasm/pkg --access public"),
        )

    def test_checker_rejects_package_toolchain_and_runtime_evidence_drift(
        self,
    ) -> None:
        checker = self.require_checker()
        policy = EXPECTED_POLICY.copy()
        versions = {
            "node": "24.20.0",
            "wasm_pack": "0.14.0",
            "typescript": "7.0.2",
            "vite": "8.2.2",
            "playwright": "1.62.1",
        }
        checker.validate_tool_versions(policy, versions)
        checker.validate_package_manifest(
            {
                "name": "pdfplumber-wasm",
                "version": "0.3.0",
                "types": "pdfplumber_wasm.d.ts",
            }
        )
        checker.validate_runtime_result(
            {
                "runtime": "node",
                "page_count": 1,
                "text_sha256": "a" * 64,
                "fixture_sha256": "b" * 64,
                "expected_sha256": "c" * 64,
            },
            "node",
            "a" * 64,
            "b" * 64,
            "c" * 64,
        )

        failures = (
            (checker.validate_tool_versions, (policy, {**versions, "node": "25.5.0"})),
            (
                checker.validate_package_manifest,
                ({"name": "not-pdfplumber", "version": "0.3.0"},),
            ),
            (
                checker.validate_runtime_result,
                (
                    {
                        "runtime": "browser",
                        "page_count": 0,
                        "text_sha256": "a" * 64,
                        "fixture_sha256": "b" * 64,
                        "expected_sha256": "c" * 64,
                    },
                    "browser",
                    "a" * 64,
                    "b" * 64,
                    "c" * 64,
                ),
            ),
        )
        for function, arguments in failures:
            with self.subTest(function=function.__name__), self.assertRaises(
                checker.WasmPackageError
            ):
                function(*arguments)

    def test_checker_report_binds_packages_wasm_inputs_tools_and_both_runtimes(
        self,
    ) -> None:
        checker = self.require_checker()
        node_bytes = b"node candidate archive\n"
        browser_bytes = b"browser candidate archive\n"
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            node_package = root / "node.tgz"
            browser_package = root / "browser.tgz"
            node_package.write_bytes(node_bytes)
            browser_package.write_bytes(browser_bytes)
            report = checker.build_report(
                source_commit="1" * 40,
                policy_path=POLICY_PATH,
                policy=EXPECTED_POLICY,
                node_package=node_package,
                node_wasm_sha256="2" * 64,
                browser_package=browser_package,
                browser_wasm_sha256="3" * 64,
                tool_versions={
                    "node": "24.20.0",
                    "wasm_pack": "0.14.0",
                    "typescript": "7.0.2",
                    "vite": "8.2.2",
                    "playwright": "1.62.1",
                    "browser": "Chromium 151.0.7922.34",
                },
                fixture_sha256="4" * 64,
                expected_sha256="5" * 64,
                text_sha256="6" * 64,
                node_result={"runtime": "node", "page_count": 1},
                browser_result={"runtime": "browser", "page_count": 1},
            )

        self.assertEqual(report["schema_version"], 1)
        self.assertEqual(report["outcome"], "compatible")
        self.assertEqual(report["source_commit"], "1" * 40)
        self.assertEqual(report["packages"]["node"]["name"], "node.tgz")
        self.assertEqual(report["packages"]["browser"]["name"], "browser.tgz")
        self.assertEqual(report["packages"]["node"]["wasm_sha256"], "2" * 64)
        self.assertEqual(report["packages"]["browser"]["wasm_sha256"], "3" * 64)
        self.assertEqual(set(report["runtimes"]), {"node", "browser"})

    def test_guidance_and_support_state_exact_proof_and_limitations(self) -> None:
        for path in (GUIDE_PATH, REFERENCE_PATH):
            with self.subTest(path=path.relative_to(REPO_ROOT)):
                self.assertTrue(path.is_file())

        if GUIDE_PATH.is_file():
            guide = GUIDE_PATH.read_text(encoding="utf-8")
            for phrase in (
                "Node.js 24.20.0",
                "TypeScript 7.0.2",
                "Playwright 1.62.1",
                "Chromium",
                "fresh npm package archive",
                "does not prove compatibility with every browser",
            ):
                with self.subTest(guide_phrase=phrase):
                    self.assertIn(phrase, guide)

        support = tomllib.loads(SUPPORT_PATH.read_text(encoding="utf-8"))
        wasm = next(surface for surface in support["surfaces"] if surface["id"] == "wasm")
        verified = "\n".join(wasm["ci_verified_platforms"])
        self.assertIn("Node.js 24.20.0", verified)
        self.assertIn("Playwright 1.62.1 Chromium", verified)
        limitations = "\n".join(wasm["known_limitations"])
        self.assertNotIn("browser end-to-end behavior is not gated", limitations)
        self.assertIn("does not establish compatibility with every browser", limitations)
        for relative in (
            "wasm-package-test-policy.toml",
            "scripts/check_wasm_package.py",
            "compat/tests/test_wasm_package_release.py",
            "docs/wasm-package-testing.md",
        ):
            self.assertIn(relative, wasm["evidence"])
        index = REFERENCE_INDEX_PATH.read_text(encoding="utf-8")
        self.assertIn("[wasm-package-testing.md](wasm-package-testing.md)", index)


if __name__ == "__main__":
    unittest.main()
