"""Exact approved-delta registry and gate contracts (PARITY-016)."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from compat.harness import approved_deltas, upstream


REGISTRY_PATH: Path = upstream.REPO_ROOT / "compat" / "approved_deltas.toml"


class ApprovedDeltaContractTests(unittest.TestCase):
    def test_committed_registry_is_target_bound_and_empty_until_approval(self) -> None:
        registry = approved_deltas.load_registry(REGISTRY_PATH)
        target = upstream.load_target()

        self.assertEqual(registry.version, target.version)
        self.assertEqual(registry.commit, target.commit)
        self.assertEqual(registry.deltas, ())
        approved_deltas.validate_target(registry, target.version, target.commit)
        with self.assertRaisesRegex(
            approved_deltas.DeltaRegistryError,
            "does not match",
        ):
            approved_deltas.validate_target(registry, "0.0.0", target.commit)

    def test_unregistered_and_stale_deltas_fail_the_gate(self) -> None:
        observed = self.observation()
        empty = approved_deltas.Registry(
            version="0.11.10",
            commit="7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
            deltas=(),
        )
        unregistered = approved_deltas.evaluate((observed,), empty)

        self.assertEqual(unregistered.unregistered, (observed,))
        self.assertEqual(unregistered.exit_code, 1)

        entry = self.entry(observed)
        stale = approved_deltas.evaluate(
            (),
            approved_deltas.Registry(empty.version, empty.commit, (entry,)),
        )
        self.assertEqual(stale.stale, (entry,))
        self.assertEqual(stale.exit_code, 1)

    def test_only_an_exact_registered_delta_can_pass(self) -> None:
        observed = self.observation()
        entry = self.entry(observed)
        registry = approved_deltas.Registry(
            version="0.11.10",
            commit="7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
            deltas=(entry,),
        )

        result = approved_deltas.evaluate((observed,), registry)

        self.assertEqual(result.approved, ((observed, entry),))
        self.assertEqual(result.unregistered, ())
        self.assertEqual(result.stale, ())
        self.assertEqual(result.exit_code, 0)

        changed = approved_deltas.ObservedDelta(
            fixture=observed.fixture,
            page=observed.page,
            api=observed.api,
            upstream_sha256=observed.upstream_sha256,
            rust_sha256="f" * 64,
        )
        changed_result = approved_deltas.evaluate((changed,), registry)
        self.assertEqual(changed_result.unregistered, (changed,))
        self.assertEqual(changed_result.stale, (entry,))
        self.assertEqual(changed_result.exit_code, 1)

    def test_value_digest_preserves_runtime_types(self) -> None:
        self.assertNotEqual(
            approved_deltas.value_digest(True),
            approved_deltas.value_digest(1),
        )
        self.assertNotEqual(
            approved_deltas.value_digest(["value"]),
            approved_deltas.value_digest(("value",)),
        )

    def test_value_digest_supports_stable_pdfminer_primitives(self) -> None:
        class PSLiteral:
            def __init__(self, name: str) -> None:
                self.name = name

        class PDFObjRef:
            def __init__(self, objid: int) -> None:
                self.objid = objid

        class PDFStream:
            def __init__(self, rawdata: bytes) -> None:
                self.attrs = {"Type": PSLiteral("XObject"), "BBox": PDFObjRef(103)}
                self.rawdata = rawdata
                self.objid = 168
                self.genno = 0

        class Page:
            page_number = 1
            initial_doctop = 0
            rotation = 0
            mediabox = (0, 0, 612, 792)
            cropbox = (0, 0, 612, 792)
            bbox = (0, 0, 612, 792)

        PSLiteral.__module__ = "pdfminer.psparser"
        PSLiteral.__qualname__ = "PSLiteral"
        PDFObjRef.__module__ = "pdfminer.pdftypes"
        PDFObjRef.__qualname__ = "PDFObjRef"
        PDFStream.__module__ = "pdfminer.pdftypes"
        PDFStream.__qualname__ = "PDFStream"
        Page.__module__ = "pdfplumber.page"
        Page.__qualname__ = "Page"

        first = approved_deltas.value_digest(PDFStream(b"first"))
        self.assertEqual(first, approved_deltas.value_digest(PDFStream(b"first")))
        self.assertNotEqual(first, approved_deltas.value_digest(PDFStream(b"changed")))
        self.assertNotEqual(
            approved_deltas.value_digest(PSLiteral("StrikeOut")),
            approved_deltas.value_digest(PSLiteral("Highlight")),
        )
        self.assertEqual(
            approved_deltas.value_digest(Page()),
            approved_deltas.value_digest(Page()),
        )

    def test_loaded_entry_requires_every_review_and_result_field(self) -> None:
        observed = self.observation()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "approved_deltas.toml"
            content = f'''schema_version = 1

[target]
version = "0.11.10"
commit = "7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62"

[[delta]]
id = "DELTA-001"
fixture = "{observed.fixture}"
page = 1
api = "page_text"
upstream_result = "upstream"
upstream_sha256 = "{observed.upstream_sha256}"
rust_result = "rust"
rust_sha256 = "{observed.rust_sha256}"
technical_reason = "documented semantic difference"
compatibility_risk = "applications observe different text"
approving_maintainer = "developer0hye"
regression_test = "compat.tests.test_approved_deltas"
review_condition = "remove when the implementations agree"
'''
            path.write_text(content, encoding="utf-8")
            registry = approved_deltas.load_registry(path)
            self.assertEqual(registry.deltas, (self.entry(observed),))

            path.write_text(
                content.replace(
                    'approving_maintainer = "developer0hye"\n',
                    "",
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                approved_deltas.DeltaRegistryError,
                "approving_maintainer must be a non-empty string",
            ):
                approved_deltas.load_registry(path)

    @staticmethod
    def observation() -> approved_deltas.ObservedDelta:
        return approved_deltas.ObservedDelta(
            fixture="tests/fixtures/generated/example.pdf",
            page=1,
            api="page_text",
            upstream_sha256=approved_deltas.value_digest("upstream"),
            rust_sha256=approved_deltas.value_digest("rust"),
        )

    @staticmethod
    def entry(
        observed: approved_deltas.ObservedDelta,
    ) -> approved_deltas.ApprovedDelta:
        return approved_deltas.ApprovedDelta(
            identifier="DELTA-001",
            fixture=observed.fixture,
            page=observed.page,
            api=observed.api,
            upstream_result="upstream",
            upstream_sha256=observed.upstream_sha256,
            rust_result="rust",
            rust_sha256=observed.rust_sha256,
            technical_reason="documented semantic difference",
            compatibility_risk="applications observe different text",
            approving_maintainer="developer0hye",
            regression_test="compat.tests.test_approved_deltas",
            review_condition="remove when the implementations agree",
        )


if __name__ == "__main__":
    unittest.main()
