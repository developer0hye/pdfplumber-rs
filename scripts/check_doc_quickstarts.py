#!/usr/bin/env python3
"""Validate and execute installation and quick-start snippets from public docs."""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUST_FIXTURE = ROOT / "tests/fixtures/generated/basic_text.pdf"
CLI_FIXTURE = ROOT / "tests/fixtures/generated/long_document.pdf"
FENCE = re.compile(r"^```([^\n]*)\n(.*?)^```\s*$", re.MULTILINE | re.DOTALL)


@dataclass(frozen=True)
class Surface:
    document: str
    installation_language: str
    installation: str
    quick_start_languages: tuple[str, ...]


SURFACES = {
    "rust": Surface(
        document="README.md",
        installation_language="toml",
        installation='[dependencies]\npdfplumber = "0.3"',
        quick_start_languages=("rust", "rust", "rust", "rust", "rust"),
    ),
    "python": Surface(
        document="crates/pdfplumber-py/README.md",
        installation_language="bash",
        installation="pip install pdfplumber-rs",
        quick_start_languages=("python",),
    ),
    "cli": Surface(
        document="crates/pdfplumber-cli/README.md",
        installation_language="bash",
        installation="cargo install pdfplumber-cli",
        quick_start_languages=("bash",),
    ),
    "wasm-node": Surface(
        document="crates/pdfplumber-wasm/README.md",
        installation_language="bash",
        installation="npm install pdfplumber-wasm",
        quick_start_languages=("javascript",),
    ),
}

CI_COMMANDS = (
    "python scripts/check_doc_quickstarts.py --check",
    "python scripts/check_doc_quickstarts.py --rust --cli",
    "python scripts/check_doc_quickstarts.py --python --wheel dist/*.whl",
    "python scripts/check_doc_quickstarts.py --wasm-node --npm-package crates/pdfplumber-wasm/pkg-node",
)


class QuickStartError(RuntimeError):
    """A rendered snippet or clean-environment execution is invalid."""


def section(text: str, heading: str) -> str:
    match = re.search(rf"^## {re.escape(heading)}[^\n]*$", text, re.MULTILINE)
    if match is None:
        raise QuickStartError(f"missing level-two {heading!r} section")
    next_heading = re.search(r"^## ", text[match.end() :], re.MULTILINE)
    end = len(text) if next_heading is None else match.end() + next_heading.start()
    return text[match.end() : end]


def executable_fences(text: str) -> list[tuple[str, str]]:
    fences: list[tuple[str, str]] = []
    for match in FENCE.finditer(text):
        language = match.group(1).split(",", 1)[0].strip()
        if language in {"bash", "javascript", "python", "rust", "toml"}:
            fences.append((language, match.group(2).strip()))
    return fences


def surface_snippets(name: str) -> tuple[str, list[str]]:
    spec = SURFACES[name]
    text = (ROOT / spec.document).read_text(encoding="utf-8")
    installation = executable_fences(section(text, "Installation"))
    expected_installation = [(spec.installation_language, spec.installation)]
    if installation != expected_installation:
        raise QuickStartError(
            f"{spec.document}: installation drift:\n"
            f"expected {expected_installation!r}, observed {installation!r}"
        )

    quick_starts = executable_fences(section(text, "Quick Start"))
    languages = tuple(language for language, _ in quick_starts)
    if languages != spec.quick_start_languages:
        raise QuickStartError(
            f"{spec.document}: quick-start languages drift:\n"
            f"expected {spec.quick_start_languages!r}, observed {languages!r}"
        )
    return installation[0][1], [body for _, body in quick_starts]


def check_static_contract() -> None:
    counts = []
    for name in SURFACES:
        _, quick_starts = surface_snippets(name)
        counts.append(f"{name}={len(quick_starts)}")

    workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    missing = [command for command in CI_COMMANDS if command not in workflow]
    if missing:
        raise QuickStartError(f"CI does not execute documentation contract: {missing}")
    print(f"documentation quick-start contract verified: {', '.join(counts)}")


def run(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
) -> None:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        rendered = " ".join(command)
        raise QuickStartError(
            f"command failed ({completed.returncode}) in {cwd}: {rendered}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )


def rust_program(snippet: str) -> str:
    if re.search(r"\bfn main\s*\(", snippet):
        return snippet + "\n"

    lines = snippet.splitlines()
    imports: list[str] = []
    body: list[str] = []
    in_imports = True
    for line in lines:
        if in_imports and (line.startswith("use ") or not line.strip()):
            imports.append(line)
        else:
            in_imports = False
            body.append(line)
    indented = "\n".join(f"    {line}" if line else "" for line in body)
    return f"{'\n'.join(imports).rstrip()}\n\nfn main() {{\n{indented}\n}}\n"


def run_rust_quick_starts() -> None:
    _, snippets = surface_snippets("rust")
    with tempfile.TemporaryDirectory(prefix="pdfplumber-rust-docs-") as temp:
        consumer = Path(temp)
        bins = consumer / "src/bin"
        bins.mkdir(parents=True)
        cargo_toml = (
            "[package]\n"
            'name = "pdfplumber-doc-quickstarts"\n'
            'version = "0.0.0"\n'
            'edition = "2021"\n\n'
            f"{SURFACES['rust'].installation}\n\n"
            "[patch.crates-io]\n"
            f'pdfplumber = {{ path = "{ROOT / "crates/pdfplumber"}" }}\n'
        )
        (consumer / "Cargo.toml").write_text(cargo_toml, encoding="utf-8")
        shutil.copyfile(RUST_FIXTURE, consumer / "document.pdf")

        names: list[str] = []
        for index, snippet in enumerate(snippets, start=1):
            name = f"quickstart_{index}"
            names.append(name)
            (bins / f"{name}.rs").write_text(rust_program(snippet), encoding="utf-8")

        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(ROOT / "target/doc-quickstarts/rust")
        for name in names:
            run(["cargo", "run", "--quiet", "--bin", name], cwd=consumer, env=env)
    print(f"rendered Rust quick starts passed: {len(snippets)}")


def run_cli_quick_start() -> None:
    _, snippets = surface_snippets("cli")
    with tempfile.TemporaryDirectory(prefix="pdfplumber-cli-docs-") as temp:
        consumer = Path(temp)
        install_root = consumer / "install"
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(ROOT / "target/doc-quickstarts/cli")
        run(
            [
                "cargo",
                "install",
                "--path",
                str(ROOT / "crates/pdfplumber-cli"),
                "--root",
                str(install_root),
                "--debug",
            ],
            cwd=consumer,
            env=env,
        )
        shutil.copyfile(CLI_FIXTURE, consumer / "document.pdf")
        env["PATH"] = f"{install_root / 'bin'}{os.pathsep}{env['PATH']}"
        for snippet in snippets:
            run(["bash", "-euxo", "pipefail", "-c", snippet], cwd=consumer, env=env)
    print(f"rendered CLI quick starts passed: {len(snippets)}")


def single_artifact(paths: list[Path], label: str) -> Path:
    existing = [path.resolve() for path in paths if path.is_file() or path.is_dir()]
    if len(existing) != 1:
        raise QuickStartError(f"expected exactly one {label}, observed {existing}")
    return existing[0]


def run_python_quick_start(wheels: list[Path]) -> None:
    wheel = single_artifact(wheels, "wheel")
    _, snippets = surface_snippets("python")
    with tempfile.TemporaryDirectory(prefix="pdfplumber-python-docs-") as temp:
        consumer = Path(temp)
        venv = consumer / ".venv"
        run([sys.executable, "-m", "venv", str(venv)], cwd=consumer)
        python = venv / "bin/python"
        run(
            [str(python), "-m", "pip", "install", "--no-deps", str(wheel)], cwd=consumer
        )
        shutil.copyfile(RUST_FIXTURE, consumer / "document.pdf")
        script = consumer / "quickstart.py"
        script.write_text(snippets[0] + "\n", encoding="utf-8")
        env = os.environ.copy()
        env.pop("PYTHONPATH", None)
        run([str(python), str(script)], cwd=consumer, env=env)
    print("rendered Python quick start passed: 1")


def run_wasm_node_quick_start(packages: list[Path]) -> None:
    package = single_artifact(packages, "npm package")
    _, snippets = surface_snippets("wasm-node")
    with tempfile.TemporaryDirectory(prefix="pdfplumber-wasm-docs-") as temp:
        consumer = Path(temp)
        (consumer / "package.json").write_text(
            '{"name":"pdfplumber-doc-quickstart","private":true,"type":"module"}\n',
            encoding="utf-8",
        )
        run(
            [
                "npm",
                "install",
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
                str(package),
            ],
            cwd=consumer,
        )
        shutil.copyfile(RUST_FIXTURE, consumer / "document.pdf")
        script = consumer / "quickstart.mjs"
        script.write_text(snippets[0] + "\n", encoding="utf-8")
        run(["node", str(script)], cwd=consumer)
    print("rendered Node/WebAssembly quick start passed: 1")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check", action="store_true", help="validate docs and CI wiring"
    )
    parser.add_argument("--rust", action="store_true", help="execute Rust quick starts")
    parser.add_argument(
        "--cli", action="store_true", help="install and execute the CLI quick start"
    )
    parser.add_argument(
        "--python",
        action="store_true",
        help="install and execute the Python quick start",
    )
    parser.add_argument(
        "--wheel", type=Path, nargs="+", default=[], help="candidate Python wheel"
    )
    parser.add_argument(
        "--wasm-node",
        action="store_true",
        help="install and execute the Node/WASM quick start",
    )
    parser.add_argument(
        "--npm-package",
        type=Path,
        nargs="+",
        default=[],
        help="candidate npm package directory or archive",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not any((args.check, args.rust, args.cli, args.python, args.wasm_node)):
        raise QuickStartError("select --check or at least one executable surface")

    check_static_contract()
    if args.rust:
        run_rust_quick_starts()
    if args.cli:
        run_cli_quick_start()
    if args.python:
        run_python_quick_start(args.wheel)
    if args.wasm_node:
        run_wasm_node_quick_start(args.npm_package)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except QuickStartError as error:
        print(f"documentation quick-start check failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
