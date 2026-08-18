from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src"
try:
    import ezjww
except ModuleNotFoundError:
    if str(SRC) not in sys.path:
        sys.path.insert(0, str(SRC))
    import ezjww


class DiagnosticsCatalogTests(unittest.TestCase):
    def test_catalog_is_sorted_unique_and_complete(self):
        expected = {
            "CP932_DECODE_REPLACED",
            "ENTITY_LIST_TRUNCATED",
            "UNRESOLVED_BLOCK_REFERENCES",
            "UNSUPPORTED_DXF_ENTITIES",
        }

        self.assertEqual(tuple(sorted(set(ezjww.ALL_ISSUE_CODES))), ezjww.ALL_ISSUE_CODES)
        self.assertEqual(set(ezjww.ALL_ISSUE_CODES), expected)
        self.assertEqual(set(ezjww.ISSUE_CODES), expected)

    def test_catalog_details_are_json_friendly(self):
        for code in ezjww.ALL_ISSUE_CODES:
            details = ezjww.issue_code_details(code)
            self.assertEqual(details["code"], code)
            self.assertIn(details["severity"], {"info", "warning", "error"})
            self.assertIn("action", details)
            self.assertTrue(details["area"])
            self.assertTrue(details["condition"])

    def test_document_lists_every_public_code(self):
        text = (ROOT / "docs" / "DIAGNOSTICS.md").read_text(encoding="utf-8")

        for code in ezjww.ALL_ISSUE_CODES:
            self.assertIn(f"`{code}`", text)

    def test_signature_document_publishes_exact_bytes(self):
        text = (ROOT / "docs" / "JWW_SIGNATURE.md").read_text(encoding="utf-8")

        self.assertIn("`JwwData.`", text)
        self.assertIn("`4A 77 77 44 61 74 61 2E`", text)
        self.assertIn("byte offset `0`", text)


if __name__ == "__main__":
    unittest.main()
