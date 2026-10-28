//! Framing and value variants seen in real DOS-era JWC files (2026-10 corpus of
//! 23 files that the reference-corpus rules rejected). Every variant is rebuilt
//! synthetically from the generated fixtures; no customer file is embedded.

use std::fs;
use std::path::{Path, PathBuf};

use ezjww_core::diagnostics::{
    JWC_ARC_ANGLE_NORMALIZED, JWC_ATTRIBUTE_UNVERIFIED, JWC_ELLIPSE_AXES_SWAPPED,
    JWC_LINE_STYLE_DEFAULTED, JWS_SCHEMA_VERSION_USED,
};
use ezjww_core::jwc::{
    normalize_jwc_document, JwcCoordinateSpace, JwcEntityData, JwcPaper, JWC_MAX_ENTITIES,
};
use ezjww_core::{parse_jwc_document, parse_jwc_header, parse_jws_document, Entity, JwcError};

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

fn set_csv(data: &mut [u8], block: usize, index: usize, value: &str) {
    let end = data[block..block + 199]
        .iter()
        .position(|&b| b == 0)
        .unwrap();
    let mut fields: Vec<String> = std::str::from_utf8(&data[block..block + end])
        .unwrap()
        .split(',')
        .map(str::to_string)
        .collect();
    fields[index] = value.into();
    let text = fields.join(",");
    assert!(text.len() < 199);
    data[block..block + 199].fill(b' ');
    data[block..block + text.len()].copy_from_slice(text.as_bytes());
    data[block + text.len()] = 0;
}

fn attribute_fields(doc: &ezjww_core::JwcDocument) -> Vec<(String, usize)> {
    doc.diagnostics
        .iter()
        .filter(|d| d.code == JWC_ATTRIBUTE_UNVERIFIED)
        .map(|d| {
            let details = d.unverified_details().unwrap();
            (details.field.clone(), details.count)
        })
        .collect()
}

#[test]
fn missing_fixed_lfs_and_csv_terminators_are_reported_not_rejected() {
    // Memo block padded with spaces and no LF at byte 199.
    let mut data = fixture("q001");
    data[199] = b' ';
    let doc = parse_jwc_document(&data).unwrap();
    assert_eq!(doc.entities.len(), 1);
    assert_eq!(attribute_fields(&doc), vec![("header.lf".to_string(), 1)]);

    // csv0 block padded with spaces to the LF at byte 399, no NUL at all.
    let mut data = fixture("q001");
    let end = 200 + data[200..399].iter().position(|&b| b == 0).unwrap();
    data[end..399].fill(b' ');
    let doc = parse_jwc_document(&data).unwrap();
    assert_eq!(doc.header.counts.lines, 1);
    assert_eq!(
        attribute_fields(&doc),
        vec![("header.csv.terminator".to_string(), 1)]
    );
    // A NUL-less block that is not printable ASCII is still corrupt.
    data[300] = 0x01;
    assert!(matches!(
        parse_jwc_document(&data),
        Err(JwcError::InvalidValue { offset: 300, .. })
    ));
}

#[test]
fn short_far_pointers_zeroed_write_layers_and_stale_temporary_slots_are_accepted() {
    // "0:0" instead of "4000:0000" (no texts, so no references depend on it).
    let mut data = fixture("q001");
    set_csv(&mut data, 600, 0, "0:0");
    set_csv(&mut data, 600, 1, "0:0");
    let doc = parse_jwc_document(&data).unwrap();
    assert_eq!(doc.header.layout.string_pool_start, 0);
    assert_eq!(
        attribute_fields(&doc),
        vec![("header.pool_address".to_string(), 2)]
    );

    // Write-layer table left zeroed instead of packed (group << 4 | layer).
    let mut data = fixture("q001");
    data[2405..2421].fill(0);
    let doc = parse_jwc_document(&data).unwrap();
    assert!(doc.header.layer_groups.iter().all(|g| g.write_layer == 0));
    assert_eq!(
        attribute_fields(&doc),
        vec![("header.group.write_layer".to_string(), 15)]
    );

    // Inactive temporary-point slots carry stale memory.
    let mut data = fixture("q001");
    data[800] = 1;
    data[1204 + 40] = 0xff;
    let doc = parse_jwc_document(&data).unwrap();
    assert!(doc.header.temporary_points.is_empty());
    assert_eq!(
        attribute_fields(&doc),
        vec![("header.temporary_points.inactive".to_string(), 2)]
    );
}

#[test]
fn oversize_paper_codes_and_other_coordinate_extents_scale_the_drawing() {
    let mut data = fixture("q001");
    set_csv(&mut data, 200, 11, "8");
    set_csv(&mut data, 200, 30, "678");
    let doc = parse_jwc_document(&data).unwrap();
    assert_eq!(doc.header.paper, JwcPaper::TwoA);
    assert_eq!(doc.header.paper.dimensions_mm(), (1682., 1189.));
    assert_eq!(doc.header.coordinate_extent, 678.);
    assert_eq!(serde_json::to_value(JwcPaper::TwoA).unwrap(), "2A");
    let JwcEntityData::Line { start, .. } = doc.entities[0].data else {
        panic!("expected line")
    };
    let normalized = normalize_jwc_document(&doc, JwcCoordinateSpace::PaperMillimeters).unwrap();
    let Entity::Line(line) = &normalized.entities[0] else {
        panic!("expected line")
    };
    let expected_x = start.x * 1682. / 678. - 841.;
    assert!((line.start_x - expected_x).abs() < 1e-9);
    assert!(normalized
        .report
        .notices
        .iter()
        .any(|n| n.field == "coordinate_extent" && n.detail.contains("678")));
    for (code, paper) in [
        (9, JwcPaper::ThreeA),
        (10, JwcPaper::FourA),
        (11, JwcPaper::FiveA),
        (12, JwcPaper::M10),
        (13, JwcPaper::M50),
        (14, JwcPaper::M100),
    ] {
        assert_eq!(JwcPaper::from_code(code), Some(paper));
    }
    for code in [5, 6, 7, 15, 99] {
        let mut data = fixture("q001");
        set_csv(&mut data, 200, 11, &code.to_string());
        assert!(matches!(
            parse_jwc_header(&data),
            Err(JwcError::UnsupportedLayout { .. })
        ));
    }
}

#[test]
fn pool_relative_text_references_are_resolved() {
    let mut data = fixture("q030");
    let header = parse_jwc_header(&data).unwrap();
    let pool_length = header.layout.string_pool.byte_length;
    assert!(pool_length > 0);
    set_csv(&mut data, 600, 0, "EFBB:0000");
    set_csv(&mut data, 600, 1, &format!("EFBB:{pool_length:04X}"));
    let reference_offset = header.layout.text_records.byte_offset + 16;
    data[reference_offset..reference_offset + 4].copy_from_slice(&0_u32.to_le_bytes());
    let doc = parse_jwc_document(&data).unwrap();
    assert_eq!(doc.header.layout.string_pool_start, 0xEFBB_0000);
    assert_eq!(doc.header.layout.reference_base, 0);
    let original = parse_jwc_document(&fixture("q030")).unwrap();
    let (
        JwcEntityData::Text { content, .. },
        JwcEntityData::Text {
            content: expected, ..
        },
    ) = (&doc.entities[0].data, &original.entities[0].data)
    else {
        panic!("expected texts")
    };
    assert_eq!(content, expected);
    assert!(attribute_fields(&doc).contains(&("text.string_reference".to_string(), 1)));
}

#[test]
fn large_record_counts_and_text_pools_are_within_the_new_limits() {
    // 150,000 lines (the 2026-10 corpus has a 107,000-record survey drawing).
    let base = fixture("q001");
    let mut data = base[..2421].to_vec();
    set_csv(&mut data, 200, 0, "150000");
    for _ in 0..150_000 {
        data.extend_from_slice(&base[2421..2443]);
    }
    data.extend_from_slice(&base[2443..]);
    let doc = parse_jwc_document(&data).unwrap();
    assert_eq!(doc.entities.len(), 150_000);
    assert!(doc.entities.len() <= JWC_MAX_ENTITIES);
    let normalized = normalize_jwc_document(&doc, JwcCoordinateSpace::PaperMillimeters).unwrap();
    assert_eq!(normalized.entities.len(), 150_000);

    // 300 texts of 255 bytes: a 76,800-byte pool (the old limit was 65,535).
    let base = fixture("q030");
    let header = parse_jwc_header(&base).unwrap();
    let records = header.layout.text_records.byte_offset;
    let pool_end = header.layout.string_pool.byte_offset + header.layout.string_pool.byte_length;
    let mut data = base[..2421].to_vec();
    set_csv(&mut data, 200, 2, "300");
    set_csv(&mut data, 600, 0, "4000:0000");
    set_csv(&mut data, 600, 1, "4001:2C00");
    for i in 0..300_u32 {
        let mut record = base[records..records + 24].to_vec();
        record[16..20].copy_from_slice(&(0x4000_0000 + 256 * i).to_le_bytes());
        data.extend_from_slice(&record);
    }
    for _ in 0..300 {
        data.extend(std::iter::repeat_n(b'A', 255));
        data.push(0);
    }
    data.extend_from_slice(&base[pool_end..]);
    let doc = parse_jwc_document(&data).unwrap();
    assert_eq!(doc.header.layout.string_pool.byte_length, 76_800);
    assert_eq!(doc.entities.len(), 300);
    assert!(matches!(
        &doc.entities[299].data,
        JwcEntityData::Text { content, .. } if content.len() == 255
    ));
}

#[test]
fn style_zero_and_flat_ellipses_convert_without_changing_the_geometry() {
    let mut data = fixture("q001");
    data[2437] = 0;
    let doc = parse_jwc_document(&data).unwrap();
    assert_eq!(doc.diagnostics[0].code, JWC_LINE_STYLE_DEFAULTED);
    let normalized = normalize_jwc_document(&doc, JwcCoordinateSpace::PaperMillimeters).unwrap();
    let Entity::Line(line) = &normalized.entities[0] else {
        panic!("expected line")
    };
    assert_eq!(line.base.pen_style, 1);
    assert!(normalized
        .report
        .notices
        .iter()
        .any(|n| n.field == "line_style" && n.kind == "default"));

    // flatness 2.0 with tilt 0 is the same ellipse as flatness 0.5 with tilt 90°
    // and twice the radius; the parametric angles shift by -90°.
    let mut data = fixture("q010");
    data[2433..2435].copy_from_slice(&20000_u16.to_le_bytes());
    data[2435..2439].copy_from_slice(&(30 * 65536_i32).to_le_bytes());
    data[2439..2443].copy_from_slice(&(120 * 65536_i32).to_le_bytes());
    data[2443..2447].copy_from_slice(&0_i32.to_le_bytes());
    let doc = parse_jwc_document(&data).unwrap();
    assert!(doc
        .diagnostics
        .iter()
        .any(|d| d.code == JWC_ELLIPSE_AXES_SWAPPED));
    let JwcEntityData::Arc { radius, .. } = doc.entities[0].data else {
        panic!("expected arc")
    };
    let normalized = normalize_jwc_document(&doc, JwcCoordinateSpace::PaperMillimeters).unwrap();
    let Entity::Arc(arc) = &normalized.entities[0] else {
        panic!("expected arc")
    };
    let (width, _) = doc.header.paper.dimensions_mm();
    assert!((arc.radius - 2. * radius * width / 518.).abs() < 1e-9);
    assert!((arc.flatness - 0.5).abs() < 1e-12);
    assert!((arc.tilt_angle - 90_f64.to_radians()).abs() < 1e-12);
    assert!((arc.start_angle - 300_f64.to_radians()).abs() < 1e-9);
    assert!((arc.arc_angle - 90_f64.to_radians()).abs() < 1e-9);
    assert!(normalized
        .report
        .notices
        .iter()
        .any(|n| n.field == "arc.flatness" && n.detail.contains("swapped")));
    assert!(!doc
        .diagnostics
        .iter()
        .any(|d| d.code == JWC_ARC_ANGLE_NORMALIZED));
}

#[test]
fn jws_records_follow_their_class_schema_when_the_header_version_is_older() {
    // Jw_cad 6 writes header 420 while its records carry schema 600; the native
    // basic_v600 fixture pairs header 600 with schema 700, so lowering its header
    // to 420 reproduces the mismatch with known entities.
    let mut data = fs::read(root().join("jws_samples/native/basic_v600.jws")).unwrap();
    data[200..204].copy_from_slice(&420_u32.to_le_bytes());
    let doc = parse_jws_document(&data).unwrap();
    assert_eq!(doc.header.version, 420);
    assert_eq!(doc.entities.len(), 3);
    assert_eq!(doc.diagnostics[0].code, JWS_SCHEMA_VERSION_USED);
    assert_eq!(doc.diagnostics[0].severity, "info");
    let details = doc.diagnostics[0].unverified_details().unwrap();
    assert_eq!(details.values, vec!["420".to_string(), "700".to_string()]);
    let reference =
        parse_jws_document(&fs::read(root().join("jws_samples/native/basic_v600.jws")).unwrap())
            .unwrap();
    assert_eq!(doc.entities, reference.entities);
}
