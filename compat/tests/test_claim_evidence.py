from __future__ import annotations

import re
import unittest
from pathlib import Path, PurePosixPath
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[2]
README = ROOT / "README.md"
RELEASE_NOTES = ROOT / "docs" / "releases" / "v0.3.0.md"

MARKDOWN_LINK = re.compile(r"\[[^\]]+\]\((?P<target>[^)\s]+)(?:\s+[^)]*)?\)")
REPOSITORY_BLOB_PREFIX = "/developer0hye/pdfplumber-rs/blob/main/"


def markdown_section(text: str, heading: str) -> str:
    match = re.search(
        rf"^{re.escape(heading)}\n(?P<body>.*?)(?=^##\s|\Z)",
        text,
        re.MULTILINE | re.DOTALL,
    )
    if match is None:
        raise AssertionError(f"missing Markdown section {heading!r}")
    return match.group("body").strip()


def paragraph_containing(text: str, phrase: str) -> str:
    for paragraph in re.split(r"\n\s*\n", text):
        if phrase in paragraph:
            return paragraph
    raise AssertionError(f"missing claim containing {phrase!r}")


def list_items(section: str) -> list[str]:
    items: list[list[str]] = []
    current: list[str] | None = None
    for line in section.splitlines():
        if line.startswith("- "):
            if current is not None:
                items.append(current)
            current = [line]
        elif current is not None and (line.startswith("  ") or not line.strip()):
            current.append(line)
        elif current is not None:
            items.append(current)
            current = None
    if current is not None:
        items.append(current)
    return ["\n".join(item).strip() for item in items]


def table_rows(section: str) -> list[str]:
    return [
        line
        for line in section.splitlines()
        if line.startswith("|")
        and not line.startswith("|---")
        and "| Surface |" not in line
    ]


def repository_target(document: Path, target: str) -> tuple[PurePosixPath, str] | None:
    target_without_fragment, separator, fragment = target.partition("#")
    parsed = urlparse(target_without_fragment)
    if parsed.scheme:
        if parsed.netloc != "github.com" or not parsed.path.startswith(
            REPOSITORY_BLOB_PREFIX
        ):
            return None
        relative = PurePosixPath(parsed.path.removeprefix(REPOSITORY_BLOB_PREFIX))
    else:
        resolved = (document.parent / target_without_fragment).resolve()
        try:
            relative = PurePosixPath(resolved.relative_to(ROOT).as_posix())
        except ValueError:
            return None
    return relative, fragment if separator else ""


def markdown_anchors(path: Path) -> set[str]:
    anchors: set[str] = set()
    for heading in re.findall(
        r"^#{1,6}\s+(?P<heading>.+?)\s*$",
        path.read_text(encoding="utf-8"),
        re.MULTILINE,
    ):
        plain = re.sub(r"[`*_]", "", heading).lower()
        anchors.add(re.sub(r"[^a-z0-9 -]", "", plain).strip().replace(" ", "-"))
    return anchors


def is_allowed_evidence_link(document: Path, target: str) -> bool:
    resolved = repository_target(document, target)
    if resolved is None:
        return False
    relative, fragment = resolved
    path = ROOT / relative
    if not path.is_file():
        return False
    if fragment and fragment not in markdown_anchors(path):
        return False
    if relative == PurePosixPath("docs/support.md"):
        return bool(fragment)
    if relative.parts[:2] == ("docs", "readiness") and relative.suffix == ".md":
        return bool(fragment)
    if (
        relative.parts[:2] == ("compat", "tests")
        and relative.name.startswith("test_")
        and relative.suffix == ".py"
    ):
        return True
    return (
        "benches" in relative.parts
        and relative.name == "README.md"
        and bool(fragment)
    )


def evidence_links(document: Path, block: str) -> list[str]:
    return [
        target
        for target in MARKDOWN_LINK.findall(block)
        if is_allowed_evidence_link(document, target)
    ]


class ProductClaimEvidenceContractTests(unittest.TestCase):
    def assert_claims_have_evidence(
        self, document: Path, claims: list[tuple[str, str]]
    ) -> None:
        failures = [
            label
            for label, claim in claims
            if not evidence_links(document, claim)
        ]
        self.assertEqual(
            [],
            failures,
            "claims without adjacent evidence: " + ", ".join(failures),
        )

    def test_readme_opening_claims_link_stable_repository_evidence(self) -> None:
        text = README.read_text(encoding="utf-8")
        phrases = (
            "Evidence-driven PDF extraction for Rust",
            "Use the Rust crate to extract text",
            "Maturity: `0.3.x` alpha",
            "Release `0.3.0`",
            "Compatibility work is checked against",
        )
        self.assert_claims_have_evidence(
            README,
            [(phrase, paragraph_containing(text, phrase)) for phrase in phrases],
        )

    def test_readme_use_case_feature_wasm_and_msrv_claims_link_evidence(self) -> None:
        text = README.read_text(encoding="utf-8")
        choose = markdown_section(text, "## Choose `pdfplumber-rs` when…")
        features = markdown_section(text, "## Features")
        wasm = markdown_section(text, "## WASM Support")
        msrv = markdown_section(text, "## Minimum Supported Rust Version")
        claims = [
            (f"choose:{index}", item)
            for index, item in enumerate(list_items(choose), start=1)
        ]
        claims.extend(
            (f"feature:{index}", item)
            for index, item in enumerate(list_items(features), start=1)
        )
        claims.extend(
            (
                ("choose:ocr", paragraph_containing(choose, "does not perform")),
                ("wasm", paragraph_containing(wasm, "wasm32-unknown-unknown")),
                ("msrv", paragraph_containing(msrv, "Rust 1.85")),
            )
        )
        self.assert_claims_have_evidence(README, claims)

    def test_release_identity_and_upgrade_claims_link_evidence(self) -> None:
        text = RELEASE_NOTES.read_text(encoding="utf-8")
        opening = text.split("## Who should upgrade?", 1)[0]
        identity_rows = [
            line
            for line in opening.splitlines()
            if line.startswith("|")
            and not line.startswith("|---")
            and "| Surface |" not in line
        ]
        release_metadata = list_items(opening)
        guidance = list_items(markdown_section(text, "## Who should upgrade?"))
        claims = [
            (f"identity:{index}", row)
            for index, row in enumerate(identity_rows, start=1)
        ]
        claims.extend(
            (f"release-metadata:{index}", item)
            for index, item in enumerate(release_metadata, start=1)
        )
        claims.append(
            (
                "prerelease-boundary",
                paragraph_containing(opening, "This GitHub release is a prerelease"),
            )
        )
        claims.extend(
            (f"upgrade:{index}", item)
            for index, item in enumerate(guidance, start=1)
        )
        self.assert_claims_have_evidence(RELEASE_NOTES, claims)

    def test_release_change_limitation_and_artifact_claims_link_evidence(self) -> None:
        text = RELEASE_NOTES.read_text(encoding="utf-8")
        changes = list_items(markdown_section(text, "## Behavior changes"))
        limitations = list_items(markdown_section(text, "## Known limitations"))
        artifact_section = markdown_section(text, "## Artifact matrix")
        artifacts = table_rows(artifact_section)
        claims = [
            (f"change:{index}", item)
            for index, item in enumerate(changes, start=1)
        ]
        claims.extend(
            (f"limitation:{index}", item)
            for index, item in enumerate(limitations, start=1)
        )
        claims.extend(
            (f"artifact:{index}", row)
            for index, row in enumerate(artifacts, start=1)
        )
        self.assert_claims_have_evidence(RELEASE_NOTES, claims)


if __name__ == "__main__":
    unittest.main()
