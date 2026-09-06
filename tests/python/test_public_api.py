from __future__ import annotations

import json
import sys
import tempfile
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src"
try:
    import ezjww
except ModuleNotFoundError:
    if str(SRC) not in sys.path:
        sys.path.insert(0, str(SRC))
    import ezjww


def sample_path() -> Path:
    return ROOT / "jww_samples" / "Test1.jww"


class PublicApiTests(unittest.TestCase):
    def test_is_jww_file_uses_signature_not_extension(self):
        with tempfile.TemporaryDirectory(prefix="ezjww_signature_") as tmp_dir:
            tmp = Path(tmp_dir)
            valid = tmp / "drawing.bin"
            disguised = tmp / "drawing.jww"
            short = tmp / "short.jww"
            valid.write_bytes(b"JwwData.")
            disguised.write_bytes(b"NotJWW!!")
            short.write_bytes(b"Jww")

            self.assertTrue(ezjww.is_jww_file(str(valid)))
            self.assertFalse(ezjww.is_jww_file(str(disguised)))
            self.assertFalse(ezjww.is_jww_file(str(short)))

    def test_read_document_exposes_parser_diagnostics(self):
        document = ezjww.read_document(str(sample_path()))
        self.assertIn("diagnostics", document)
        self.assertIsInstance(document["diagnostics"], list)
        self.assertEqual(document["metadata_settings"], [])

    def test_read_header_exposes_the_complete_screen_palette(self):
        header = ezjww.read_header(str(sample_path()))
        palette = header["palette"]
        self.assertIsNotNone(palette)
        assert palette is not None
        self.assertEqual(len(palette["pen_colors"]), 10)
        self.assertIsNotNone(palette["extended_colors"])
        assert palette["extended_colors"] is not None
        self.assertEqual(len(palette["extended_colors"]), 257)
        self.assertEqual(palette["extended_colors"][1], 0x000000)
        self.assertEqual(palette["extended_colors"][2], 0x0000FF)

    def test_internal_settings_are_exposed_but_not_converted_to_dxf_text(self):
        path = ROOT / "jww_samples" / "block_regressions" / "non_block.jww"

        document = ezjww.read_document(str(path))
        dxf_document = ezjww.read_dxf_document(str(path))

        self.assertEqual(len(document["metadata_settings"]), 6)
        self.assertEqual(
            {setting["key"] for setting in document["metadata_settings"]},
            {
                "Printer_Orientation",
                "Printer_PaperSize",
                "Printer_D2dBMP",
                "Printer_BmpZENTAI",
                "View_Direct2d",
                "Draw_BmpTOUKA",
            },
        )
        converted_text = {
            entity.get("content", "")
            for entity in dxf_document["entities"]
            if entity.get("type") == "TEXT"
        }
        self.assertFalse(
            any(
                setting["raw"] in converted_text
                for setting in document["metadata_settings"]
            )
        )

    def test_invalid_cp932_is_reported_end_to_end(self):
        raw = bytearray(sample_path().read_bytes())
        memo_prefix = "日影図".encode("cp932")
        damaged_offset = raw.find(memo_prefix)
        self.assertGreaterEqual(damaged_offset, 0)
        raw[damaged_offset] = 0xFF

        with tempfile.TemporaryDirectory(prefix="ezjww_cp932_") as tmp_dir:
            damaged = Path(tmp_dir) / "damaged.jww"
            damaged.write_bytes(raw)

            document = ezjww.read_document(str(damaged))
            result = ezjww.audit(str(damaged))

        self.assertEqual(len(document["diagnostics"]), 1)
        diagnostic = document["diagnostics"][0]
        self.assertEqual(diagnostic["code"], "CP932_DECODE_REPLACED")
        self.assertEqual(diagnostic["details"]["field"], "header.memo")
        self.assertEqual(diagnostic["details"]["byte_offset"], damaged_offset)
        self.assertGreaterEqual(
            diagnostic["details"]["replacement_characters"],
            1,
        )
        self.assertIn("CP932_DECODE_REPLACED", result["issue_codes"])
        self.assertEqual(result["decode_error_count"], 1)
        self.assertGreaterEqual(result["decode_replacement_characters"], 1)

    def test_readfile_modelspace_query(self):
        drawing = ezjww.readfile(sample_path())
        msp = drawing.modelspace()
        self.assertGreater(len(msp), 0)
        lines = msp.query("LINE")
        self.assertGreater(len(lines), 0)
        self.assertTrue(all(e.get("type") == "LINE" for e in lines))

    def test_modelspace_query_selector_multi_type(self):
        drawing = ezjww.readfile(sample_path())
        msp = drawing.modelspace()
        selected = msp.query("LINE POINT")
        self.assertGreater(len(selected), 0)
        self.assertTrue(all(e.get("type") in {"LINE", "POINT"} for e in selected))
        self.assertEqual(len(selected), len(msp.query("LINE")) + len(msp.query("POINT")))

    def test_modelspace_query_selector_filters(self):
        drawing = ezjww.readfile(sample_path())
        msp = drawing.modelspace()
        # Pen colors resolve through the screen color palette in the file header,
        # and the black ink this sample draws with lands on ACI 7.
        with_filters = msp.query('LINE[layer=="#lv4", color==7]')
        self.assertGreater(len(with_filters), 0)
        self.assertTrue(all(e.get("type") == "LINE" for e in with_filters))
        self.assertTrue(all(e.get("layer") == "#lv4" for e in with_filters))
        self.assertTrue(all(e.get("color") == 7 for e in with_filters))

        # The color predicate is really applied: the layer carries no red line.
        self.assertEqual(len(msp.query('LINE[layer=="#lv4", color==1]')), 0)

        all_on_layer = msp.query('[layer=="#lv4"]')
        self.assertGreaterEqual(len(all_on_layer), len(with_filters))
        self.assertTrue(all(e.get("layer") == "#lv4" for e in all_on_layer))

    def test_modelspace_query_rejects_invalid_selector(self):
        drawing = ezjww.readfile(sample_path())
        msp = drawing.modelspace()
        with self.assertRaises(ValueError):
            msp.query("LINE[layer~=5]")

    def test_new_drawing_defaults(self):
        drawing = ezjww.new()
        self.assertEqual(len(drawing.modelspace()), 0)
        audit = drawing.audit()
        self.assertFalse(audit["has_issues"])
        self.assertEqual(audit["unsupported_entities"], [])
        self.assertEqual(audit["unresolved_def_numbers"], [])

    def test_audit_from_path(self):
        result = ezjww.audit(sample_path())
        self.assertIn("has_issues", result)
        self.assertIn("issue_codes", result)
        self.assertIn("unresolved_count", result)
        self.assertIn("unsupported_count", result)
        self.assertIn("unsupported_entities", result)
        self.assertIn("unresolved_def_numbers", result)
        self.assertIn("diagnostics", result)
        self.assertIn("decode_error_count", result)
        self.assertIn("decode_replacement_characters", result)
        self.assertIn("decode_affected_fields", result)
        self.assertEqual(result["unresolved_def_numbers"], [])
        self.assertEqual(result["issue_codes"], [])
        self.assertEqual(result["unresolved_count"], 0)
        self.assertEqual(result["unsupported_count"], 0)
        self.assertEqual(result["decode_error_count"], 0)
        self.assertEqual(result["decode_replacement_characters"], 0)
        self.assertEqual(result["decode_affected_fields"], [])
        self.assertEqual(result["diagnostics"], [])

    def test_audit_includes_structured_cp932_parser_diagnostic(self):
        parser_diagnostic = {
            "code": "CP932_DECODE_REPLACED",
            "severity": "warning",
            "message": "CP932 decoding replaced 1 sequence.",
            "action": "normalized",
            "details": {
                "encoding": "cp932",
                "field": "entity.text.content",
                "byte_offset": 120,
                "byte_length": 2,
                "replacement_characters": 1,
                "had_errors": True,
            },
        }
        drawing = ezjww.Drawing(
            source_path="damaged.jww",
            jww_document={
                "validation": {
                    "total_references": 0,
                    "resolved_references": 0,
                    "unresolved_def_numbers": [],
                },
                "diagnostics": [parser_diagnostic],
            },
            dxf_document={
                "layers": [],
                "entities": [],
                "blocks": [],
                "unsupported_entities": [],
            },
        )

        result = drawing.audit()

        self.assertTrue(result["has_issues"])
        self.assertEqual(result["issue_codes"], ["CP932_DECODE_REPLACED"])
        self.assertEqual(result["diagnostics"], [parser_diagnostic])
        self.assertEqual(result["decode_error_count"], 1)
        self.assertEqual(result["decode_replacement_characters"], 1)
        self.assertEqual(result["decode_affected_fields"], ["entity.text.content"])

    def test_bbox_from_path(self):
        result = ezjww.bbox(sample_path())
        self.assertIsNotNone(result)
        assert result is not None
        self.assertIn("min_x", result)
        self.assertIn("min_y", result)
        self.assertIn("max_x", result)
        self.assertIn("max_y", result)
        self.assertIn("width", result)
        self.assertIn("height", result)
        self.assertIn("entity_count", result)
        self.assertGreater(result["width"], 0.0)
        self.assertGreater(result["height"], 0.0)
        self.assertGreater(result["entity_count"], 0)

    def test_bbox_for_new_drawing_is_none(self):
        drawing = ezjww.new()
        self.assertIsNone(drawing.bbox())

    def test_stats_from_path(self):
        result = ezjww.stats(sample_path())
        self.assertIn("entity_count", result)
        self.assertIn("type_count", result)
        self.assertIn("layer_count", result)
        self.assertIn("color_count", result)
        self.assertIn("by_type", result)
        self.assertIn("by_layer", result)
        self.assertIn("by_color", result)
        self.assertGreater(result["entity_count"], 0)
        self.assertIn("LINE", result["by_type"])

    def test_modelspace_stats_for_subset(self):
        drawing = ezjww.readfile(sample_path())
        lines = drawing.modelspace().query("LINE")
        result = ezjww.Modelspace(lines).stats()
        self.assertEqual(set(result["by_type"].keys()), {"LINE"})
        self.assertEqual(result["entity_count"], len(lines))

    def test_report_from_path(self):
        result = ezjww.report(sample_path())
        self.assertIn("source_path", result)
        self.assertIn("explode_inserts", result)
        self.assertIn("max_block_nesting", result)
        self.assertIn("audit", result)
        self.assertIn("bbox", result)
        self.assertIn("stats", result)
        self.assertIn("has_issues", result["audit"])
        self.assertIn("entity_count", result["stats"])

    def test_to_dxf_string_from_path(self):
        text = ezjww.to_dxf_string(sample_path())
        self.assertIn("SECTION", text)
        self.assertIn("  9\n$ACADVER\n  1\nAC1015\n", text)
        self.assertTrue(text.endswith("  0\nEOF\n"))

    def test_write_dxf_with_report_targets_ac1024(self):
        with tempfile.TemporaryDirectory(prefix="ezjww_report_") as tmp_dir:
            output = Path(tmp_dir) / "output.dxf"
            report = ezjww.write_dxf_with_report(
                str(sample_path()),
                str(output),
                target_version="AC1024",
            )
            text = output.read_text(encoding="ascii")

        self.assertIn("  9\n$ACADVER\n  1\nAC1024\n", text)
        self.assertEqual(report["target_version"], "AC1024")
        self.assertGreater(report["source_entities"], 0)
        self.assertGreater(report["converted_entities"], 0)
        self.assertEqual(report["diagnostics"], [])
        self.assertEqual(report["unsupported_entity_counts"], {})
        self.assertFalse(report["validation"]["has_unresolved"])

    def test_dxf_writers_reject_unsupported_target_version(self):
        with self.assertRaisesRegex(ValueError, "expected AC1015 or AC1024"):
            ezjww.to_dxf_string(sample_path(), target_version="AC1009")

    def test_drawing_to_dxf_string_with_options(self):
        drawing = ezjww.readfile(sample_path())
        text = drawing.to_dxf_string(explode_inserts=True, max_block_nesting=16)
        self.assertIn("ENTITIES", text)
        self.assertTrue(text.endswith("  0\nEOF\n"))

    def test_drawing_to_dxf_string_rejects_new_drawing(self):
        drawing = ezjww.new()
        with self.assertRaises(ValueError):
            drawing.to_dxf_string()

    def test_to_dxf_accepts_explode_options(self):
        drawing = ezjww.readfile(sample_path())
        regular = drawing.to_dxf()
        exploded = drawing.to_dxf(explode_inserts=True, max_block_nesting=16)
        self.assertIn("entities", regular)
        self.assertIn("entities", exploded)

    def test_to_dxf_rejects_invalid_max_block_nesting(self):
        drawing = ezjww.readfile(sample_path())
        with self.assertRaises(ValueError):
            drawing.to_dxf(explode_inserts=True, max_block_nesting=0)

    def test_cli_to_dxf_report_json(self):
        with tempfile.TemporaryDirectory(prefix="ezjww_test_") as tmp_dir:
            tmp = Path(tmp_dir)
            dxf_out = tmp / "out.dxf"
            report_out = tmp / "report.json"
            code = ezjww._run(
                [
                    "to-dxf",
                    str(sample_path()),
                    "-o",
                    str(dxf_out),
                    "--report",
                    "json",
                    "--report-path",
                    str(report_out),
                ]
            )
            self.assertEqual(code, 0)
            self.assertTrue(dxf_out.exists())
            self.assertTrue(report_out.exists())
            report = json.loads(report_out.read_text(encoding="utf-8"))
            self.assertTrue(report["ok"])
            self.assertIn("audit", report)
            self.assertIn("explode_inserts", report)
            self.assertIn("max_block_nesting", report)
            self.assertEqual(report["text_em_scale"], 1.0)

    def test_print_json_fallback_for_non_utf8_stdout(self):
        class _AsciiStdout:
            def __init__(self):
                self.parts: list[str] = []

            def write(self, text: str) -> int:
                text.encode("ascii")
                self.parts.append(text)
                return len(text)

            def flush(self) -> None:
                return None

            def getvalue(self) -> str:
                return "".join(self.parts)

        fake_stdout = _AsciiStdout()
        with patch("sys.stdout", new=fake_stdout):
            ezjww._print_json({"text": "日本語"})

        parsed = json.loads(fake_stdout.getvalue())
        self.assertEqual(parsed["text"], "日本語")

    def test_cli_audit_json(self):
        buf = StringIO()
        with patch("sys.stdout", new=buf):
            code = ezjww._run(["audit", str(sample_path()), "--json"])
        self.assertEqual(code, 0)
        out = json.loads(buf.getvalue())
        self.assertIn("has_issues", out)
        self.assertIn("issue_codes", out)
        self.assertIn("unsupported_count", out)
        self.assertIn("unresolved_count", out)
        self.assertFalse(out["has_issues"])

    def test_cli_audit_fail_on_issues(self):
        fake = {
            "source_path": "dummy.jww",
            "total_references": 1,
            "resolved_references": 0,
            "unresolved_def_numbers": [10],
            "unresolved_count": 1,
            "unsupported_entities": [],
            "unsupported_count": 0,
            "issue_codes": ["UNRESOLVED_BLOCK_REFERENCES"],
            "has_issues": True,
            "warnings": ["unresolved block references detected"],
        }
        with patch.object(ezjww, "audit", return_value=fake):
            code = ezjww._run(["audit", str(sample_path()), "--fail-on-issues"])
        self.assertEqual(code, 3)

    def test_cli_audit_rejects_invalid_max_block_nesting(self):
        code = ezjww._run(["audit", str(sample_path()), "--max-block-nesting", "0"])
        self.assertEqual(code, 2)

    def test_cli_bbox_json(self):
        buf = StringIO()
        with patch("sys.stdout", new=buf):
            code = ezjww._run(["bbox", str(sample_path()), "--json", "--explode-inserts"])
        self.assertEqual(code, 0)
        out = json.loads(buf.getvalue())
        self.assertIn("min_x", out)
        self.assertIn("min_y", out)
        self.assertIn("max_x", out)
        self.assertIn("max_y", out)
        self.assertIn("width", out)
        self.assertIn("height", out)
        self.assertIn("entity_count", out)

    def test_cli_bbox_rejects_invalid_max_block_nesting(self):
        code = ezjww._run(["bbox", str(sample_path()), "--max-block-nesting", "0"])
        self.assertEqual(code, 2)

    def test_cli_stats_json(self):
        buf = StringIO()
        with patch("sys.stdout", new=buf):
            code = ezjww._run(["stats", str(sample_path()), "--json"])
        self.assertEqual(code, 0)
        out = json.loads(buf.getvalue())
        self.assertIn("entity_count", out)
        self.assertIn("by_type", out)
        self.assertIn("by_layer", out)
        self.assertIn("by_color", out)
        self.assertGreater(out["entity_count"], 0)

    def test_cli_stats_rejects_invalid_max_block_nesting(self):
        code = ezjww._run(["stats", str(sample_path()), "--max-block-nesting", "0"])
        self.assertEqual(code, 2)

    def test_cli_report_json(self):
        buf = StringIO()
        with patch("sys.stdout", new=buf):
            code = ezjww._run(["report", str(sample_path()), "--json"])
        self.assertEqual(code, 0)
        out = json.loads(buf.getvalue())
        self.assertIn("audit", out)
        self.assertIn("bbox", out)
        self.assertIn("stats", out)
        self.assertIn("has_issues", out["audit"])

    def test_cli_report_fail_on_issues(self):
        fake = {
            "source_path": "dummy.jww",
            "explode_inserts": False,
            "max_block_nesting": 32,
            "audit": {"has_issues": True},
            "bbox": None,
            "stats": {"entity_count": 0},
        }
        with patch.object(ezjww, "report", return_value=fake):
            code = ezjww._run(["report", str(sample_path()), "--fail-on-issues"])
        self.assertEqual(code, 3)

    def test_cli_report_rejects_invalid_max_block_nesting(self):
        code = ezjww._run(["report", str(sample_path()), "--max-block-nesting", "0"])
        self.assertEqual(code, 2)

    def test_parse_plot_figsize_accepts_comma_or_x_separator(self):
        self.assertEqual(ezjww._parse_figsize("16,11"), (16.0, 11.0))
        self.assertEqual(ezjww._parse_figsize("16x11"), (16.0, 11.0))
        with self.assertRaises(ValueError):
            ezjww._parse_figsize("16")
        with self.assertRaises(ValueError):
            ezjww._parse_figsize("16,0")

    def test_normalize_plot_linewidth_rejects_non_positive_values(self):
        self.assertEqual(ezjww._normalize_plot_linewidth(0.18), 0.18)
        with self.assertRaises(ValueError):
            ezjww._normalize_plot_linewidth(0.0)

    def test_normalize_plot_point_size_rejects_negative_values(self):
        self.assertEqual(ezjww._normalize_plot_point_size(4.0), 4.0)
        self.assertEqual(ezjww._normalize_plot_point_size(0.0), 0.0)
        with self.assertRaises(ValueError):
            ezjww._normalize_plot_point_size(-1.0)

    def test_cli_to_dxf_rejects_invalid_max_block_nesting(self):
        with tempfile.TemporaryDirectory(prefix="ezjww_test_") as tmp_dir:
            tmp = Path(tmp_dir)
            dxf_out = tmp / "out.dxf"
            code = ezjww._run(
                [
                    "to-dxf",
                    str(sample_path()),
                    "-o",
                    str(dxf_out),
                    "--max-block-nesting",
                    "0",
                ]
            )
            self.assertEqual(code, 2)
            self.assertFalse(dxf_out.exists())

    def test_cli_to_dxf_dir_report_json(self):
        with tempfile.TemporaryDirectory(prefix="ezjww_test_") as tmp_dir:
            tmp = Path(tmp_dir)
            out_dir = tmp / "dxf"
            report_out = tmp / "dir_report.json"
            code = ezjww._run(
                [
                    "to-dxf-dir",
                    str(ROOT / "jww_samples"),
                    "-o",
                    str(out_dir),
                    "--report",
                    "json",
                    "--report-path",
                    str(report_out),
                ]
            )
            self.assertEqual(code, 0)
            self.assertTrue(report_out.exists())
            report = json.loads(report_out.read_text(encoding="utf-8"))
            self.assertEqual(report["failed"], 0)
            self.assertGreater(report["converted"], 0)
            self.assertEqual(len(report["items"]), report["converted"])
            self.assertIn("explode_inserts", report)
            self.assertIn("max_block_nesting", report)
            self.assertEqual(report["text_em_scale"], 1.0)

    def test_dxf_text_carries_the_jww_width_correction(self):
        entities = ezjww.read_dxf_document(str(sample_path()))["entities"]
        texts = [e for e in entities if e["type"] == "TEXT"]
        self.assertTrue(texts, "sample has no TEXT to check")
        for text in texts:
            self.assertIn("width_factor", text)
            self.assertGreater(text["width_factor"], 0.0)

    def test_text_em_scale_moves_only_the_dxf_height(self):
        # group 41 carries the JWW pitch; only group 40 may move.
        inflating = 1.364
        spec = ezjww.read_dxf_document(str(sample_path()))["entities"]
        scaled = ezjww.read_dxf_document(
            str(sample_path()),
            text_em_scale=inflating,
        )["entities"]

        spec_texts = [e for e in spec if e["type"] == "TEXT"]
        scaled_texts = [e for e in scaled if e["type"] == "TEXT"]
        self.assertEqual(len(spec_texts), len(scaled_texts))
        self.assertTrue(spec_texts, "sample has no TEXT to check")
        for plain, corrected in zip(spec_texts, scaled_texts):
            self.assertAlmostEqual(plain["width_factor"], corrected["width_factor"])
            self.assertAlmostEqual(
                plain["height"] / inflating,
                corrected["height"],
            )

    def test_text_em_scale_rejects_a_value_that_cannot_divide(self):
        path = str(sample_path())
        for scale in (0.0, -1.0, float("nan"), float("inf")):
            with self.assertRaises(ValueError, msg=scale) as caught:
                ezjww.read_dxf_document(path, text_em_scale=scale)
            self.assertIn("text_em_scale", str(caught.exception))
            # The wrappers reject it before reaching the extension.
            with self.assertRaises(ValueError, msg=scale):
                ezjww.readfile(path).to_dxf(text_em_scale=scale)
            with self.assertRaises(ValueError, msg=scale):
                ezjww.to_dxf_string(path, text_em_scale=scale)

    def test_modelspace_reflects_the_text_em_scale(self):
        drawing = ezjww.readfile(sample_path())
        inflating = 1.364
        spec = drawing.modelspace().query("TEXT")
        scaled = drawing.modelspace(text_em_scale=inflating).query("TEXT")

        self.assertTrue(spec, "sample has no TEXT to check")
        self.assertEqual(len(spec), len(scaled))
        for plain, corrected in zip(spec, scaled):
            self.assertAlmostEqual(plain["height"] / inflating, corrected["height"])
            self.assertAlmostEqual(plain["width_factor"], corrected["width_factor"])

        with self.assertRaises(ValueError):
            drawing.modelspace(text_em_scale=0.0)

    def test_analysis_results_do_not_move_with_the_text_em_scale(self):
        # Why the flag is wired into modelspace/to-dxf only: nothing an analysis reads back depends on group 40,
        # so offering the option there would be noise.
        drawing = ezjww.readfile(sample_path())
        inflating = 1.364
        for explode in (False, True):
            spec = drawing.to_dxf(explode_inserts=explode)
            scaled = drawing.to_dxf(explode_inserts=explode, text_em_scale=inflating)
            self.assertEqual(
                ezjww._dxf_bbox(spec),
                ezjww._dxf_bbox(scaled),
                msg=f"explode_inserts={explode}",
            )
            self.assertEqual(
                ezjww._dxf_stats(spec),
                ezjww._dxf_stats(scaled),
                msg=f"explode_inserts={explode}",
            )
            self.assertEqual(
                spec["unsupported_entities"],
                scaled["unsupported_entities"],
                msg=f"explode_inserts={explode}",
            )

    def test_cli_to_dxf_applies_and_reports_the_text_em_scale(self):
        with tempfile.TemporaryDirectory(prefix="ezjww_test_") as tmp_dir:
            tmp = Path(tmp_dir)
            spec_out = tmp / "spec.dxf"
            scaled_out = tmp / "scaled.dxf"
            report_out = tmp / "report.json"

            self.assertEqual(
                ezjww._run(["to-dxf", str(sample_path()), "-o", str(spec_out)]),
                0,
            )
            code = ezjww._run(
                [
                    "to-dxf",
                    str(sample_path()),
                    "-o",
                    str(scaled_out),
                    "--text-em-scale",
                    "1.364",
                    "--report",
                    "json",
                    "--report-path",
                    str(report_out),
                ]
            )
            self.assertEqual(code, 0)
            report = json.loads(report_out.read_text(encoding="utf-8"))
            self.assertAlmostEqual(report["text_em_scale"], 1.364)
            # The flag has to reach the writer, not just the report.
            self.assertNotEqual(
                spec_out.read_text(encoding="utf-8"),
                scaled_out.read_text(encoding="utf-8"),
            )

    def test_cli_to_dxf_rejects_an_unusable_text_em_scale(self):
        with tempfile.TemporaryDirectory(prefix="ezjww_test_") as tmp_dir:
            dxf_out = Path(tmp_dir) / "out.dxf"
            for scale in ("0", "-1", "nan"):
                code = ezjww._run(
                    [
                        "to-dxf",
                        str(sample_path()),
                        "-o",
                        str(dxf_out),
                        "--text-em-scale",
                        scale,
                    ]
                )
                self.assertEqual(code, 2, msg=scale)
                self.assertFalse(dxf_out.exists(), msg=scale)

    def test_cli_to_dxf_dir_reports_the_text_em_scale(self):
        with tempfile.TemporaryDirectory(prefix="ezjww_test_") as tmp_dir:
            tmp = Path(tmp_dir)
            report_out = tmp / "dir_report.json"
            code = ezjww._run(
                [
                    "to-dxf-dir",
                    str(ROOT / "jww_samples"),
                    "-o",
                    str(tmp / "dxf"),
                    "--text-em-scale",
                    "1.364",
                    "--report",
                    "json",
                    "--report-path",
                    str(report_out),
                ]
            )
            self.assertEqual(code, 0)
            report = json.loads(report_out.read_text(encoding="utf-8"))
            self.assertAlmostEqual(report["text_em_scale"], 1.364)
            self.assertTrue(report["items"])
            for item in report["items"]:
                self.assertAlmostEqual(item["text_em_scale"], 1.364)

    def test_cli_to_dxf_dir_rejects_an_unusable_text_em_scale(self):
        with tempfile.TemporaryDirectory(prefix="ezjww_test_") as tmp_dir:
            tmp = Path(tmp_dir)
            out_dir = tmp / "dxf"
            code = ezjww._run(
                [
                    "to-dxf-dir",
                    str(ROOT / "jww_samples"),
                    "-o",
                    str(out_dir),
                    "--text-em-scale",
                    "0",
                ]
            )
            self.assertEqual(code, 2)
            self.assertFalse(out_dir.exists())

    def test_rejecting_a_scale_leaves_no_cache_entry(self):
        drawing = ezjww.readfile(str(sample_path()))
        for scale in (float("nan"), 0.0):
            with self.assertRaises(ValueError):
                drawing.to_dxf(text_em_scale=scale)
        self.assertEqual(len(drawing._dxf_cache), 0)
        drawing.to_dxf()
        drawing.to_dxf()
        self.assertEqual(len(drawing._dxf_cache), 1)


if __name__ == "__main__":
    unittest.main()
