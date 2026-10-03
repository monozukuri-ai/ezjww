use ezjww_core::{
    parse_document_with_diagnostics, to_jww_bytes, Coord2D, Entity, EntityBase, JwwWriteDocument,
    JwwWriteError, JwwWriteOptions, Solid,
};

fn line_document() -> JwwWriteDocument {
    let mut doc = JwwWriteDocument::default();
    doc.add_line(Coord2D::new(1.0, 2.0), Coord2D::new(11.0, 12.0));
    doc
}

#[test]
fn empty_document_roundtrip() {
    let doc = JwwWriteDocument::default();
    let bytes = to_jww_bytes(&doc).unwrap();
    let parsed = parse_document_with_diagnostics(&bytes).unwrap();
    assert_eq!(parsed.document.header.version, 700);
    assert_eq!(parsed.document.header.paper_size, 3);
    assert_eq!(parsed.document.header.memo, "");
    assert!(parsed.document.entities.is_empty());
    assert!(parsed.document.block_defs.is_empty());
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(bytes, to_jww_bytes(&doc).unwrap());
}

#[test]
fn native_saved_drawings_keep_the_generated_geometry_and_attributes() {
    use std::path::Path;
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../jww_samples/writer");
    for name in ["empty", "line"] {
        let mut input = JwwWriteDocument::default();
        if name == "line" {
            input.add_line(Coord2D::new(0.0, 0.0), Coord2D::new(100.0, 0.0));
        }
        assert_eq!(
            to_jww_bytes(&input).unwrap(),
            std::fs::read(fixtures.join(format!("{name}.jww"))).unwrap()
        );
        for suffix in ["_saved.jww", "_reopened.jww"] {
            let bytes = std::fs::read(fixtures.join(format!("{name}{suffix}"))).unwrap();
            let parsed = parse_document_with_diagnostics(&bytes).unwrap();
            assert!(parsed.diagnostics.is_empty());
            assert_eq!(parsed.document.header.paper_size, 3);
            assert_eq!(parsed.document.header.memo, "\r\n"); // native normalizes empty memo
            assert!(parsed
                .document
                .header
                .layer_groups
                .iter()
                .all(|group| group.scale == 1.0));
            assert!(parsed.document.block_defs.is_empty());
            let geometry: Vec<_> = parsed.document.entities.into_iter().filter(|entity| {
                !matches!(entity, Entity::Text(text) if ezjww_core::metadata_setting_from_text(text).is_some())
            }).collect();
            assert_eq!(geometry, input.entities, "{name}{suffix}");
        }
    }
}

#[test]
fn matches_independent_existing_empty_fixture() {
    let doc = JwwWriteDocument {
        options: JwwWriteOptions {
            memo: "\r\n".into(),
            paper_size: 1,
            ..JwwWriteOptions::default()
        },
        ..JwwWriteDocument::default()
    };
    assert_eq!(
        to_jww_bytes(&doc).unwrap(),
        include_bytes!("../../../jwc_samples/inputs/q000.jww")
    );
}

#[test]
fn lines_roundtrip_with_shared_class_reference() {
    let mut doc = line_document();
    doc.add_line(Coord2D::new(-10.5, 20.25), Coord2D::new(0.0, -30.0));
    let parsed = parse_document_with_diagnostics(&to_jww_bytes(&doc).unwrap()).unwrap();
    assert_eq!(parsed.document.entities, doc.entities);
    assert!(parsed.diagnostics.is_empty());
}

#[test]
fn extended_entity_count_roundtrip() {
    for count in [65534, 65535, 65536] {
        let mut doc = line_document();
        doc.entities.resize(count, doc.entities[0].clone());
        let parsed = parse_document_with_diagnostics(&to_jww_bytes(&doc).unwrap()).unwrap();
        assert_eq!(parsed.document.entities, doc.entities);
        assert!(parsed.diagnostics.is_empty());
    }
}

#[test]
fn variable_length_unicode_memo_does_not_shift_entity_detection() {
    for memo in ["日本語𠮷\r\n".into(), "あ".repeat(255), "長".repeat(65534)] {
        let mut doc = line_document();
        doc.options.memo = memo.clone();
        let parsed = parse_document_with_diagnostics(&to_jww_bytes(&doc).unwrap()).unwrap();
        assert_eq!(parsed.document.header.memo, memo);
        assert_eq!(parsed.document.entities, doc.entities);
        assert!(parsed.diagnostics.is_empty());
    }
}

#[test]
fn version_700_cp932_memo_is_traversed_by_byte_length() {
    let original = include_bytes!("../../../jwc_samples/inputs/q001.jww");
    let (memo, _, had_errors) = encoding_rs::SHIFT_JIS.encode("日本語");
    assert!(!had_errors);
    let mut bytes = original[..12].to_vec();
    bytes.push(memo.len() as u8);
    bytes.extend_from_slice(&memo);
    bytes.extend_from_slice(&original[20..]); // replace the original Unicode CRLF
    let parsed = parse_document_with_diagnostics(&bytes).unwrap();
    assert_eq!(parsed.document.header.memo, "日本語");
    assert_eq!(
        parsed.document.entities,
        parse_document_with_diagnostics(original)
            .unwrap()
            .document
            .entities
    );
    assert!(parsed.diagnostics.is_empty());
}

#[test]
fn class_tag_in_header_settings_is_not_an_entity_list() {
    let mut bytes = include_bytes!("../../../jwc_samples/inputs/q001.jww").to_vec();
    let expected = parse_document_with_diagnostics(&bytes)
        .unwrap()
        .document
        .entities;
    // First dummy section after the fixed layer tables; a plausible class tag.
    let offset = 28 + 16 * 148;
    let fake = [
        1, 0, 255, 255, 188, 2, 8, 0, b'C', b'D', b'a', b't', b'a', b'S', b'e', b'n',
    ];
    bytes[offset..offset + fake.len()].copy_from_slice(&fake);
    assert_eq!(
        parse_document_with_diagnostics(&bytes)
            .unwrap()
            .document
            .entities,
        expected
    );
}

#[test]
fn original_empty_inputs_are_readable() {
    for bytes in [
        include_bytes!("../../../jwc_samples/inputs/q000.jww").as_slice(),
        include_bytes!("../../../jwc_samples/inputs/q063.jww").as_slice(),
        include_bytes!("../../../jwc_samples/inputs/q064.jww").as_slice(),
    ] {
        let parsed = parse_document_with_diagnostics(bytes).unwrap();
        assert!(parsed.document.entities.is_empty());
        assert!(parsed.diagnostics.is_empty());
    }
}

#[test]
fn truncated_empty_header_and_trailer_are_errors() {
    let bytes = to_jww_bytes(&JwwWriteDocument::default()).unwrap();
    for length in [
        0,
        8,
        12,
        100,
        4000,
        bytes.len() - 8,
        bytes.len() - 6,
        bytes.len() - 1,
    ] {
        assert!(
            parse_document_with_diagnostics(&bytes[..length]).is_err(),
            "accepted {length} bytes"
        );
    }
}

#[test]
fn invalid_options_are_rejected() {
    let mut doc = JwwWriteDocument::default();
    doc.options.version = 600;
    assert_eq!(
        to_jww_bytes(&doc),
        Err(JwwWriteError::UnsupportedVersion(600))
    );
    doc.options.version = 700;
    doc.options.paper_size = 7;
    assert!(to_jww_bytes(&doc)
        .unwrap_err()
        .to_string()
        .starts_with("options.paper_size:"));
    doc.options.paper_size = 3;
    doc.options.memo = "bad\0memo".into();
    assert!(to_jww_bytes(&doc)
        .unwrap_err()
        .to_string()
        .starts_with("options.memo:"));
}

#[test]
fn invalid_coordinates_and_unsupported_attributes_are_rejected() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut doc = line_document();
        if let Entity::Line(line) = &mut doc.entities[0] {
            line.end_y = value;
        }
        assert!(to_jww_bytes(&doc)
            .unwrap_err()
            .to_string()
            .starts_with("entities[0].end_y:"));
    }
    let mut doc = line_document();
    if let Entity::Line(line) = &mut doc.entities[0] {
        line.base.flag = 0x20;
    }
    assert!(to_jww_bytes(&doc)
        .unwrap_err()
        .to_string()
        .starts_with("entities[0].base:"));
}

#[test]
fn unsupported_entity_is_not_silently_dropped() {
    let mut doc = line_document();
    doc.entities.push(Entity::Solid(Solid {
        base: EntityBase::default(),
        point1_x: 0.0,
        point1_y: 0.0,
        point2_x: 0.0,
        point2_y: 0.0,
        point3_x: 0.0,
        point3_y: 0.0,
        point4_x: 0.0,
        point4_y: 0.0,
        color: None,
    }));
    assert!(to_jww_bytes(&doc)
        .unwrap_err()
        .to_string()
        .starts_with("entities[1]:"));
}
