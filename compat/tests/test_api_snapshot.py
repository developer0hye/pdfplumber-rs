"""Contract for the complete pinned-upstream API snapshot (PARITY-005)."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from compat.harness import lockfile, upstream


SNAPSHOT_PATH: Path = (
    upstream.REPO_ROOT
    / "compat"
    / "snapshots"
    / "pdfplumber-v0.11.10-api.json"
)
EXPECTED_MODULES: set[str] = {
    "pdfplumber",
    "pdfplumber._typing",
    "pdfplumber._version",
    "pdfplumber.cli",
    "pdfplumber.container",
    "pdfplumber.convert",
    "pdfplumber.ctm",
    "pdfplumber.display",
    "pdfplumber.page",
    "pdfplumber.pdf",
    "pdfplumber.repair",
    "pdfplumber.structure",
    "pdfplumber.table",
    "pdfplumber.utils",
    "pdfplumber.utils.clustering",
    "pdfplumber.utils.exceptions",
    "pdfplumber.utils.generic",
    "pdfplumber.utils.geometry",
    "pdfplumber.utils.pdfinternals",
    "pdfplumber.utils.text",
}


class CompleteApiSnapshotTest(unittest.TestCase):
    def test_complete_public_surface_is_committed(self) -> None:
        self.assertTrue(
            SNAPSHOT_PATH.is_file(),
            f"missing pinned API snapshot: {SNAPSHOT_PATH.relative_to(upstream.REPO_ROOT)}",
        )
        snapshot: dict[str, object] = json.loads(
            SNAPSHOT_PATH.read_text(encoding="utf-8")
        )

        target: upstream.Target = upstream.load_target()
        self.assertEqual(snapshot["schema_version"], 1)
        self.assertEqual(
            snapshot["target"],
            {
                "project": target.project,
                "version": target.version,
                "tag": target.tag,
                "commit": target.commit,
                "repository": target.repository,
            },
        )

        environment: dict[str, object] = snapshot["environment"]  # type: ignore[assignment]
        self.assertEqual(environment["lockfile_sha256"], lockfile.digest())
        self.assertTrue(str(environment["python_version"]).startswith("3.13."))

        modules: dict[str, dict[str, object]] = snapshot["modules"]  # type: ignore[assignment]
        self.assertEqual(set(modules), EXPECTED_MODULES)
        self.assertEqual(
            modules["pdfplumber"]["all"],
            [
                "__version__",
                "utils",
                "pdfminer",
                "open",
                "repair",
                "set_debug",
            ],
        )

        for module_name, module in modules.items():
            exports: dict[str, dict[str, object]] = module["exports"]  # type: ignore[assignment]
            self.assertTrue(exports, f"{module_name} has no recorded exports")
            declared_all: object = module["all"]
            if declared_all is not None:
                self.assertTrue(
                    set(declared_all).issubset(exports),  # type: ignore[arg-type]
                    f"{module_name}.__all__ is not fully represented",
                )

            for export_name, exported in exports.items():
                self.assertIn("kind", exported, f"{module_name}.{export_name}")
                defined_in: object = exported.get("defined_in")
                if not isinstance(defined_in, str) or not defined_in.startswith(
                    "pdfplumber"
                ):
                    continue
                if exported["kind"] in {"function", "method", "class"}:
                    self.assertTrue(
                        exported.get("signature"),
                        f"missing signature for {module_name}.{export_name}",
                    )
                if exported["kind"] == "class" and defined_in == module_name:
                    self.assertIsInstance(
                        exported.get("members"),
                        dict,
                        f"missing class surface for {module_name}.{export_name}",
                    )

        self.assertEqual(
            self._class_member(modules, "pdfplumber.pdf", "PDF", "open")["kind"],
            "classmethod",
        )
        self.assertEqual(
            self._class_member(modules, "pdfplumber.pdf", "PDF", "pages")["kind"],
            "property",
        )
        self.assertEqual(
            self._class_member(modules, "pdfplumber.page", "Page", "chars")["kind"],
            "property",
        )
        for method_name in ("extract_words", "extract_text", "find_tables"):
            member: dict[str, object] = self._class_member(
                modules, "pdfplumber.page", "Page", method_name
            )
            self.assertEqual(member["kind"], "method")
            self.assertTrue(member["signature"])

        table_settings: dict[str, object] = modules["pdfplumber.table"]["exports"][  # type: ignore[index]
            "TableSettings"
        ]
        self.assertEqual(table_settings["kind"], "class")
        self.assertIn("vertical_strategy", str(table_settings["signature"]))
        self.assertEqual(
            self._class_member(modules, "pdfplumber.ctm", "CTM", "skew_x")["kind"],
            "property",
        )

        cluster_list: dict[str, object] = modules["pdfplumber.utils"]["exports"][  # type: ignore[index]
            "cluster_list"
        ]
        self.assertEqual(cluster_list["kind"], "function")
        self.assertIn("tolerance", str(cluster_list["signature"]))

        serialized: str = json.dumps(snapshot, sort_keys=True)
        self.assertNotIn(str(upstream.REPO_ROOT), serialized)
        self.assertNotIn("0x", serialized, "snapshot contains an unstable memory address")

    @staticmethod
    def _class_member(
        modules: dict[str, dict[str, object]],
        module_name: str,
        class_name: str,
        member_name: str,
    ) -> dict[str, object]:
        exported: dict[str, object] = modules[module_name]["exports"][class_name]  # type: ignore[index]
        members: dict[str, dict[str, object]] = exported["members"]  # type: ignore[assignment]
        return members[member_name]


if __name__ == "__main__":
    unittest.main()
