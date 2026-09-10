use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use ezjww_core::jwc::{
    JwcHeaderProfile, JwcLayerState, JwcPaper, JWC_FIXED_HEADER_SIZE, JWC_MAX_FILE_SIZE,
};
use ezjww_core::{
    detect_format, is_jwc_signature, parse_jwc_header, read_jwc_header_from_file, CadError,
    CadFormat, JwcError, JwwError,
};

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

fn set_csv(data: &mut [u8], block: usize, index: usize, value: &str) -> usize {
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
    let offset = block + fields[..index].iter().map(|s| s.len() + 1).sum::<usize>();
    let text = fields.join(",");
    assert!(text.len() < 199);
    data[block..block + 199].fill(b' ');
    data[block..block + text.len()].copy_from_slice(text.as_bytes());
    data[block + text.len()] = 0;
    offset
}

#[test]
fn detection_identifies_families_without_certifying_headers() {
    assert_eq!(detect_format(b"JwwData."), Some(CadFormat::Jww));
    assert_eq!(detect_format(b"jw_cad(c)data"), Some(CadFormat::Jwc));
    assert!(is_jwc_signature(b"jw_cad(c)data"));
    assert!(matches!(
        parse_jwc_header(b"jw_cad(c)data"),
        Err(JwcError::UnexpectedEof { .. })
    ));
    for data in [
        b"".as_slice(),
        b"jw_cad(c)dat",
        b"NotJww!!",
        b"JWC_TEMP.TXT",
    ] {
        assert_eq!(detect_format(data), None);
    }
    let mut unknown = fixture("q000");
    unknown[20] = b'z';
    assert_eq!(detect_format(&unknown), Some(CadFormat::Jwc));
    assert!(matches!(
        parse_jwc_header(&unknown),
        Err(JwcError::UnsupportedLayout { offset: 20, .. })
    ));
    assert!(matches!(
        parse_jwc_header(b"JwwData."),
        Err(JwcError::InvalidSignature { offset: 0 })
    ));
    assert_eq!(serde_json::to_value(CadFormat::Jwc).unwrap(), "jwc");
}

#[test]
fn empty_and_mixed_headers_have_exact_counts_and_spans() {
    let empty = parse_jwc_header(&fixture("q000")).unwrap();
    assert_eq!(empty.profile_id, JwcHeaderProfile::Fixed2421Csv32V1);
    assert_eq!(empty.source_version, None);
    assert_eq!(empty.paper, JwcPaper::A1);
    assert_eq!(empty.coordinate_extent, 518.);
    assert_eq!(empty.layout.lines.byte_offset, 2421);
    assert_eq!(empty.layout.names.byte_offset, 2421);
    assert_eq!(empty.layout.names.byte_length, 2304);
    assert_eq!(empty.raw_fixed_header.len(), JWC_FIXED_HEADER_SIZE);
    assert_eq!(empty.layer_groups[0].layers[0].name.text, "0");
    assert!(empty.diagnostics.is_empty());
    let mixed = parse_jwc_header(&fixture("r001")).unwrap();
    assert_eq!((mixed.counts.texts, mixed.counts.points), (3, 1));
    assert_eq!(mixed.layout.text_records.byte_length, 72);
    assert_eq!(mixed.layout.string_pool.byte_offset, 2493);
    assert_eq!(mixed.layout.string_pool.byte_length, 12);
    assert_eq!(mixed.layout.points.byte_offset, 2505);
    assert_eq!(mixed.layout.names.byte_offset, 2517);
}

#[test]
fn every_native_case_has_known_header_values_or_the_recorded_gap_error() {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root().join("jwc_samples/manifest.json")).unwrap())
            .unwrap();
    let mut accepted = 0;
    let mut refused = Vec::new();
    for sample in manifest["samples"].as_array().unwrap() {
        let id = sample["id"].as_str().unwrap();
        let data = fixture(id);
        match parse_jwc_header(&data) {
            Ok(header) => {
                assert_ne!(id, "r080", "long-string gap must not be silently accepted");
                assert_eq!(header.layout.names.byte_offset + 2304, data.len());
                assert_eq!(header.raw_fixed_header, data[..2421]);
                assert!(header.source_version.is_none());
                let cases: serde_json::Value = serde_json::from_slice(
                    &fs::read(
                        root()
                            .join("jwc_samples")
                            .join(sample["case_definition"].as_str().unwrap()),
                    )
                    .unwrap(),
                )
                .unwrap();
                let case = cases
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|c| c["id"] == id)
                    .unwrap();
                let expected_paper = match case["paper_size"].as_u64().unwrap_or(1) {
                    0 => JwcPaper::A0,
                    1 => JwcPaper::A1,
                    2 => JwcPaper::A2,
                    3 => JwcPaper::A3,
                    4 => JwcPaper::A4,
                    other => panic!("paper {other}"),
                };
                assert_eq!(header.paper, expected_paper, "{id}");
                assert_eq!(
                    header.write_layer_group,
                    case["write_layer_group"].as_u64().unwrap_or(0) as u8
                );
                for (index, group) in header.layer_groups.iter().enumerate() {
                    assert_eq!(
                        group.scale,
                        case["scales"][index.to_string()].as_f64().unwrap_or(1.),
                        "{id} group {index}"
                    );
                    assert_eq!(
                        group.write_layer,
                        case["write_layers"][index.to_string()]
                            .as_u64()
                            .unwrap_or(0) as u8,
                        "{id}"
                    );
                }
                accepted += 1;
            }
            Err(error) => {
                assert_eq!(id, "r080", "{id}: {error}");
                assert!(matches!(
                    error,
                    JwcError::UnsupportedLayout { offset: 2483, .. }
                ));
                refused.push(id.to_string());
            }
        }
    }
    assert_eq!(accepted, 79);
    assert_eq!(refused, ["r080"]);
}

#[test]
fn controlled_presets_states_names_and_temporary_layers_are_preserved() {
    for (id, expected) in [
        ("r014", (30, 42, 5, 2)),
        ("r015", (30, 30, 12, 2)),
        ("r016", (30, 30, 5, 5)),
        ("r017", (37, 30, 5, 2)),
    ] {
        let p = parse_jwc_header(&fixture(id)).unwrap().text_presets[3];
        assert_eq!(
            (
                p.width_tenths,
                p.height_tenths,
                p.spacing_tenths,
                p.pen_color
            ),
            expected
        );
    }
    let hidden = parse_jwc_header(&fixture("r018")).unwrap().layer_groups[2].state;
    assert!(!hidden.visible && !hidden.editable && !hidden.protected);
    let display = parse_jwc_header(&fixture("r019")).unwrap().layer_groups[2].state;
    assert!(display.visible && !display.editable);
    assert!(
        parse_jwc_header(&fixture("r020")).unwrap().layer_groups[2]
            .state
            .protected
    );
    assert!(
        parse_jwc_header(&fixture("r021")).unwrap().layer_groups[0].layers[3]
            .state
            .protected
    );
    assert_eq!(
        parse_jwc_header(&fixture("q057")).unwrap().layer_groups[0].layers[0]
            .name
            .text,
        "検証層A"
    );
    assert_eq!(
        parse_jwc_header(&fixture("q058")).unwrap().layer_groups[0]
            .name
            .text,
        "検証群AB"
    );
    let data = fixture("r003");
    let points = parse_jwc_header(&data).unwrap().temporary_points;
    assert_eq!(points.len(), 100);
    for (index, point) in points.iter().enumerate() {
        assert_eq!(usize::from(point.array_index), index + 1);
        assert_eq!(
            usize::from(point.layer_group) * 16 + usize::from(point.layer),
            index
        );
        assert_eq!(
            point.raw_x.to_le_bytes(),
            data[804 + index * 4..808 + index * 4]
        );
        assert_eq!(
            point.raw_y.to_le_bytes(),
            data[1208 + index * 4..1212 + index * 4]
        );
    }
}

#[test]
fn malformed_name_keeps_raw_bytes_and_an_absolute_cp932_diagnostic() {
    let data = fixture("r022");
    let header = parse_jwc_header(&data).unwrap();
    let name = &header.layer_groups[0].layers[0].name;
    assert_eq!(name.text, "ABCDEF\u{fffd}");
    assert_eq!(name.raw_bytes, b"ABCDEF\x93\0");
    assert_eq!(header.diagnostics.len(), 1);
    let diagnostic = &header.diagnostics[0];
    assert_eq!(diagnostic.code, "CP932_DECODE_REPLACED");
    let details = diagnostic.decode_details().unwrap();
    assert_eq!(details.byte_offset, header.layout.names.byte_offset);
    assert_eq!(details.byte_length, 7);
    assert_eq!(details.replacement_characters, 1);
    assert_eq!(details.field, "header.layer_groups[0].layers[0].name");
}

#[test]
fn cp932_names_do_not_switch_encoding_on_unicode_bom_bytes() {
    let mut data = fixture("q000");
    data[2421..2429].copy_from_slice(b"\xff\xfeA\0\0\0\0\0");
    let header = parse_jwc_header(&data).unwrap();
    assert_eq!(
        header.layer_groups[0].layers[0].name.text,
        "\u{fffd}\u{fffd}A"
    );
    assert_eq!(
        header.layer_groups[0].layers[0].name.raw_bytes,
        b"\xff\xfeA\0\0\0\0\0"
    );
    let details = header.diagnostics[0].decode_details().unwrap();
    assert_eq!(details.byte_offset, 2421);
    assert_eq!(details.byte_length, 3);
    assert_eq!(details.replacement_characters, 2);
}

#[test]
fn every_truncated_prefix_is_rejected_without_panicking() {
    for id in ["q000", "r001"] {
        let data = fixture(id);
        for length in 0..data.len() {
            let error = parse_jwc_header(&data[..length]).expect_err("truncated file accepted");
            assert!(
                matches!(error, JwcError::UnexpectedEof { .. }),
                "{id} length={length}: {error}"
            );
            assert!(error.byte_offset().is_some());
        }
    }
}

#[test]
fn unsupported_layouts_are_separate_from_invalid_values() {
    let mut cases = Vec::new();
    let mut data = fixture("q000");
    data[399] = b'\r';
    cases.push((data, 399));
    for (block, index, value) in [(200, 11, "5"), (600, 1, "3fff:0000")] {
        let mut data = fixture("q000");
        let offset = set_csv(&mut data, block, index, value);
        cases.push((data, offset));
    }
    let mut data = fixture("q000");
    data[2407] = 0x34;
    cases.push((data, 2407));
    let mut data = fixture("q000");
    data[804] = 1;
    cases.push((data, 804));
    let mut data = fixture("q000");
    let offset = data.len();
    data.push(0);
    cases.push((data, offset));
    for (data, offset) in cases {
        let error = parse_jwc_header(&data).unwrap_err();
        assert!(
            matches!(error, JwcError::UnsupportedLayout { .. }),
            "{error}"
        );
        assert_eq!(error.byte_offset(), Some(offset));
    }
    for value in ["-1", "NaN", "1.5"] {
        let mut data = fixture("q000");
        let offset = set_csv(&mut data, 200, 0, value);
        assert!(
            matches!(parse_jwc_header(&data), Err(JwcError::InvalidValue { offset: o, .. }) if o == offset)
        );
    }
    for value in [0_f32, -1., f32::NAN, f32::INFINITY] {
        let mut data = fixture("q000");
        data[1797..1801].copy_from_slice(&value.to_le_bytes());
        assert!(matches!(
            parse_jwc_header(&data),
            Err(JwcError::InvalidValue { offset: 1797, .. })
        ));
    }
}

#[test]
fn excessive_counts_and_strings_fail_before_count_based_allocations() {
    for (index, value) in [
        (0, "100001"),
        (0, "4294967295"),
        (0, "999999999999999999999999"),
        (4, "101"),
    ] {
        let mut data = fixture("q000");
        let offset = set_csv(&mut data, 200, index, value);
        assert!(
            matches!(parse_jwc_header(&data), Err(JwcError::LimitExceeded { offset: o, .. }) if o == offset)
        );
    }
    let mut data = fixture("q000");
    set_csv(&mut data, 200, 0, "60000");
    let offset = set_csv(&mut data, 200, 1, "60000");
    assert!(
        matches!(parse_jwc_header(&data), Err(JwcError::LimitExceeded { offset: o, .. }) if o == offset)
    );
    let mut data = fixture("q038");
    // Extend the known 256-byte ASCII string by one byte, and update its pool size.
    data.insert(2421 + 24 + 256, b'X');
    set_csv(&mut data, 600, 1, "4000:0102");
    assert!(matches!(
        parse_jwc_header(&data),
        Err(JwcError::LimitExceeded { offset: 2445, .. })
    ));
}

#[test]
fn bad_string_references_and_terminators_never_consume_names() {
    for pointer in [0x4000_0000_u32, 0x4000_ffff] {
        let mut data = fixture("r000");
        data[2461..2465].copy_from_slice(&pointer.to_le_bytes());
        assert!(matches!(
            parse_jwc_header(&data),
            Err(JwcError::InvalidValue { offset: 2461, .. })
        ));
    }
    let mut data = fixture("r000");
    data[2461..2465].copy_from_slice(&0x4000_0005_u32.to_le_bytes());
    assert!(matches!(
        parse_jwc_header(&data),
        Err(JwcError::UnsupportedLayout { offset: 2461, .. })
    ));
    let mut data = fixture("q030");
    data[2448] = b'X';
    assert!(matches!(
        parse_jwc_header(&data),
        Err(JwcError::InvalidValue { offset: 2445, .. })
    ));
    // A reference below the declared pool segment wraps out of range.
    let mut data = fixture("q030");
    data[2437..2441].copy_from_slice(&0_u32.to_le_bytes());
    assert!(matches!(
        parse_jwc_header(&data),
        Err(JwcError::InvalidValue { offset: 2437, .. })
    ));
    let mut data = fixture("q000");
    data[2421..2429].fill(b'A');
    assert!(matches!(
        parse_jwc_header(&data),
        Err(JwcError::InvalidValue { offset: 2421, .. })
    ));
}

#[test]
fn header_success_does_not_validate_entity_geometry_or_promote_documents() {
    let mut data = fixture("q001");
    data[2421..2425].copy_from_slice(&f32::NAN.to_le_bytes());
    let header = parse_jwc_header(&data).unwrap();
    assert_eq!(header.counts.lines, 1);
    // P3 must reject this geometry. The P2 API returns only metadata and spans.
    assert_eq!(header.layout.lines.byte_length, 22);
}

#[test]
fn file_reader_checks_content_size_and_io_error_sources() {
    let directory = std::env::temp_dir().join(format!("ezjww-jwc-p2-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let renamed = directory.join("renamed.jww");
    fs::write(&renamed, fixture("q001")).unwrap();
    assert_eq!(read_jwc_header_from_file(&renamed).unwrap().counts.lines, 1);
    let disguised = directory.join("disguised.jwc");
    fs::copy(root().join("jww_samples/Test1.jww"), &disguised).unwrap();
    assert!(matches!(
        read_jwc_header_from_file(&disguised),
        Err(JwcError::InvalidSignature { .. })
    ));
    let large = directory.join("large.jwc");
    fs::File::create(&large)
        .unwrap()
        .set_len(JWC_MAX_FILE_SIZE as u64 + 1)
        .unwrap();
    assert!(matches!(
        read_jwc_header_from_file(&large),
        Err(JwcError::LimitExceeded { offset: 0, .. })
    ));
    let error = read_jwc_header_from_file(directory.join("missing.jwc")).unwrap_err();
    assert_eq!(error.byte_offset(), None);
    assert!(error.source().is_some());
    let common: CadError = error.into();
    assert!(matches!(common, CadError::Jwc(_)));
    assert!(common.source().is_some());
    let common: CadError = JwwError::InvalidSignature.into();
    assert!(matches!(common, CadError::Jww(_)));
    assert!(CadError::UnknownFormat.source().is_none());
    fs::remove_dir_all(directory).unwrap();
}

fn to_u16_scale_variant(data: &[u8]) -> Vec<u8> {
    let mut out = data[..1797].to_vec();
    for g in 0..16 {
        let scale = f32::from_le_bytes(data[1797 + 4 * g..1801 + 4 * g].try_into().unwrap());
        out.extend_from_slice(&(scale as u16).to_le_bytes());
    }
    out.extend_from_slice(&data[1861..]);
    out[22] = b'.';
    out
}

#[test]
fn u16_scale_signature_variant_shifts_every_later_offset_by_32_bytes() {
    for id in ["q070", "q050", "r024", "q057"] {
        let reference = parse_jwc_header(&fixture(id)).unwrap();
        let data = to_u16_scale_variant(&fixture(id));
        let header = parse_jwc_header(&data).unwrap_or_else(|error| panic!("{id}: {error}"));
        assert_eq!(header.profile_id, JwcHeaderProfile::Fixed2389U16ScaleV1);
        assert_eq!(header.fixed_header_size, 2389);
        assert_eq!(header.layout.lines.byte_offset, 2389);
        assert_eq!(
            header.layout.names.byte_offset,
            reference.layout.names.byte_offset - 32
        );
        assert_eq!(header.counts, reference.counts);
        assert_eq!(header.text_presets, reference.text_presets);
        for (a, b) in header.layer_groups.iter().zip(&reference.layer_groups) {
            assert_eq!(a.scale, b.scale, "{id}");
            assert_eq!(a.state, b.state, "{id}");
            assert_eq!(a.write_layer, b.write_layer, "{id}");
            assert_eq!(a.name.text, b.name.text, "{id}");
            assert_eq!(
                a.layers.iter().map(|l| &l.name.text).collect::<Vec<_>>(),
                b.layers.iter().map(|l| &l.name.text).collect::<Vec<_>>(),
                "{id}"
            );
        }
        assert!(
            header.diagnostics.is_empty(),
            "{id}: {:?}",
            header.diagnostics
        );
        // A zero u16 scale is substituted with 1 and reported.
        let mut zero = data.clone();
        zero[1797 + 2 * 3..1797 + 2 * 3 + 2].fill(0);
        let header = parse_jwc_header(&zero).unwrap();
        assert_eq!(header.layer_groups[3].scale, 1.);
        assert_eq!(header.diagnostics[0].code, "JWC_GROUP_SCALE_DEFAULTED");
    }
    let mut unknown = fixture("q000");
    unknown[22] = b'x';
    assert!(matches!(
        parse_jwc_header(&unknown),
        Err(JwcError::UnsupportedLayout { offset: 22, .. })
    ));
}

#[test]
fn string_pool_addresses_are_far_pointers_relative_to_the_declared_start() {
    for segment in ["0000", "1234", "6647"] {
        let mut data = fixture("q070");
        set_csv(&mut data, 600, 0, &format!("{segment}:0000"));
        set_csv(&mut data, 600, 1, &format!("{segment}:0004"));
        let reference = 2421 + 22 + 32 + 16;
        let value = u32::from_str_radix(segment, 16).unwrap() << 16;
        data[reference..reference + 4].copy_from_slice(&value.to_le_bytes());
        let header = parse_jwc_header(&data).unwrap_or_else(|error| panic!("{segment}: {error}"));
        assert_eq!(header.layout.string_pool.byte_length, 4);
        assert_eq!(header.layout.string_pool_start(), value);
        assert!(header
            .diagnostics
            .iter()
            .all(|d| d.code != "JWC_ATTRIBUTE_UNVERIFIED"));
    }
    let mut mismatch = fixture("q070");
    set_csv(&mut mismatch, 600, 0, "1234:0000");
    set_csv(&mut mismatch, 600, 1, "1234:0004");
    assert!(matches!(
        parse_jwc_header(&mismatch),
        Err(JwcError::InvalidValue { offset: 2491, .. })
    ));
}

#[test]
fn settings_that_differ_from_the_reference_corpus_are_reported_not_rejected() {
    let mut data = fixture("q000");
    let offset = set_csv(&mut data, 200, 31, "");
    // Remove the final comma entirely to recreate the observed CSV31 shape.
    let comma = offset - 1;
    data[comma] = 0;
    data[comma + 1..399].fill(b' ');
    assert!(parse_jwc_header(&data).is_ok());
    let mut data = fixture("q000");
    set_csv(&mut data, 200, 5, "5");
    set_csv(&mut data, 200, 7, "2");
    set_csv(&mut data, 200, 10, "226");
    set_csv(&mut data, 200, 27, "0");
    set_csv(&mut data, 200, 28, "0");
    set_csv(&mut data, 400, 0, "0.1");
    set_csv(&mut data, 600, 23, "5");
    let header = parse_jwc_header(&data).unwrap();
    assert_eq!((header.write_layer_group, header.write_layer), (14, 2));
    let settings: Vec<_> = header
        .diagnostics
        .iter()
        .filter(|d| d.code == "JWC_HEADER_SETTINGS_UNVERIFIED")
        .map(|d| {
            let details = d.unverified_details().unwrap();
            (details.field.clone(), details.count)
        })
        .collect();
    assert_eq!(
        settings,
        [
            // [28] (view origin Y) depends on the paper and is not a reference constant.
            ("header.csv0".to_string(), 3),
            ("header.csv1".to_string(), 1),
            ("header.csv2".to_string(), 1)
        ]
    );
    assert!(header.diagnostics.iter().all(|d| d.severity == "info"));
    // Layer state bits outside the verified combinations are retained.
    let mut data = fixture("q000");
    data[1861] = 5;
    let header = parse_jwc_header(&data).unwrap();
    assert_eq!(
        header.layer_groups[0].state,
        JwcLayerState {
            editable: true,
            visible: true,
            protected: false
        }
    );
    assert!(header
        .diagnostics
        .iter()
        .any(|d| d.code == "JWC_ATTRIBUTE_UNVERIFIED"
            && d.unverified_details().unwrap().field == "layer.edit"));
}
