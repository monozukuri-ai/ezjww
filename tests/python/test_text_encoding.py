"""Per-file detection of UTF-8 ANSI strings (Jw_cad under Wine with a UTF-8 locale)."""

from __future__ import annotations

import sys
import tempfile
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


def _unicode_record(text: str) -> bytes:
    units = text.encode("utf-16-le")
    count = len(units) // 2
    assert count < 0xFF
    return b"\xff\xfe\xff" + bytes([count]) + units


def _ansi_record(raw: bytes) -> bytes:
    assert len(raw) < 0xFF
    return bytes([len(raw)]) + raw


def _drawing_bytes(content: str, layer_name: str) -> bytes:
    drawing = ezjww.new()
    drawing.options["layer_groups"][0]["layers"][1]["name"] = layer_name
    drawing.modelspace().add_text(content, (0.0, 0.0), (10.0, 0.0))
    return drawing.to_jww_bytes()


def _as_ansi(data: bytes, replacements: dict[str, bytes]) -> bytes:
    """Replace the Unicode CString records of the given texts with ANSI records."""
    for text, raw in replacements.items():
        unicode = _unicode_record(text)
        assert data.count(unicode) == 1, text
        data = data.replace(unicode, _ansi_record(raw))
    return data


class TextEncodingTests(unittest.TestCase):
    def setUp(self) -> None:
        # The drawing re-reads its source path (audit, DXF), so keep the file alive.
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)

    def _read(self, data: bytes) -> ezjww.Drawing:
        path = Path(self._tmp.name) / f"sample{len(data)}.jww"
        path.write_bytes(data)
        return ezjww.readfile(path)

    def _layer_name(self, drawing: ezjww.Drawing) -> str:
        return drawing.header["layer_groups"][0]["layers"][1]["name"]

    def _content(self, drawing: ezjww.Drawing) -> str:
        texts = [e for e in drawing.jww_document["entities"] if e["type"] == "TEXT"]
        self.assertEqual(len(texts), 1)
        return texts[0]["content"]

    def test_utf8_strings_are_detected_per_file(self):
        data = _as_ansi(
            _drawing_bytes("女子便所", "ダイ"),
            {"女子便所": "女子便所".encode("utf-8"), "ダイ": "ダイ".encode("utf-8")},
        )
        drawing = self._read(data)

        self.assertEqual(drawing.text_encoding, "utf-8")
        self.assertEqual(drawing.jww_document["text_encoding"], "utf-8")
        self.assertEqual(self._content(drawing), "女子便所")
        self.assertEqual(self._layer_name(drawing), "ダイ")
        detected = drawing.jww_document["diagnostics"][0]
        self.assertEqual(detected["code"], ezjww.TEXT_ENCODING_DETECTED)
        self.assertEqual(detected["severity"], "info")
        self.assertEqual(detected["details"]["encoding"], "utf-8")
        self.assertEqual(detected["details"]["non_ascii_strings"], 2)
        self.assertEqual(detected["details"]["utf8_sequences"], 6)
        self.assertGreaterEqual(detected["details"]["cp932_anomalies"], 1)
        audit = drawing.audit()
        self.assertIn(ezjww.TEXT_ENCODING_DETECTED, audit["issue_codes"])
        self.assertFalse(audit["has_issues"])

    def test_cp932_strings_stay_cp932(self):
        data = _as_ansi(
            _drawing_bytes("女子便所", "ダイ"),
            {"女子便所": "女子便所".encode("cp932"), "ダイ": "ダイ".encode("cp932")},
        )
        drawing = self._read(data)

        self.assertEqual(drawing.text_encoding, "cp932")
        self.assertEqual(self._content(drawing), "女子便所")
        self.assertEqual(self._layer_name(drawing), "ダイ")
        codes = [d["code"] for d in drawing.jww_document["diagnostics"]]
        self.assertNotIn(ezjww.TEXT_ENCODING_DETECTED, codes)

    def test_one_coincidental_utf8_string_is_not_enough(self):
        # CP932 `ﾒｰ` is D2 B0, which is also well-formed UTF-8 (one sequence, no anomaly).
        data = _as_ansi(_drawing_bytes("ﾒｰ", "A"), {"ﾒｰ": "ﾒｰ".encode("cp932")})
        drawing = self._read(data)

        self.assertEqual(drawing.text_encoding, "cp932")
        self.assertEqual(self._content(drawing), "ﾒｰ")

    def test_name_cut_inside_a_utf8_sequence_is_trimmed(self):
        cut = "Aダ0_積".encode("utf-8")[:-1]
        data = _as_ansi(
            _drawing_bytes("女子便所", "Aダ0_積"),
            {"女子便所": "女子便所".encode("utf-8"), "Aダ0_積": cut},
        )
        drawing = self._read(data)

        self.assertEqual(drawing.text_encoding, "utf-8")
        self.assertEqual(self._layer_name(drawing), "Aダ0_")
        replaced = [
            d
            for d in drawing.jww_document["diagnostics"]
            if d["code"] == ezjww.CP932_DECODE_REPLACED
        ]
        self.assertEqual(len(replaced), 1)
        self.assertEqual(replaced[0]["details"]["encoding"], "utf-8")
        self.assertEqual(replaced[0]["details"]["replacement_characters"], 1)

    def test_wine_mix_of_cp932_font_and_utf8_text(self):
        # Jw_cad under Wine copies the font name from its CP932 string table
        # while the typed text and names are UTF-8.
        data = _as_ansi(
            _drawing_bytes("女子便所", "ダイ"),
            {
                "女子便所": "女子便所".encode("utf-8"),
                "ダイ": "ダイ".encode("utf-8"),
                "ＭＳ ゴシック": "ＭＳ ゴシック".encode("cp932"),
            },
        )
        drawing = self._read(data)

        self.assertEqual(drawing.text_encoding, "utf-8")
        self.assertEqual(self._content(drawing), "女子便所")
        self.assertEqual(self._layer_name(drawing), "ダイ")
        text = next(e for e in drawing.jww_document["entities"] if e["type"] == "TEXT")
        self.assertEqual(text["font_name"], "ＭＳ ゴシック")
        codes = [d["code"] for d in drawing.jww_document["diagnostics"]]
        self.assertEqual(codes, [ezjww.TEXT_ENCODING_DETECTED])

    def test_unicode_strings_are_not_affected(self):
        drawing = self._read(_drawing_bytes("女子便所", "ダイ"))

        self.assertEqual(drawing.text_encoding, "cp932")
        self.assertEqual(self._content(drawing), "女子便所")
        self.assertEqual(self._layer_name(drawing), "ダイ")


if __name__ == "__main__":
    unittest.main()
