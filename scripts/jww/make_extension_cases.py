#!/usr/bin/env python3
"""Generate native acceptance inputs for writer requests R1-R8 (no native claim)."""

import argparse
import json
from pathlib import Path
import ezjww


def cases():
    result = {}
    d = ezjww.new()
    m = d.modelspace()
    for i, pen in enumerate((1, 9, 100, 101, 116, 117, 256, 356)):
        m.add_line((-70, i * 5), (-30, i * 5), jwwattribs={"pen_color": pen})
    for i, color in enumerate((0x1256AB, 0xA53212, 0x782AE0)):
        m.add_line((0, i * 5), (40, i * 5), color=color)
    result["sxf_colors"] = d
    d = ezjww.new()
    for i, style in enumerate([16, 17, 18, 19, *range(30, 63)]):
        d.modelspace().add_line(
            (-60, -80 + i * 4), (60, -80 + i * 4), jwwattribs={"pen_style": style}
        )
    d.options["line_types"]["sxf"][17].update(
        name="custom 6-2",
        pattern=0x77777777,
        unit_dots=4,
        pitch=1,
        printer_pitch=64,
        segments_mm=[6.0, 2.0],
    )
    result["sxf_linetypes"] = d
    d = ezjww.new()
    m = d.modelspace()
    m.add_solid((-60, -20), (-40, -20), (-40, 0), (-60, 0))
    m.add_solid((-20, -20), (0, -20), (-10, 0), color=0x1256AB)
    m.add_circle_solid((30, -10), 10, color=0xA53212)
    m.add_circle_solid((-50, 30), 10, start_angle=30, sweep_angle=240)
    m.add_circle_solid((-10, 30), 15, inner_radius=8, flatness=0.8, ring_style=105)
    m.add_circle_solid((30, 30), 15, inner_radius=8, flatness=0.8, ring_style=106)
    result["solid"] = d
    d = ezjww.new()
    m = d.modelspace()
    m.add_ellipse((-40, 0), 20, 0.5)
    m.add_ellipse((10, 0), 20, 0.5, tilt_angle=30)
    m.add_ellipse((60, 0), 20, 0.5, tilt_angle=30, start_angle=20, sweep_angle=240)
    result["ellipse"] = d
    d = ezjww.new()
    m = d.modelspace()
    m.add_dimension((-60, -30), (-20, -30), "40", (-45, -26))
    m.add_dimension((0, -30), (0, 10), "40", (4, -15), text_options={"angle": 90})
    m.add_dimension(
        (20, 30),
        (60, 30),
        "40",
        (35, 34),
        sxf_mode=1,
        aux_lines=[((20, 0), (20, 35)), ((60, 0), (60, 35))],
        aux_points=[(20, 30), (60, 30), (20, 0), (60, 0)],
    )
    result["dimension"] = d
    d = ezjww.new()
    unit = ezjww.new().modelspace()
    unit.add_line((0, 0), (1000, 0))
    unit.add_circle((500, 500), 300)
    d.add_block("unit", unit.entities)
    unit.entities = []
    ref = unit.add_block_ref(0, (0, 0), scale_x=0.01, scale_y=0.01, rotation=30)
    d.add_block("assembly", [ref])
    d.modelspace().add_block_ref("assembly", (-20, -20), rotation=45)
    d.modelspace().add_block_ref("unit", (20, 20), scale_x=0.01, scale_y=0.01)
    result["block"] = d
    d = ezjww.new()
    for i, (text, angle) in enumerate(
        [("日本語 ABC", 0), ("半角ｶﾅ 123", 30), ("Rotate", 90)]
    ):
        d.modelspace().add_text(text, (-60, -30 + i * 20), angle=angle, spacing=0.5)
    d.modelspace().add_text("Preset 3", (10, 0), text_type=3)
    result["text_auto_end"] = d
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("output", type=Path)
    args = p.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for name, drawing in cases().items():
        for suffix in (".jww", ".json"):
            if (args.output / (name + suffix)).exists():
                raise FileExistsError(args.output / (name + suffix))
        drawing.saveas(args.output / (name + ".jww"))
        (args.output / (name + ".json")).write_text(
            json.dumps(drawing.modelspace()._input(), ensure_ascii=False, indent=2)
            + "\n",
            encoding="utf-8",
        )
        print(name, len(drawing.modelspace()))


if __name__ == "__main__":
    main()
