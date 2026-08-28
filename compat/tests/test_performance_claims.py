from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def public_claim_surfaces() -> list[Path]:
    paths = [ROOT / "README.md"]
    paths.extend(sorted(ROOT.glob("crates/*/README.md")))
    paths.extend(sorted(ROOT.glob("crates/*/benches/README.md")))
    paths.extend(sorted(ROOT.glob("docs/*.md")))
    paths.extend(sorted(ROOT.glob("docs/releases/*.md")))
    return paths


PROHIBITED_CLAIMS = {
    "unqualified high-performance claim": re.compile(
        r"\bhigh[- ]performance\b", re.IGNORECASE
    ),
    "unqualified near-native claim": re.compile(r"\bnear[- ]native\b", re.IGNORECASE),
    "cross-project speedup claim": re.compile(r"\bspeedups?\b", re.IGNORECASE),
    "unqualified faster claim": re.compile(r"\bfaster\b", re.IGNORECASE),
    "unqualified memory-efficiency claim": re.compile(
        r"\bmemory[- ]efficient\b", re.IGNORECASE
    ),
    "unqualified lower-memory claim": re.compile(
        r"\blower memory footprint\b", re.IGNORECASE
    ),
    "language-implies-performance claim": re.compile(
        r"\bzero-cost abstractions\b|\bcompiled performance\b", re.IGNORECASE
    ),
    "unreproducible benchmark attribution": re.compile(
        r"\brepresentative baselines\b|\bcommunity benchmarks confirm\b|\bbenchmarks show\b",
        re.IGNORECASE,
    ),
    "approximate multiplier claim": re.compile(r"~\s*\d+(?:\.\d+)?x\b", re.IGNORECASE),
}


class PerformanceClaimContractTests(unittest.TestCase):
    def test_public_surfaces_have_no_unverified_broad_performance_claims(self) -> None:
        failures: list[str] = []
        for path in public_claim_surfaces():
            text = path.read_text(encoding="utf-8")
            for label, pattern in PROHIBITED_CLAIMS.items():
                for match in pattern.finditer(text):
                    line = text.count("\n", 0, match.start()) + 1
                    failures.append(
                        f"{path.relative_to(ROOT)}:{line}: {label}: {match.group(0)!r}"
                    )

        self.assertEqual([], failures, "\n".join(failures))

    def test_package_and_benchmark_readmes_defer_to_the_score_contract(self) -> None:
        readmes = {
            ROOT / "crates/pdfplumber-py/README.md": (
                "../../docs/comparison.md",
                "../../docs/benchmarks/corpus-v0.3.0.md",
            ),
            ROOT / "crates/pdfplumber-wasm/README.md": (
                "../../docs/comparison.md",
                "../../docs/benchmarks/corpus-v0.3.0.md",
            ),
            ROOT / "crates/pdfplumber/benches/README.md": (
                "../../../docs/comparison.md",
                "../../../docs/benchmarks/corpus-v0.3.0.md",
            ),
        }

        for path, (comparison_link, corpus_link) in readmes.items():
            with self.subTest(path=path.relative_to(ROOT)):
                text = path.read_text(encoding="utf-8")
                self.assertIn(comparison_link, text)
                self.assertIn(corpus_link, text)
                self.assertIn("SCORE-002", text)
                self.assertIn("SCORE-009", text)

        comparison = (ROOT / "docs/comparison.md").read_text(encoding="utf-8")
        self.assertIn(
            "No cross-project performance result is currently claimed by `pdfplumber-rs`.",
            comparison,
        )


if __name__ == "__main__":
    unittest.main()
