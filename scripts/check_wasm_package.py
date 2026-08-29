#!/usr/bin/env python3
"""Install and exercise candidate WebAssembly packages in Node and Chromium."""

from __future__ import annotations

import argparse
import functools
import hashlib
import http.server
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
from pathlib import Path
from typing import Any

import tomllib

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_POLICY = ROOT / "wasm-package-test-policy.toml"
DEFAULT_TOOLS = ROOT / "compat" / "wasm-package-tests"


class WasmPackageError(RuntimeError):
    """The candidate package or its execution evidence violated the policy."""


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def load_policy(path: Path) -> dict[str, Any]:
    policy = tomllib.loads(path.read_text(encoding="utf-8"))
    required = {
        "schema_version",
        "node_version",
        "wasm_pack_version",
        "typescript_version",
        "vite_version",
        "playwright_version",
        "browser",
        "fixture",
        "expected",
    }
    if set(policy) != required:
        raise WasmPackageError(
            f"policy keys must be exactly {sorted(required)}, observed {sorted(policy)}"
        )
    if policy["schema_version"] != 1 or policy["browser"] != "chromium":
        raise WasmPackageError("unsupported WebAssembly package policy")
    for key in required - {"schema_version"}:
        if not isinstance(policy[key], str) or not policy[key]:
            raise WasmPackageError(f"policy {key} must be a non-empty string")
    return policy


def validate_tool_versions(policy: dict[str, Any], versions: dict[str, str]) -> None:
    expected = {
        "node": policy["node_version"],
        "wasm_pack": policy["wasm_pack_version"],
        "typescript": policy["typescript_version"],
        "vite": policy["vite_version"],
        "playwright": policy["playwright_version"],
    }
    observed = {key: versions.get(key) for key in expected}
    if observed != expected:
        raise WasmPackageError(
            f"toolchain drift: expected {expected!r}, observed {observed!r}"
        )


def validate_source_checkout(
    expected_commit: str, head_commit: str, status: str
) -> None:
    if expected_commit != head_commit:
        raise WasmPackageError(
            f"source commit {expected_commit} != checkout HEAD {head_commit}"
        )
    if status.strip():
        raise WasmPackageError(f"source checkout is dirty:\n{status.rstrip()}")


def validate_package_manifest(manifest: dict[str, Any]) -> None:
    expected = {
        "name": "pdfplumber-wasm",
        "version": "0.3.0",
        "types": "pdfplumber_wasm.d.ts",
    }
    observed = {key: manifest.get(key) for key in expected}
    if observed != expected:
        raise WasmPackageError(
            f"candidate package identity drift: expected {expected!r}, observed {observed!r}"
        )


def validate_runtime_result(
    result: dict[str, Any],
    runtime: str,
    text_sha256: str,
    fixture_sha256: str,
    expected_sha256: str,
) -> None:
    expected = {
        "runtime": runtime,
        "page_count": 1,
        "text_sha256": text_sha256,
        "fixture_sha256": fixture_sha256,
        "expected_sha256": expected_sha256,
    }
    if result != expected:
        raise WasmPackageError(
            f"{runtime} result drift: expected {expected!r}, observed {result!r}"
        )


def package_record(path: Path, wasm_sha256: str) -> dict[str, Any]:
    return {
        "name": path.name,
        "sha256": sha256_file(path),
        "size": path.stat().st_size,
        "wasm_sha256": wasm_sha256,
    }


def build_report(
    *,
    source_commit: str,
    policy_path: Path,
    policy: dict[str, Any],
    node_package: Path,
    node_wasm_sha256: str,
    browser_package: Path,
    browser_wasm_sha256: str,
    tool_versions: dict[str, str],
    fixture_sha256: str,
    expected_sha256: str,
    text_sha256: str,
    node_result: dict[str, Any],
    browser_result: dict[str, Any],
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "outcome": "compatible",
        "source_commit": source_commit,
        "policy": {
            "name": policy_path.name,
            "sha256": sha256_file(policy_path),
            "values": policy,
        },
        "packages": {
            "node": package_record(node_package, node_wasm_sha256),
            "browser": package_record(browser_package, browser_wasm_sha256),
        },
        "tool_versions": tool_versions,
        "inputs": {
            "fixture_sha256": fixture_sha256,
            "expected_sha256": expected_sha256,
            "text_sha256": text_sha256,
        },
        "runtimes": {"node": node_result, "browser": browser_result},
    }


def run(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    timeout: int = 180,
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=False,
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    if completed.returncode != 0:
        raise WasmPackageError(
            f"command failed ({completed.returncode}) in {cwd}: {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed


def tool_binary(tools: Path, name: str) -> Path:
    suffix = ".cmd" if os.name == "nt" else ""
    path = tools / "node_modules" / ".bin" / f"{name}{suffix}"
    if not path.is_file():
        raise WasmPackageError(f"missing locked tool binary: {path}")
    return path


def parse_version(output: str, pattern: str, label: str) -> str:
    match = re.search(pattern, output.strip())
    if match is None:
        raise WasmPackageError(f"could not parse {label} version from {output!r}")
    return match.group(1)


def collect_tool_versions(tools: Path) -> dict[str, str]:
    commands = {
        "node": (["node", "--version"], r"^v([0-9]+\.[0-9]+\.[0-9]+)$"),
        "wasm_pack": (
            ["wasm-pack", "--version"],
            r"^wasm-pack ([0-9]+\.[0-9]+\.[0-9]+)$",
        ),
        "typescript": (
            [str(tool_binary(tools, "tsc")), "--version"],
            r"^Version ([0-9]+\.[0-9]+\.[0-9]+)$",
        ),
        "vite": (
            [str(tool_binary(tools, "vite")), "--version"],
            r"^vite/([0-9]+\.[0-9]+\.[0-9]+)",
        ),
        "playwright": (
            [str(tool_binary(tools, "playwright")), "--version"],
            r"^Version ([0-9]+\.[0-9]+\.[0-9]+)$",
        ),
    }
    versions: dict[str, str] = {}
    for label, (command, pattern) in commands.items():
        completed = run(command, cwd=ROOT)
        versions[label] = parse_version(completed.stdout, pattern, label)
    return versions


def read_manifest(package: Path) -> dict[str, Any]:
    manifest_path = package / "package.json"
    if not manifest_path.is_file():
        raise WasmPackageError(f"missing candidate package manifest: {manifest_path}")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    validate_package_manifest(manifest)
    declarations = package / manifest["types"]
    if not declarations.is_file():
        raise WasmPackageError(
            f"missing checked TypeScript declarations: {declarations}"
        )
    return manifest


def single_wasm(package: Path) -> Path:
    files = sorted(package.glob("*.wasm"))
    if len(files) != 1:
        raise WasmPackageError(
            f"expected exactly one WebAssembly binary in {package}: {files}"
        )
    return files[0]


def pack_candidate(package: Path, destination: Path) -> Path:
    destination.mkdir(parents=True)
    completed = run(
        [
            "npm",
            "pack",
            str(package),
            "--pack-destination",
            str(destination),
            "--json",
        ],
        cwd=ROOT,
    )
    records = json.loads(completed.stdout)
    if not isinstance(records, list) or len(records) != 1:
        raise WasmPackageError(
            f"npm pack returned an unexpected inventory: {records!r}"
        )
    archive = destination / records[0]["filename"]
    if not archive.is_file():
        raise WasmPackageError(f"npm pack did not create {archive}")
    return archive


def install_candidate(consumer: Path, archive: Path) -> None:
    (consumer / "package.json").write_text(
        json.dumps(
            {
                "name": f"pdfplumber-wasm-{consumer.name}",
                "version": "0.0.0",
                "private": True,
                "type": "module",
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    run(
        [
            "npm",
            "install",
            "--ignore-scripts",
            "--no-audit",
            "--no-fund",
            "--package-lock=false",
            "--offline",
            str(archive),
        ],
        cwd=consumer,
    )


def parse_json_output(completed: subprocess.CompletedProcess[str], label: str) -> Any:
    lines = [line for line in completed.stdout.splitlines() if line.strip()]
    if not lines:
        raise WasmPackageError(f"{label} produced no JSON result")
    try:
        return json.loads(lines[-1])
    except json.JSONDecodeError as error:
        raise WasmPackageError(
            f"{label} final output was not JSON: {lines[-1]!r}"
        ) from error


def run_node_consumer(
    consumer: Path,
    tools: Path,
    fixture: Path,
    expected: Path,
) -> dict[str, Any]:
    for name in ("node-consumer.ts", "tsconfig.node.json"):
        shutil.copyfile(tools / name, consumer / name)
    type_roots = tools / "node_modules" / "@types"
    run(
        [
            str(tool_binary(tools, "tsc")),
            "--project",
            "tsconfig.node.json",
            "--typeRoots",
            str(type_roots),
        ],
        cwd=consumer,
    )
    completed = run(
        [
            "node",
            "dist/node-consumer.js",
            str(fixture),
            str(expected),
        ],
        cwd=consumer,
    )
    result = parse_json_output(completed, "Node consumer")
    if not isinstance(result, dict):
        raise WasmPackageError("Node consumer result must be an object")
    return result


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, format: str, *args: object) -> None:
        del format, args


def run_browser_consumer(
    consumer: Path,
    tools: Path,
    fixture: Path,
    expected: Path,
) -> tuple[dict[str, Any], str]:
    for name in ("browser-consumer.ts", "index.html", "tsconfig.browser.json"):
        shutil.copyfile(tools / name, consumer / name)
    public = consumer / "public"
    public.mkdir()
    shutil.copyfile(fixture, public / "document.pdf")
    shutil.copyfile(expected, public / "expected.jsonl")
    run(
        [
            str(tool_binary(tools, "tsc")),
            "--project",
            "tsconfig.browser.json",
        ],
        cwd=consumer,
    )
    run(
        [str(tool_binary(tools, "vite")), "build", "--outDir", "dist"],
        cwd=consumer,
    )

    handler = functools.partial(QuietHandler, directory=str(consumer / "dist"))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        port = server.server_address[1]
        completed = run(
            [
                "node",
                str(tools / "run-browser.mjs"),
                f"http://127.0.0.1:{port}/",
            ],
            cwd=tools,
            timeout=120,
        )
    finally:
        server.shutdown()
        thread.join(timeout=10)
        server.server_close()

    payload = parse_json_output(completed, "browser consumer")
    if not isinstance(payload, dict) or not isinstance(payload.get("result"), dict):
        raise WasmPackageError("browser consumer result must contain an object result")
    browser = payload.get("browser")
    if not isinstance(browser, str) or not browser.startswith("Chromium "):
        raise WasmPackageError(f"unexpected browser identity: {browser!r}")
    return payload["result"], browser


def expected_text(path: Path) -> str:
    lines = [line for line in path.read_text(encoding="utf-8").splitlines() if line]
    if len(lines) != 1:
        raise WasmPackageError(
            "expected fixture output must contain exactly one record"
        )
    record = json.loads(lines[0])
    if record.get("page") != 1 or not isinstance(record.get("text"), str):
        raise WasmPackageError("expected fixture output must describe page 1 text")
    return record["text"]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    parser.add_argument("--tools", type=Path, default=DEFAULT_TOOLS)
    parser.add_argument("--node-package", type=Path, required=True)
    parser.add_argument("--browser-package", type=Path, required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if re.fullmatch(r"[0-9a-f]{40}", args.source_commit) is None:
        raise WasmPackageError("source commit must be a lowercase 40-character Git SHA")

    policy_path = args.policy.resolve()
    tools = args.tools.resolve()
    node_package_dir = args.node_package.resolve()
    browser_package_dir = args.browser_package.resolve()
    output = args.output.resolve()
    head_commit = run(["git", "rev-parse", "HEAD"], cwd=ROOT).stdout.strip()
    source_status = run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"], cwd=ROOT
    ).stdout
    validate_source_checkout(args.source_commit, head_commit, source_status)
    policy = load_policy(policy_path)
    versions = collect_tool_versions(tools)
    validate_tool_versions(policy, versions)
    read_manifest(node_package_dir)
    read_manifest(browser_package_dir)
    node_wasm_sha256 = sha256_file(single_wasm(node_package_dir))
    browser_wasm_sha256 = sha256_file(single_wasm(browser_package_dir))

    fixture = (ROOT / policy["fixture"]).resolve()
    expected = (ROOT / policy["expected"]).resolve()
    text_sha256 = sha256_bytes(expected_text(expected).encode("utf-8"))
    fixture_sha256 = sha256_file(fixture)
    expected_sha256 = sha256_file(expected)

    with tempfile.TemporaryDirectory(prefix="pdfplumber-wasm-package-") as temporary:
        root = Path(temporary)
        node_archive = pack_candidate(node_package_dir, root / "node-package")
        browser_archive = pack_candidate(browser_package_dir, root / "browser-package")

        node_consumer = root / "node-consumer"
        browser_consumer = root / "browser-consumer"
        node_consumer.mkdir()
        browser_consumer.mkdir()
        install_candidate(node_consumer, node_archive)
        install_candidate(browser_consumer, browser_archive)

        node_result = run_node_consumer(node_consumer, tools, fixture, expected)
        browser_result, browser_version = run_browser_consumer(
            browser_consumer, tools, fixture, expected
        )
        validate_runtime_result(
            node_result,
            "node",
            text_sha256,
            fixture_sha256,
            expected_sha256,
        )
        validate_runtime_result(
            browser_result,
            "browser",
            text_sha256,
            fixture_sha256,
            expected_sha256,
        )
        output.parent.mkdir(parents=True, exist_ok=True)
        retained_node = output.parent / "pdfplumber-wasm-node.tgz"
        retained_browser = output.parent / "pdfplumber-wasm-browser.tgz"
        shutil.copyfile(node_archive, retained_node)
        shutil.copyfile(browser_archive, retained_browser)
        report_versions = {**versions, "browser": browser_version}
        report = build_report(
            source_commit=args.source_commit,
            policy_path=policy_path,
            policy=policy,
            node_package=retained_node,
            node_wasm_sha256=node_wasm_sha256,
            browser_package=retained_browser,
            browser_wasm_sha256=browser_wasm_sha256,
            tool_versions=report_versions,
            fixture_sha256=fixture_sha256,
            expected_sha256=expected_sha256,
            text_sha256=text_sha256,
            node_result=node_result,
            browser_result=browser_result,
        )

        output.write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )

    print(
        "WebAssembly package consumers passed: "
        f"Node.js {versions['node']}, {browser_version}, exact one-page output"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, WasmPackageError, subprocess.TimeoutExpired) as error:
        print(f"WebAssembly package check failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
