use ezjww_core::{
    parse_document_with_diagnostics, to_jww_bytes, Coord2D, Entity, JwwWriteDocument,
};
#[path = "../examples/support/writer_cases.rs"]
mod writer_cases;

fn roundtrip(doc: &JwwWriteDocument) -> ezjww_core::JwwDocument {
    let bytes = to_jww_bytes(doc).unwrap();
    assert_eq!(bytes, to_jww_bytes(doc).unwrap());
    let parsed = parse_document_with_diagnostics(&bytes).unwrap();
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.document.entities, doc.entities);
    assert!(parsed.document.block_defs.is_empty());
    parsed.document
}

fn invalid(doc: &JwwWriteDocument, field: &str) {
    let error = to_jww_bytes(doc).unwrap_err().to_string();
    assert!(error.starts_with(&format!("{field}:")), "{error}");
}

#[test]
fn mixed_entities_and_all_layer_settings_roundtrip() {
    for name in ["basic", "settings", "unicode"] {
        let doc = writer_cases::drawing(name).unwrap();
        let parsed = roundtrip(&doc);
        assert_eq!(parsed.header.paper_size, doc.options.paper_size);
        assert_eq!(parsed.header.memo, doc.options.memo);
        assert_eq!(
            parsed.header.write_layer_group,
            doc.options.write_layer_group
        );
        if name == "settings" {
            assert_eq!(parsed.header.layer_groups, doc.options.layer_groups);
        }
    }
}

#[test]
fn degrees_and_circle_flags_have_the_native_representation() {
    let mut doc = JwwWriteDocument::default();
    let arc = doc.add_arc(Coord2D::new(3.0, 5.0), 7.0, 350.0, 30.0);
    assert!((arc.start_angle - 6.1086523819801535).abs() < 1e-15);
    assert!((arc.arc_angle - std::f64::consts::PI / 6.0).abs() < 1e-15);
    assert!(!arc.is_full_circle);
    let circle = doc.add_circle(Coord2D::new(3.0, 5.0), 7.0);
    assert!(circle.is_full_circle);
    assert_eq!(circle.arc_angle, std::f64::consts::TAU);
    let text = doc.add_text(Coord2D::new(0.0, 0.0), Coord2D::new(0.0, 12.0), "90度");
    text.angle = 90.0;
    roundtrip(&doc);
}

#[test]
fn interleaved_classes_and_late_new_class_references() {
    let mut doc = writer_cases::drawing("basic").unwrap();
    let initial = doc.entities.clone();
    doc.entities.extend(initial);
    roundtrip(&doc);
    let mut doc = JwwWriteDocument::default();
    doc.add_line(Coord2D::new(0.0, 0.0), Coord2D::new(1.0, 1.0));
    doc.entities.resize(32766, doc.entities[0].clone());
    doc.add_point(Coord2D::new(1.0, 2.0));
    doc.add_point(Coord2D::new(3.0, 4.0));
    doc.add_text(Coord2D::new(0.0, 0.0), Coord2D::new(3.0, 0.0), "A");
    doc.add_text(Coord2D::new(0.0, 0.0), Coord2D::new(3.0, 0.0), "B");
    roundtrip(&doc);
}

#[test]
fn unicode_lengths_and_header_names_do_not_shift_following_sections() {
    for length in [0, 254, 255, 256, 65533, 65534, 65535, 65536] {
        let mut doc = JwwWriteDocument::default();
        let value = if length >= 2 {
            "あ".repeat(length - 2) + "𠮷"
        } else {
            String::new()
        };
        doc.options.layer_groups[15].name = value.clone();
        doc.options.layer_groups[15].layers[15].name = value.clone();
        let text = doc.add_text(
            Coord2D::new(0.0, 0.0),
            Coord2D::new(5.0, 0.0),
            value.clone(),
        );
        if !value.is_empty() {
            text.font_name = value.clone();
        }
        let parsed = roundtrip(&doc);
        assert_eq!(
            parsed.header.layer_groups[15].name,
            if value.is_empty() { "GroupF" } else { &value }
        );
        assert_eq!(
            parsed.header.layer_groups[15].layers[15].name,
            if value.is_empty() { "F-F" } else { &value }
        );
        // Variable strings must not damage the copied palette/line-type tables.
        let defaults = roundtrip(&JwwWriteDocument::default());
        assert_eq!(parsed.header.palette, defaults.header.palette);
        assert_eq!(parsed.header.line_types, defaults.header.line_types);
    }
    let mut doc = JwwWriteDocument::default();
    doc.add_text(Coord2D::new(0.0, 0.0), Coord2D::new(0.0, 0.0), "");
    roundtrip(&doc);
}

#[test]
fn basic_pen_attributes_and_layer_extremes_roundtrip() {
    let mut doc = JwwWriteDocument::default();
    for style in 1..=9 {
        let line = doc.add_line(Coord2D::new(0.0, 0.0), Coord2D::new(1.0, 2.0));
        line.base.pen_style = style;
        line.base.pen_color = u16::from(style);
        line.base.pen_width = 500;
        line.base.layer = 15;
        line.base.layer_group = 15;
    }
    roundtrip(&doc);
}

#[test]
fn invalid_arc_geometry_is_rejected() {
    for (field, value) in [
        ("radius", 0.0),
        ("radius", -1.0),
        ("radius", f64::INFINITY),
        ("center_x", f64::NAN),
        ("center_y", f64::NEG_INFINITY),
        ("start_angle", -0.1),
        ("start_angle", std::f64::consts::TAU),
        ("arc_angle", 0.0),
        ("arc_angle", -1.0),
        ("arc_angle", std::f64::consts::TAU),
        ("flatness", 0.5),
        ("tilt_angle", 1.0),
    ] {
        let mut doc = JwwWriteDocument::default();
        let arc = doc.add_arc(Coord2D::new(0.0, 0.0), 1.0, 0.0, 90.0);
        match field {
            "radius" => arc.radius = value,
            "center_x" => arc.center_x = value,
            "center_y" => arc.center_y = value,
            "start_angle" => arc.start_angle = value,
            "arc_angle" => arc.arc_angle = value,
            "flatness" => arc.flatness = value,
            "tilt_angle" => arc.tilt_angle = value,
            _ => unreachable!(),
        }
        invalid(&doc, &format!("entities[0].{field}"));
    }
    let mut doc = JwwWriteDocument::default();
    doc.add_circle(Coord2D::new(0.0, 0.0), 1.0).arc_angle = 1.0;
    invalid(&doc, "entities[0].arc_angle");
}

#[test]
fn points_do_not_silently_discard_marker_data() {
    for field in [
        "x",
        "y",
        "is_temporary",
        "code",
        "angle",
        "scale",
        "base.pen_style",
    ] {
        let mut doc = JwwWriteDocument::default();
        let point = doc.add_point(Coord2D::new(1.0, 2.0));
        match field {
            "x" => point.x = f64::NAN,
            "y" => point.y = f64::INFINITY,
            "is_temporary" => point.is_temporary = true,
            "code" => point.code = 1,
            "angle" => point.angle = 1.0,
            "scale" => point.scale = 1.0,
            "base.pen_style" => point.base.pen_style = 100,
            _ => unreachable!(),
        }
        invalid(&doc, &format!("entities[0].{field}"));
    }
}

#[test]
fn text_validation_distinguishes_dimension_flags_from_line_widths() {
    for field in [
        "size_x",
        "size_y",
        "spacing",
        "angle",
        "end_x",
        "text_type",
        "base.pen_width",
        "base.pen_style",
        "font_name",
        "content",
    ] {
        let mut doc = JwwWriteDocument::default();
        let text = doc.add_text(Coord2D::new(0.0, 0.0), Coord2D::new(4.0, 0.0), "ABC");
        match field {
            "size_x" => text.size_x = 0.0,
            "size_y" => text.size_y = -1.0,
            "spacing" => text.spacing = -1.0,
            "angle" => text.angle = 360.0,
            "end_x" => text.end_x = f64::INFINITY,
            "text_type" => text.text_type = 10001,
            "base.pen_width" => text.base.pen_width = 25,
            "base.pen_style" => text.base.pen_style = 2,
            "font_name" => text.font_name.clear(),
            "content" => text.content = "^@BMimage.bmp".into(),
            _ => unreachable!(),
        }
        invalid(&doc, &format!("entities[0].{field}"));
    }
    for content in ["bad\0text", "two\nlines", "tab\ttext"] {
        let mut doc = JwwWriteDocument::default();
        doc.add_text(Coord2D::new(0.0, 0.0), Coord2D::new(1.0, 0.0), content);
        invalid(&doc, "entities[0].content");
    }
    let mut doc = JwwWriteDocument::default();
    doc.add_text(
        Coord2D::new(0.0, -1000.0),
        Coord2D::new(0.0, -1000.0),
        "Printer_Orientation=1",
    );
    invalid(&doc, "entities[0].content");
}

#[test]
fn invalid_layer_settings_and_pen_attributes_are_errors() {
    for field in [
        "write_layer_group",
        "state",
        "write_layer",
        "scale",
        "protect",
        "name",
        "layers[0].state",
        "layers[0].protect",
        "layers[0].name",
    ] {
        let mut doc = JwwWriteDocument::default();
        let group = &mut doc.options.layer_groups[0];
        match field {
            "write_layer_group" => doc.options.write_layer_group = 16,
            "state" => group.state = 2,
            "write_layer" => group.write_layer = 16,
            "scale" => group.scale = f64::NAN,
            "protect" => group.protect = 3,
            "name" => group.name = "\0".into(),
            "layers[0].state" => group.layers[0].state = 2,
            "layers[0].protect" => group.layers[0].protect = 3,
            "layers[0].name" => group.layers[0].name = "\0".into(),
            _ => unreachable!(),
        }
        let prefix = if field == "write_layer_group" {
            "options."
        } else {
            "options.layer_groups[0]."
        };
        invalid(&doc, &format!("{prefix}{field}"));
    }
    for scale in [0.0, -1.0, f64::INFINITY] {
        let mut doc = JwwWriteDocument::default();
        doc.options.layer_groups[15].scale = scale;
        invalid(&doc, "options.layer_groups[15].scale");
    }
    for field in [
        "layer",
        "layer_group",
        "pen_style",
        "pen_color",
        "pen_width",
    ] {
        let mut doc = JwwWriteDocument::default();
        let line = doc.add_line(Coord2D::new(0.0, 0.0), Coord2D::new(1.0, 1.0));
        match field {
            "layer" => line.base.layer = 16,
            "layer_group" => line.base.layer_group = 16,
            "pen_style" => line.base.pen_style = 100,
            "pen_color" => line.base.pen_color = 100,
            "pen_width" => line.base.pen_width = 501,
            _ => unreachable!(),
        }
        invalid(&doc, &format!("entities[0].base.{field}"));
    }
}

#[test]
fn native_saved_geometry_text_and_layer_settings_match() {
    let fixtures =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../jww_samples/writer/basic");
    for name in ["basic", "settings", "unicode"] {
        let doc = writer_cases::drawing(name).unwrap();
        assert_eq!(
            to_jww_bytes(&doc).unwrap(),
            std::fs::read(fixtures.join(format!("{name}.jww"))).unwrap()
        );
        let original = roundtrip(&doc);
        for suffix in ["_saved.jww", "_reopened.jww"] {
            let saved = parse_document_with_diagnostics(
                &std::fs::read(fixtures.join(format!("{name}{suffix}"))).unwrap(),
            )
            .unwrap();
            assert!(saved.diagnostics.is_empty());
            let saved = saved.document;
            assert_eq!(saved.header.version, 700);
            assert_eq!(saved.header.paper_size, doc.options.paper_size);
            assert_eq!(
                saved.header.memo,
                if doc.options.memo.is_empty() {
                    "\r\n"
                } else {
                    &doc.options.memo
                }
            );
            assert_eq!(
                saved.header.write_layer_group,
                doc.options.write_layer_group
            );
            assert_eq!(saved.header.layer_groups, original.header.layer_groups);
            assert!(saved.block_defs.is_empty());
            let actual: Vec<_> = saved.entities.into_iter().filter(|entity| {
                !matches!(entity, Entity::Text(text) if ezjww_core::metadata_setting_from_text(text).is_some())
            }).collect();
            // Native Jw_cad deletes empty text. The Rust codec itself preserves it.
            let expected: Vec<_> = doc
                .entities
                .iter()
                .filter(|entity| !matches!(entity, Entity::Text(text) if text.content.is_empty()))
                .cloned()
                .collect();
            assert_eq!(actual.len(), expected.len(), "{name}{suffix}");
            let mut text_index = 0;
            for (mut a, b) in actual.into_iter().zip(expected) {
                match (&mut a, &b) {
                    (Entity::Arc(a), Entity::Arc(b)) => {
                        // Native saves may wrap the start angle across 0/2*pi.
                        assert!((a.start_angle.sin() - b.start_angle.sin()).abs() < 1e-12);
                        assert!((a.start_angle.cos() - b.start_angle.cos()).abs() < 1e-12);
                        a.start_angle = b.start_angle;
                    }
                    (Entity::Text(a), Entity::Text(b)) => {
                        // Independent measured native baseline endpoints; no font
                        // measurement model is reused from the writer or converter.
                        let endpoints: &[(f64, f64)] = match name {
                            "basic" => &[(-32.75, -40.0), (30.0, -31.0)],
                            "unicode" => &[(24.0, 10.0), (765.0, 20.0), (3072.0, 30.0)],
                            _ => unreachable!(),
                        };
                        assert_eq!((a.end_x, a.end_y), endpoints[text_index]);
                        text_index += 1;
                        a.end_x = b.end_x;
                        a.end_y = b.end_y;
                    }
                    _ => {}
                }
                assert_eq!(a, b, "{name}{suffix}");
            }
        }
    }
}
