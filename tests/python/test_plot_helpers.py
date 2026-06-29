from __future__ import annotations

import importlib.util
import math
import unittest
from pathlib import Path


def load_plot_module():
    root = Path(__file__).resolve().parents[2]
    module_path = root / "src" / "ezjww" / "plot.py"
    spec = importlib.util.spec_from_file_location("ezjww_plot_module", module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"failed to load module from {module_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PLOT = load_plot_module()


class PlotHelperTests(unittest.TestCase):
    def test_normalize_layer_filter(self):
        self.assertIsNone(PLOT._normalize_layer_filter(None))
        self.assertEqual(PLOT._normalize_layer_filter([" A ", "", "B"]), {"A", "B"})
        self.assertIsNone(PLOT._normalize_layer_filter([" ", ""]))

    def test_entity_color_can_render_monochrome(self):
        self.assertEqual(PLOT._entity_color(1, monochrome=False), "#ff0000")
        self.assertEqual(PLOT._entity_color(1, monochrome=True), "#000000")
        self.assertEqual(PLOT._entity_color(5, monochrome=True), "#000000")

    def test_line_style_mapping(self):
        self.assertEqual(PLOT._line_style("CONTINUOUS"), "-")
        self.assertEqual(PLOT._line_style("dashed"), (0.0, (7.0, 3.0)))
        self.assertEqual(PLOT._line_style("DASHED2"), (0.0, (14.0, 5.0)))
        self.assertEqual(PLOT._line_style("DASHEDX2"), (0.0, (14.0, 5.0)))
        self.assertEqual(PLOT._line_style("DASHDOT"), (0.0, (10.0, 3.0, 0.0, 3.0)))
        self.assertEqual(PLOT._line_style("CENTER"), (0.0, (18.0, 4.0, 4.0, 4.0)))
        self.assertEqual(PLOT._line_style("CENTER2"), (0.0, (28.0, 6.0, 6.0, 6.0)))
        self.assertEqual(PLOT._line_style("CENTERX2"), (0.0, (28.0, 6.0, 6.0, 6.0)))
        self.assertEqual(PLOT._line_style("DOT"), (0.0, (0.0, 3.0)))
        self.assertEqual(PLOT._line_style("DOT2"), (0.0, (0.0, 5.0)))
        self.assertEqual(PLOT._line_style("DASHDOTX2"), (0.0, (18.0, 4.0, 0.0, 4.0)))
        self.assertEqual(PLOT._line_style("DOTX2"), (0.0, (0.0, 5.0)))
        self.assertEqual(PLOT._line_style("unknown"), "-")

    def test_line_capstyle_uses_round_caps_for_real_dots(self):
        self.assertEqual(PLOT._line_capstyle("DASHDOT"), "round")
        self.assertEqual(PLOT._line_capstyle("DASHDOTX2"), "round")
        self.assertEqual(PLOT._line_capstyle("DOT"), "round")
        self.assertIsNone(PLOT._line_capstyle("DASHED"))
        self.assertIsNone(PLOT._line_capstyle("CONTINUOUS"))

    def test_text_fontsize_uses_data_unit_scale_without_six_point_floor(self):
        self.assertAlmostEqual(PLOT._text_fontsize(2.0, 1.0, 2.5), 5.0)
        self.assertAlmostEqual(PLOT._text_fontsize(2.0, 1.0, 1.0), 2.0)

    def test_entity_linewidth_uses_dxf_line_weight(self):
        self.assertAlmostEqual(
            PLOT._entity_linewidth({"line_weight": 20}, 0.18),
            20 * 72 / 2540,
        )
        self.assertAlmostEqual(PLOT._entity_linewidth({"line_weight": -3}, 0.18), 0.18)
        self.assertAlmostEqual(PLOT._entity_linewidth({}, 0.18), 0.18)

    def test_filled_polygon_edges_are_hidden_by_default(self):
        hidden = PLOT._filled_polygon_edge_kwargs("black", 0.5, draw_edges=False)
        self.assertEqual(hidden["edgecolor"], "none")
        self.assertEqual(hidden["linewidth"], 0.0)
        self.assertFalse(hidden["antialiased"])

        visible = PLOT._filled_polygon_edge_kwargs("black", 0.5, draw_edges=True)
        self.assertEqual(visible["edgecolor"], "black")
        self.assertGreater(visible["linewidth"], 0.0)
        self.assertTrue(visible["antialiased"])

    def test_normalize_polygon_points_fixes_crossed_quad(self):
        points = [(0.0, 10.0), (10.0, 0.0), (10.0, 10.0), (0.0, 0.0)]
        normalized = PLOT._normalize_polygon_points(points)
        self.assertFalse(PLOT._polygon_points_cross(normalized))
        self.assertEqual(normalized[0], points[0])

    def test_ellipse_points_endpoints_for_full_loop(self):
        points = PLOT._ellipse_points(10.0, 5.0, 3.0, 0.0, 0.5, 0.0, 2.0 * math.pi)
        self.assertGreaterEqual(len(points), 24)
        self.assertAlmostEqual(points[0][0], points[-1][0], places=6)
        self.assertAlmostEqual(points[0][1], points[-1][1], places=6)


    def test_text_anchor_uses_jww_text_box_center(self):
        self.assertEqual(
            PLOT._text_anchor({"x": 0.0, "y": 0.0, "end_x": 2.5, "end_y": 0.0, "height": 2.5}),
            (1.25, 1.25, "center", "center"),
        )
        self.assertEqual(
            PLOT._text_anchor({"x": 1.25, "y": 0.0, "end_x": 1.25, "end_y": 2.5, "height": 2.5}),
            (0.0, 1.25, "center", "center"),
        )

    def test_text_anchor_falls_back_for_legacy_text_entities(self):
        self.assertEqual(
            PLOT._text_anchor({"x": 3.0, "y": 4.0, "height": 2.5}),
            (3.0, 4.0, "left", "bottom"),
        )

    def test_axes_are_hidden_by_default(self):
        import matplotlib

        matplotlib.use("Agg")
        import matplotlib.pyplot as plt

        ax = PLOT.plot_dxf_document({"entities": []})
        try:
            self.assertFalse(ax.axison)
        finally:
            plt.close(ax.figure)

    def test_axes_can_be_shown_for_debugging(self):
        import matplotlib

        matplotlib.use("Agg")
        import matplotlib.pyplot as plt

        ax = PLOT.plot_dxf_document({"entities": []}, show_axes=True)
        try:
            self.assertTrue(ax.axison)
            self.assertEqual(ax.get_xlabel(), "X")
            self.assertEqual(ax.get_ylabel(), "Y")
            self.assertEqual(ax.get_title(), "JWW Plot")
        finally:
            plt.close(ax.figure)

    def test_aci_to_color(self):
        self.assertEqual(PLOT._aci_to_color(1), "#ff0000")
        self.assertEqual(PLOT._aci_to_color(7), "#000000")
        fallback = PLOT._aci_to_color(200)
        self.assertIsInstance(fallback, tuple)
        self.assertEqual(len(fallback), 3)


if __name__ == "__main__":
    unittest.main()
