//! Original inputs for the PR5 native application compatibility matrix.
use ezjww_core::{Coord2D, JwwWriteDocument};

pub const CASES: &[&str] = &[
    "empty",
    "line",
    "circle",
    "arc",
    "point",
    "text",
    "basic",
    "settings",
    "unicode",
    "attributes",
    "text_styles",
    "text_rotations",
    "count65534",
    "count65535",
    "count65536",
    "late_classes",
    "cstring_boundaries",
    "memo_ascii64",
    "memo_ascii65",
    "memo_utf16_64",
    "memo_utf16_65",
];

pub fn drawing(name: &str) -> Option<JwwWriteDocument> {
    if let Some(doc) = super::writer_cases::drawing(name) {
        return Some(doc);
    }
    let mut doc = JwwWriteDocument::default();
    match name {
        "circle" => {
            doc.add_circle(Coord2D::new(5.0, 10.0), 20.0);
        }
        "arc" => {
            doc.add_arc(Coord2D::new(5.0, 10.0), 20.0, 350.0, 30.0);
        }
        "point" => {
            doc.add_point(Coord2D::new(5.0, 10.0));
        }
        "text" => {
            doc.add_text(
                Coord2D::new(-20.0, 0.0),
                Coord2D::new(10.0, 0.0),
                "日本語 ABC",
            );
        }
        "attributes" => {
            for pen in 1..=9 {
                for (i, width) in [0, 1, 25, 500].into_iter().enumerate() {
                    let y = 60.0 - (pen * 4 + i as u16) as f64 * 3.0;
                    let line = doc.add_line(Coord2D::new(-60.0, y), Coord2D::new(-5.0, y));
                    line.base.pen_style = pen as u8;
                    line.base.pen_color = pen;
                    line.base.pen_width = width;
                    line.base.layer = pen;
                    let arc =
                        doc.add_arc(Coord2D::new(20.0 + i as f64 * 20.0, y), 2.0, 45.0, 270.0);
                    arc.base = line_base(pen, width);
                }
            }
        }
        "text_styles" | "text_rotations" => {
            for (i, angle) in [0.0, 45.0, 90.0, 180.0, 270.0, 359.0]
                .into_iter()
                .enumerate()
            {
                let x = -75.0 + (i % 3) as f64 * 60.0;
                let y = 40.0 - (i / 3) as f64 * 65.0;
                // text_styles probes inconsistent baseline/angle values; the
                // ordinary rotation case aligns both inputs.
                // Freeze the originally measured endpoints rather than relying
                // on platform-specific sin/cos rounding in fixture generation.
                let endpoints = [
                    (-45.0, 40.0),
                    (6.2132034355964265, 61.21320343559643),
                    (45.0, 70.0),
                    (-105.0, -24.999999999999996),
                    (-15.000000000000005, -55.0),
                    (74.99543085469173, -25.523572193118508),
                ];
                let (end_x, end_y) = if name == "text_rotations" {
                    endpoints[i]
                } else {
                    (x + 30.0, y)
                };
                let text =
                    doc.add_text(Coord2D::new(x, y), Coord2D::new(end_x, end_y), "日本語 ABC");
                text.angle = angle;
                text.size_x = 2.5;
                text.size_y = 4.0;
                text.spacing = 0.5;
                text.base.pen_color = (i + 1) as u16;
                if i % 2 == 1 {
                    text.font_name = "ＭＳ 明朝".into();
                }
            }
        }
        "count65534" | "count65535" | "count65536" | "late_classes" => {
            let count = match name {
                "count65534" => 65534,
                "count65535" => 65535,
                "count65536" => 65536,
                _ => 32766,
            };
            // Unique short segments: native deduplication cannot hide a lost
            // record, and every coordinate fits within the default A3 sheet.
            for i in 0..count {
                let x = -100.0 + (i % 256) as f64 * 0.75;
                let y = -60.0 + (i / 256) as f64 * 0.5;
                doc.add_line(Coord2D::new(x, y), Coord2D::new(x + 0.25, y + 0.125));
            }
            if name == "late_classes" {
                // Each new class and its repeated reference occur after the
                // 15-bit PID boundary. CIRCLE and ARC share CDataEnko.
                doc.add_circle(Coord2D::new(20.0, 50.0), 5.0);
                doc.add_arc(Coord2D::new(35.0, 50.0), 5.0, 350.0, 30.0);
                doc.add_point(Coord2D::new(50.0, 50.0));
                doc.add_point(Coord2D::new(55.0, 50.0));
                doc.add_text(
                    Coord2D::new(-50.0, 50.0),
                    Coord2D::new(-40.0, 50.0),
                    "後半A",
                );
                doc.add_text(
                    Coord2D::new(-20.0, 50.0),
                    Coord2D::new(-10.0, 50.0),
                    "後半B",
                );
            }
        }
        "cstring_boundaries" => {
            doc.options.memo = "長".repeat(65532) + "𠮷"; // 65534 UTF-16 units
            for (i, length) in [254, 255, 256, 65533, 65534, 65535, 65536]
                .into_iter()
                .enumerate()
            {
                let value = "層".repeat(length - 2) + "𠮷";
                doc.options.layer_groups[i].name = value.clone();
                doc.options.layer_groups[i].layers[2].name = value;
            }
            doc.add_line(Coord2D::new(-20.0, 0.0), Coord2D::new(20.0, 0.0));
        }
        "memo_ascii64" => doc.options.memo = "m".repeat(64),
        "memo_ascii65" => doc.options.memo = "m".repeat(65),
        "memo_utf16_64" => doc.options.memo = "長".repeat(62) + "𠮷",
        "memo_utf16_65" => doc.options.memo = "長".repeat(63) + "𠮷",
        _ => return None,
    }
    Some(doc)
}

fn line_base(pen: u16, width: u16) -> ezjww_core::EntityBase {
    ezjww_core::EntityBase {
        pen_style: pen as u8,
        pen_color: pen,
        pen_width: width,
        layer: pen,
        ..ezjww_core::EntityBase::default()
    }
}
