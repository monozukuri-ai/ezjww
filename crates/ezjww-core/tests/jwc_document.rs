use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use ezjww_core::diagnostics::{JWC_ATTRIBUTE_UNVERIFIED, JWC_CURVE_MARKERS_UNVERIFIED};
use ezjww_core::jwc::{JwcDocumentProfile, JwcEntityData, JwcLayerAddress, JWC_MAX_FILE_SIZE};
use ezjww_core::{
    metadata_setting_from_text, parse_document, parse_jwc_document, parse_jwc_header,
    read_jwc_document_from_file, Coord2D, Entity, JwcDocument, JwcError,
};
use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        root()
            .join("jwc_samples/generated")
            .join(format!("{name}.jwc")),
    )
    .unwrap()
}

fn cases() -> Vec<Value> {
    ["cases.json", "followup_cases.json"]
        .into_iter()
        .flat_map(|name| {
            serde_json::from_slice::<Vec<Value>>(
                &fs::read(root().join("jwc_samples").join(name)).unwrap(),
            )
            .unwrap()
        })
        .collect()
}

fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        actual.is_finite() && (actual - expected).abs() <= tolerance,
        "{actual} != {expected}, tolerance {tolerance}"
    );
}

// Test-only oracle conversion, pinned by P0 native JWW/DXF evidence. The parser
// itself deliberately leaves these coordinates in the original JWC units.
fn paper(point: Coord2D, doc: &JwcDocument) -> Coord2D {
    let (width, height) = doc.header.paper.dimensions_mm();
    Coord2D::new(
        point.x * width / 518. - width / 2.,
        point.y * width / 518. - height / 2.,
    )
}

fn known_coordinate(point: Coord2D, known: &[Value], doc: &JwcDocument) {
    let actual = paper(point, doc);
    let factor = doc.header.paper.dimensions_mm().0 / 518.;
    for ((value, raw), expected) in [actual.x, actual.y]
        .into_iter()
        .zip([point.x, point.y])
        .zip(known)
    {
        let half_ulp = if raw == 0. {
            2_f64.powi(-150)
        } else {
            2_f64.powf(raw.abs().log2().floor() - 24.)
        };
        near(value, expected.as_f64().unwrap(), half_ulp * factor + 1e-9);
    }
}

fn native_coordinate(point: Coord2D, x: f64, y: f64, doc: &JwcDocument, tolerance: f64) {
    let actual = paper(point, doc);
    near(actual.x, x, tolerance);
    near(actual.y, y, tolerance);
}

fn native_layer(layer: JwcLayerAddress, base: &ezjww_core::EntityBase) {
    assert_eq!(u16::from(layer.group), base.layer_group);
    assert_eq!(u16::from(layer.layer), base.layer);
}

#[test]
fn native_corpus_matches_known_inputs_and_independently_reopened_jww() {
    let mut accepted = 0;
    let mut rejected = Vec::new();
    for case in cases() {
        let id = case["id"].as_str().unwrap();
        let data = fixture(id);
        let doc = match parse_jwc_document(&data) {
            Ok(doc) => doc,
            Err(error) => {
                let expected = match id {
                    "r080" => 2483,
                    _ => panic!("{id}: {error}"),
                };
                assert!(matches!(error, JwcError::UnsupportedLayout { .. }));
                assert_eq!(error.byte_offset(), Some(expected));
                rejected.push(id.to_owned());
                continue;
            }
        };
        assert_eq!(doc.profile_id, JwcDocumentProfile::Fixed2421BasicV1);
        assert_eq!(doc.header.fixed_header_size, 2421);
        assert_eq!(
            doc.entity_counts().values().sum::<usize>(),
            doc.entities.len()
        );
        // Entity text must not add decode diagnostics beyond the header's own.
        let decode_codes = |diagnostics: &[ezjww_core::Diagnostic]| {
            diagnostics
                .iter()
                .filter(|d| d.unverified_details().is_none())
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(
            decode_codes(&doc.diagnostics),
            decode_codes(&doc.header.diagnostics),
            "native text should decode: {id}"
        );
        let native = parse_document(
            &fs::read(
                root()
                    .join("jwc_samples/generated")
                    .join(format!("{id}rt.jww")),
            )
            .unwrap(),
        )
        .unwrap();
        let mut remaining: Vec<_> = native
            .entities
            .iter()
            .filter(|e| !matches!(e, Entity::Text(t) if metadata_setting_from_text(t).is_some()))
            .collect();
        let mut expected: Vec<_> = case["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| !(e["type"] == "text" && e["content"] == ""))
            .collect();
        if let Some(order) = case["expected_stored_order"].as_array() {
            expected = order
                .iter()
                .map(|i| expected[i.as_u64().unwrap() as usize])
                .collect();
        }
        expected.sort_by_key(|e| {
            if e["temporary"] == true {
                0
            } else {
                match e["type"].as_str().unwrap() {
                    "line" => 1,
                    "arc" => 2,
                    "text" => 3,
                    "point" => 4,
                    _ => unreachable!(),
                }
            }
        });
        assert_eq!(doc.entities.len(), expected.len(), "{id}");
        assert_eq!(doc.entities.len(), remaining.len(), "{id}");
        for (entity, known) in doc.entities.iter().zip(expected) {
            let raw: Vec<u8> = entity
                .source
                .spans
                .iter()
                .flat_map(|s| data[s.byte_offset..s.byte_offset + s.byte_length].to_vec())
                .collect();
            assert_eq!(entity.source.raw_bytes, raw, "{id}");
            let native_index = remaining
                .iter()
                .position(|native| match (&entity.data, native) {
                    (JwcEntityData::Line { .. }, Entity::Line(_))
                    | (JwcEntityData::Arc { .. }, Entity::Arc(_))
                    | (JwcEntityData::Text { .. }, Entity::Text(_)) => true,
                    (JwcEntityData::Point { .. }, Entity::Point(p)) => !p.is_temporary,
                    (JwcEntityData::TemporaryPoint { .. }, Entity::Point(p)) => p.is_temporary,
                    _ => false,
                })
                .unwrap();
            let native = remaining.remove(native_index);
            match (&entity.data, native) {
                (
                    JwcEntityData::Line {
                        start,
                        end,
                        attributes,
                    },
                    Entity::Line(line),
                ) => {
                    assert_eq!(known["type"], "line");
                    let xy = known["xy"].as_array().unwrap();
                    known_coordinate(*start, &xy[..2], &doc);
                    known_coordinate(*end, &xy[2..], &doc);
                    native_coordinate(*start, line.start_x, line.start_y, &doc, 1e-7);
                    native_coordinate(*end, line.end_x, line.end_y, &doc, 1e-7);
                    native_layer(attributes.layer, &line.base);
                    assert_eq!(attributes.pen_style, line.base.pen_style);
                    assert_eq!(u16::from(attributes.pen_color), line.base.pen_color);
                    // Native Jw_cad clears some stored flag bits when it re-exports
                    // JWW (see `expected_native_flag_changes` in the case files).
                    let expected_flag = case["expected_native_flag_changes"]["line"]
                        [attributes.flags_raw.to_string()]
                    .as_u64()
                    .map_or(attributes.flags_raw, |value| value as u16);
                    assert_eq!(expected_flag, line.base.flag, "{id}");
                }
                (
                    JwcEntityData::Arc {
                        center,
                        radius,
                        flatness,
                        start_angle_degrees,
                        end_angle_degrees,
                        tilt_angle_degrees,
                        is_full_circle,
                        attributes,
                    },
                    Entity::Arc(arc),
                ) => {
                    assert_eq!(known["type"], "arc");
                    known_coordinate(*center, known["center"].as_array().unwrap(), &doc);
                    native_coordinate(*center, arc.center_x, arc.center_y, &doc, 1e-7);
                    let factor = doc.header.paper.dimensions_mm().0 / 518.;
                    near(
                        radius * factor,
                        known["radius"].as_f64().unwrap(),
                        2_f64.powf(radius.log2().floor() - 24.) * factor + 1e-9,
                    );
                    near(radius * factor, arc.radius, 1e-7);
                    near(*flatness, known["flatness"].as_f64().unwrap_or(1.), 1e-4);
                    near(*flatness, arc.flatness, 1e-10);
                    near(
                        *tilt_angle_degrees,
                        known["tilt_deg"].as_f64().unwrap_or(0.),
                        1. / 65536. + 1e-9,
                    );
                    near(tilt_angle_degrees.to_radians(), arc.tilt_angle, 1e-10);
                    assert_eq!(*is_full_circle, known["full"].as_bool().unwrap_or(false));
                    assert_eq!(*is_full_circle, arc.is_full_circle);
                    if !is_full_circle {
                        let begin = known["start_deg"].as_f64().unwrap_or(0.);
                        let end = (begin + known["sweep_deg"].as_f64().unwrap_or(360.)) % 360.;
                        near(*start_angle_degrees, begin, 1. / 65536. + 1e-9);
                        near(*end_angle_degrees, end, 1. / 65536. + 1e-9);
                        // Native JWW may express the same angle as -10 instead
                        // of JWC's 350 degrees. Compare modulo one revolution.
                        near(
                            start_angle_degrees.to_radians(),
                            arc.start_angle.rem_euclid(std::f64::consts::TAU),
                            1e-10,
                        );
                        near(
                            (end_angle_degrees - start_angle_degrees)
                                .rem_euclid(360.)
                                .to_radians(),
                            arc.arc_angle,
                            1e-10,
                        );
                    }
                    native_layer(attributes.layer, &arc.base);
                    assert_eq!(attributes.pen_style, arc.base.pen_style);
                    assert_eq!(u16::from(attributes.pen_color), arc.base.pen_color);
                    assert_eq!(attributes.flags_raw, arc.base.flag);
                }
                (
                    JwcEntityData::Point {
                        position,
                        layer,
                        pen_color,
                        flags_raw,
                    },
                    Entity::Point(point),
                ) => {
                    assert_eq!(known["type"], "point");
                    known_coordinate(*position, known["xy"].as_array().unwrap(), &doc);
                    native_coordinate(*position, point.x, point.y, &doc, 1e-7);
                    native_layer(*layer, &point.base);
                    assert_eq!(u16::from(*pen_color), point.base.pen_color);
                    assert_eq!(*flags_raw, point.base.flag);
                }
                (
                    JwcEntityData::TemporaryPoint {
                        position,
                        layer,
                        array_index,
                    },
                    Entity::Point(point),
                ) => {
                    assert_eq!(known["temporary"], true);
                    known_coordinate(*position, known["xy"].as_array().unwrap(), &doc);
                    native_coordinate(*position, point.x, point.y, &doc, 1e-7);
                    native_layer(*layer, &point.base);
                    assert_eq!(entity.source.spans.len(), 3);
                    assert_eq!(
                        entity.source.spans[0].byte_offset,
                        800 + usize::from(*array_index) * 4
                    );
                }
                (
                    JwcEntityData::Text {
                        start,
                        end,
                        layer,
                        text_preset,
                        content,
                        raw_content,
                        string_source,
                    },
                    Entity::Text(text),
                ) => {
                    assert_eq!(known["type"], "text");
                    let default_xy = serde_json::json!([1, 2]);
                    known_coordinate(
                        *start,
                        known.get("xy").unwrap_or(&default_xy).as_array().unwrap(),
                        &doc,
                    );
                    // Native read adjusts text baseline endpoints from the quantized
                    // baseline and text preset; retain the P0 1e-4 comparison bound.
                    native_coordinate(*start, text.start_x, text.start_y, &doc, 1e-4);
                    native_coordinate(*end, text.end_x, text.end_y, &doc, 1e-4);
                    near(
                        (end.y - start.y).atan2(end.x - start.x).to_degrees(),
                        text.angle,
                        1e-7,
                    );
                    native_layer(*layer, &text.base);
                    assert_eq!(content, known["content"].as_str().unwrap());
                    assert_eq!(content, &text.content);
                    assert_eq!(u32::from(*text_preset), text.text_type);
                    let preset = doc.header.text_presets[usize::from(*text_preset)];
                    near(f64::from(preset.width_tenths) / 10., text.size_x, 1e-7);
                    near(f64::from(preset.height_tenths) / 10., text.size_y, 1e-7);
                    near(f64::from(preset.spacing_tenths) / 10., text.spacing, 1e-7);
                    assert_eq!(preset.pen_color, text.base.pen_color);
                    let bytes = &data[string_source.byte_offset
                        ..string_source.byte_offset + string_source.byte_length];
                    assert_eq!(raw_content, &bytes[..bytes.len() - 1]);
                    assert_eq!(bytes.last(), Some(&0));
                }
                _ => panic!("{id}: mismatched entity"),
            }
        }
        assert!(remaining.is_empty());
        accepted += 1;
    }
    assert_eq!(accepted, 79);
    assert_eq!(rejected, ["r080"]);
}

#[test]
fn empty_mixed_and_source_unit_contracts_are_explicit() {
    let empty = parse_jwc_document(&fixture("q000")).unwrap();
    assert!(empty.entities.is_empty() && empty.entity_counts().is_empty());
    assert!(parse_jwc_document(&fixture("q035"))
        .unwrap()
        .entities
        .is_empty());
    let mixed = parse_jwc_document(&fixture("q070")).unwrap();
    assert_eq!(
        mixed
            .entities
            .iter()
            .map(|e| e.entity_type())
            .collect::<Vec<_>>(),
        ["LINE", "CIRCLE", "TEXT", "POINT"]
    );
    let a = parse_jwc_document(&fixture("q001")).unwrap();
    let b = parse_jwc_document(&fixture("q050")).unwrap();
    assert_eq!(
        a.entities, b.entities,
        "group scale must not be applied during record decoding"
    );
    assert_ne!(
        a.header.layer_groups[0].scale,
        b.header.layer_groups[0].scale
    );
    let json = serde_json::to_value(&mixed).unwrap();
    assert_eq!(json["profile_id"], "fixed2421_basic_v1");
    assert!(json["header"]["source_version"].is_null());
    assert_eq!(json["entities"][0]["type"], "line");
    assert!(json["entities"][2].get("font_name").is_none());
}

#[test]
fn all_coordinate_fields_reject_nonfinite_values_with_exact_offsets() {
    for (id, offsets) in [
        ("q001", vec![2421, 2425, 2429, 2433]),
        ("q010", vec![2421, 2425, 2429]),
        ("q020", vec![2421, 2425]),
        ("q030", vec![2421, 2425, 2429, 2433]),
        ("q021", vec![804, 1208]),
    ] {
        for offset in offsets {
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut data = fixture(id);
                data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                assert!(
                    matches!(parse_jwc_document(&data), Err(JwcError::InvalidValue { offset: o, .. }) if o == offset)
                );
            }
        }
    }
}

#[test]
fn unverified_attributes_are_retained_and_reported_not_dropped() {
    // (fixture, offset, value, expected JWC_ATTRIBUTE_UNVERIFIED field or "" for none)
    for (id, offset, value, field) in [
        ("q001", 2437, 5, ""),                  // JWW-numbered dash-dot style
        ("q001", 2437, 18, "entity.pen_style"), // style 2 with attribute bit 0x10
        ("q001", 2438, 6, ""),                  // pen 6 exists in real files
        ("q001", 2440, 1, "entity.spare"),
        ("q001", 2441, 2, "line.flags"),
        ("q010", 2447, 9, ""),
        ("q010", 2450, 1, "entity.spare"),
        ("q010", 2451, 1, "arc.flags"),
        ("q020", 2430, 7, ""),
        ("q020", 2431, 1, "point.flags"),
        ("q030", 2443, 1, "text.flags"),
        ("q030", 1715, 6, ""), // preset 3 uses pen 6
    ] {
        let mut data = fixture(id);
        data[offset] = value;
        let doc = parse_jwc_document(&data).unwrap_or_else(|error| panic!("{id}: {error}"));
        assert_eq!(doc.entities.len(), 1, "{id}");
        let attributes: Vec<_> = doc
            .diagnostics
            .iter()
            .filter(|d| d.code == JWC_ATTRIBUTE_UNVERIFIED)
            .collect();
        if field.is_empty() {
            assert!(attributes.is_empty(), "{id} {offset}: {attributes:?}");
        } else {
            assert_eq!(attributes.len(), 1, "{id} {offset}");
            let details = attributes[0].unverified_details().unwrap();
            assert_eq!(details.field, field, "{id}");
            assert_eq!((details.byte_offset, details.count), (offset, 1), "{id}");
            assert_eq!(attributes[0].severity, "warning");
            assert_eq!(attributes[0].action, "retained");
        }
    }
    // Values with no plausible meaning still fail at the exact byte.
    for (id, offset, value) in [
        ("q001", 2437, 0),
        ("q001", 2437, 10),
        ("q001", 2438, 0),
        ("q001", 2438, 10),
        ("q030", 2441, 0),
        ("q030", 2441, 11),
        ("q030", 1715, 10),
    ] {
        let mut data = fixture(id);
        data[offset] = value;
        let error = parse_jwc_document(&data).unwrap_err();
        assert!(
            matches!(error, JwcError::UnsupportedLayout { .. }),
            "{id}: {error}"
        );
        assert_eq!(error.byte_offset(), Some(offset), "{id}");
    }
    // Curve markers outside the verified sequence keep the lines ungrouped.
    for (id, offset, value, marker) in [
        ("q001", 2441, 0x80, "0x80"),
        ("q001", 2441, 0x40, "unterminated"),
        ("r024", 2485, 0, "0x00"),
    ] {
        let mut data = fixture(id);
        data[offset] = value;
        let doc = parse_jwc_document(&data).unwrap_or_else(|error| panic!("{id}: {error}"));
        let markers: Vec<_> = doc
            .diagnostics
            .iter()
            .filter(|d| d.code == JWC_CURVE_MARKERS_UNVERIFIED)
            .collect();
        assert_eq!(markers.len(), 1, "{id}");
        let details = markers[0].unverified_details().unwrap();
        assert_eq!(
            (details.count, details.values.as_slice()),
            (1, &[marker.to_string()][..]),
            "{id}"
        );
    }
    // r011 stores line flag 2, which native Jw_cad clears on re-export.
    let flagged = parse_jwc_document(&fixture("r011")).unwrap();
    assert!(matches!(
        flagged.entities[0].data,
        JwcEntityData::Line { attributes, .. } if attributes.flags_raw == 2
    ));
    assert_eq!(flagged.diagnostics.len(), 1);
    let grouped = parse_jwc_document(&fixture("r024")).unwrap();
    assert!(grouped.diagnostics.is_empty());
    let flags: Vec<_> = grouped
        .entities
        .iter()
        .map(|e| match e.data {
            JwcEntityData::Line { attributes, .. } => attributes.flags_raw,
            _ => panic!(),
        })
        .collect();
    assert_eq!(flags, [0, 0x40, 0x80, 0xc0, 0x40, 0xc0]);
}

#[test]
fn arc_radius_flatness_and_angle_ranges_fail_while_tilts_and_elliptical_arcs_are_read() {
    for radius in [0_f32, -1.] {
        let mut data = fixture("q010");
        data[2429..2433].copy_from_slice(&radius.to_le_bytes());
        assert!(matches!(
            parse_jwc_document(&data),
            Err(JwcError::InvalidValue { offset: 2429, .. })
        ));
    }
    for flatness in [0_u16, 10001, u16::MAX] {
        let mut data = fixture("q010");
        data[2433..2435].copy_from_slice(&flatness.to_le_bytes());
        let error = parse_jwc_document(&data).unwrap_err();
        assert_eq!(error.byte_offset(), Some(2433));
    }
    for offset in [2435, 2439, 2443] {
        for raw in [-1_i32, 360 * 65536, i32::MAX, i32::MIN] {
            let mut data = fixture("q010");
            data[offset..offset + 4].copy_from_slice(&raw.to_le_bytes());
            assert!(
                matches!(parse_jwc_document(&data), Err(JwcError::UnsupportedLayout { offset: o, .. }) if o == offset)
            );
        }
    }
    // Equal nonzero angles denote a closed curve in real files.
    let mut equal = fixture("q010");
    equal[2435..2439].copy_from_slice(&65536_i32.to_le_bytes());
    equal[2439..2443].copy_from_slice(&65536_i32.to_le_bytes());
    let doc = parse_jwc_document(&equal).unwrap();
    assert!(matches!(
        doc.entities[0].data,
        JwcEntityData::Arc { is_full_circle: true, start_angle_degrees, .. } if start_angle_degrees == 1.
    ));
    assert_eq!(doc.entities[0].entity_type(), "CIRCLE");
    assert_eq!(
        doc.diagnostics[0].unverified_details().unwrap().field,
        "arc.angles"
    );
    // Partial elliptical arcs and tilted circular arcs are read as stored.
    let mut ellipse = fixture("q012");
    ellipse[2433..2435].copy_from_slice(&5000_u16.to_le_bytes());
    let doc = parse_jwc_document(&ellipse).unwrap();
    assert!(matches!(
        doc.entities[0].data,
        JwcEntityData::Arc { flatness, is_full_circle: false, .. } if flatness == 0.5
    ));
    let mut tilted = fixture("q012");
    tilted[2443..2447].copy_from_slice(&(30 * 65536_i32).to_le_bytes());
    let doc = parse_jwc_document(&tilted).unwrap();
    assert!(matches!(
        doc.entities[0].data,
        JwcEntityData::Arc { tilt_angle_degrees, flatness, .. } if tilt_angle_degrees == 30. && flatness == 1.
    ));
    assert!(doc.diagnostics.is_empty());
}

#[test]
fn cp932_text_diagnostics_keep_bytes_and_do_not_sniff_boms() {
    let mut data = fixture("q030");
    data[2445..2448].copy_from_slice(b"\xff\xfeA");
    let names = parse_jwc_header(&data).unwrap().layout.names.byte_offset;
    data[names..names + 8].copy_from_slice(b"ABCDEF\x93\0");
    let doc = parse_jwc_document(&data).unwrap();
    let JwcEntityData::Text {
        content,
        raw_content,
        string_source,
        ..
    } = &doc.entities[0].data
    else {
        panic!()
    };
    assert_eq!(content, "\u{fffd}\u{fffd}A");
    assert_eq!(raw_content, b"\xff\xfeA");
    assert_eq!(string_source.byte_offset, 2445);
    assert_eq!(string_source.byte_length, 4);
    assert_eq!(doc.header.diagnostics.len(), 1);
    assert_eq!(doc.diagnostics.len(), 2);
    assert_eq!(doc.diagnostics[0], doc.header.diagnostics[0]);
    let details = doc.diagnostics[1].decode_details().unwrap();
    assert_eq!(details.field, "entities[0].content");
    assert_eq!(
        (
            details.byte_offset,
            details.byte_length,
            details.replacement_characters
        ),
        (2445, 3, 2)
    );
    assert_eq!(doc.diagnostics[1].code, "CP932_DECODE_REPLACED");
    let mut pooled = fixture("r001");
    pooled[2493..2496].copy_from_slice(b"AA\x93");
    let doc = parse_jwc_document(&pooled).unwrap();
    let details = doc.diagnostics[0].decode_details().unwrap();
    assert_eq!(details.byte_offset, 2493);
    assert_eq!(details.byte_length, 3);
    assert_eq!(details.replacement_characters, 1);
}

#[test]
fn truncated_or_corrupt_documents_never_return_partial_entities() {
    for id in ["q070", "r003", "r001", "q038"] {
        let data = fixture(id);
        for end in 0..data.len() {
            assert!(
                parse_jwc_document(&data[..end]).is_err(),
                "{id} prefix {end}"
            );
        }
    }
    let mut late_failure = fixture("r001");
    let offset = parse_jwc_header(&late_failure)
        .unwrap()
        .layout
        .points
        .byte_offset;
    late_failure[offset..offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(parse_jwc_header(&late_failure).is_ok());
    assert!(
        matches!(parse_jwc_document(&late_failure), Err(JwcError::InvalidValue { offset: o, .. }) if o == offset)
    );
    let mut bad_reference = fixture("r001");
    bad_reference[2461..2465].copy_from_slice(&0x4000_ffff_u32.to_le_bytes());
    assert!(matches!(
        parse_jwc_document(&bad_reference),
        Err(JwcError::InvalidValue { offset: 2461, .. })
    ));
    let mut missing_nul = fixture("q030");
    missing_nul[2448] = b'X';
    assert!(matches!(
        parse_jwc_document(&missing_nul),
        Err(JwcError::InvalidValue { offset: 2445, .. })
    ));
    let mut zero_baseline = fixture("q030");
    let start = zero_baseline[2421..2429].to_vec();
    zero_baseline[2429..2437].copy_from_slice(&start);
    assert!(matches!(
        parse_jwc_document(&zero_baseline),
        Err(JwcError::UnsupportedLayout { offset: 2421, .. })
    ));
    assert!(parse_jwc_document(&fixture("r080")).is_err());
}

#[test]
fn file_reader_uses_content_and_bounded_reads() {
    let directory = std::env::temp_dir().join(format!("ezjww-jwc-p3-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("drawing.jww");
    fs::write(&path, fixture("q070")).unwrap();
    assert_eq!(
        read_jwc_document_from_file(&path).unwrap().entities.len(),
        4
    );
    let missing = read_jwc_document_from_file(directory.join("missing")).unwrap_err();
    assert!(matches!(missing, JwcError::Io(_)));
    assert!(missing.source().is_some());
    let oversized = directory.join("large.jwc");
    fs::File::create(&oversized)
        .unwrap()
        .set_len(JWC_MAX_FILE_SIZE as u64 + 1)
        .unwrap();
    assert!(matches!(
        read_jwc_document_from_file(&oversized),
        Err(JwcError::LimitExceeded { offset: 0, .. })
    ));
    fs::write(&path, b"JwwData.").unwrap();
    assert!(matches!(
        read_jwc_document_from_file(&path),
        Err(JwcError::InvalidSignature { .. })
    ));
    fs::remove_dir_all(directory).unwrap();
}
