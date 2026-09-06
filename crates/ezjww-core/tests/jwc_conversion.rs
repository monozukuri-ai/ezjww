use std::fs;
use std::path::Path;

use ezjww_core::jwc::{
    convert_jwc_document, normalize_jwc_document, JwcConversionError, JwcConvertOptions,
    JwcCoordinateSpace, JwcEntityData, JwcLayerAddress,
};
use ezjww_core::{parse_jwc_document, ConvertOptions, DxfEntity, DxfTargetVersion, Entity};

fn fixture(id: &str) -> ezjww_core::JwcDocument {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../jwc_samples/generated")
        .join(format!("{id}.jwc"));
    parse_jwc_document(&fs::read(path).unwrap()).unwrap()
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} != {b}");
}

#[test]
fn source_to_paper_and_model_units_apply_each_group_scale_once() {
    for id in ["q001", "q005", "q050", "q054", "r081"] {
        let source = fixture(id);
        let paper = normalize_jwc_document(&source, JwcCoordinateSpace::PaperMillimeters).unwrap();
        let model = normalize_jwc_document(&source, JwcCoordinateSpace::ModelMillimeters).unwrap();
        let base = paper.entities[0].base();
        let scale = source.header.layer_groups[usize::from(base.layer_group)].scale;
        let factor = source.header.paper.dimensions_mm().0 / 518.;
        match (
            &source.entities[0].data,
            &paper.entities[0],
            &model.entities[0],
        ) {
            (JwcEntityData::Line { start, end, .. }, Entity::Line(p), Entity::Line(m)) => {
                let (w, h) = source.header.paper.dimensions_mm();
                near(p.start_x, start.x * factor - w / 2.);
                near(p.start_y, start.y * factor - h / 2.);
                near(p.end_x, end.x * factor - w / 2.);
                near(p.end_y, end.y * factor - h / 2.);
                near(m.start_x, p.start_x * scale);
                near(m.end_y, p.end_y * scale);
            }
            (JwcEntityData::Arc { radius, .. }, Entity::Arc(p), Entity::Arc(m)) => {
                near(p.radius, radius * factor);
                near(m.radius, p.radius * scale);
            }
            _ => panic!(),
        }
        let converted = convert_jwc_document(
            &source,
            JwcConvertOptions {
                coordinates: JwcCoordinateSpace::ModelMillimeters,
                ..Default::default()
            },
        )
        .unwrap();
        if let (DxfEntity::Line(d), Entity::Line(m)) =
            (&converted.document.entities[0], &model.entities[0])
        {
            near(d.x1, m.start_x);
            near(d.y2, m.end_y);
        }
        assert_eq!(model.report.mappings[0].applied_group_scale, scale);
        assert_eq!(paper.report.mappings[0].applied_group_scale, 1.);
        assert_eq!(paper.report.mappings[0].source_entity_index, 0);
    }
}

#[test]
fn explicit_color_and_style_mapping_matches_native_reference_policy() {
    for (id, color, style) in [
        ("q001", 132, "CONTINUOUS"),
        ("q060", 18, "CONTINUOUS"),
        ("q080", 92, "JWC_DASHED2"),
        ("q061", 132, "JWC_DASHED1"),
        ("q083", 52, "CONTINUOUS"),
        ("q084", 212, "CONTINUOUS"),
    ] {
        let out = convert_jwc_document(&fixture(id), JwcConvertOptions::default()).unwrap();
        let (actual_color, actual_style) = match &out.document.entities[0] {
            DxfEntity::Line(l) => {
                assert_eq!(l.line_weight, -3);
                (l.color, l.line_type.as_str())
            }
            DxfEntity::Point(p) => (p.color, p.line_type.as_str()),
            _ => panic!(),
        };
        assert_eq!((actual_color, actual_style), (color, style));
        assert!(out
            .report
            .notices
            .iter()
            .any(|n| n.field == "palette" && n.kind == "default"));
    }
}

#[test]
fn groups_and_temporary_points_keep_semantics_without_inventing_source_ids() {
    let source = fixture("r024");
    let out = normalize_jwc_document(&source, JwcCoordinateSpace::PaperMillimeters).unwrap();
    assert_eq!(
        out.entities
            .iter()
            .map(|e| e.base().group)
            .collect::<Vec<_>>(),
        [0, 1, 1, 1, 2, 2]
    );
    assert!(out
        .report
        .notices
        .iter()
        .any(|n| n.field == "group" && n.kind == "derived"));
    let source = fixture("r003");
    let out = normalize_jwc_document(&source, JwcCoordinateSpace::PaperMillimeters).unwrap();
    assert!(out
        .entities
        .iter()
        .all(|e| matches!(e,Entity::Point(p) if p.is_temporary)));
    let dxf = convert_jwc_document(&source, JwcConvertOptions::default()).unwrap();
    assert_eq!(dxf.document.entities.len(), 100);
    assert!(dxf
        .document
        .entities
        .iter()
        .all(|e| matches!(e,DxfEntity::Point(p) if p.color==18)));
    assert_eq!(dxf.report.mappings.len(), 100);
    assert!(dxf.document.blocks.is_empty());
}

#[test]
fn layer_and_group_visibility_editability_and_protection_reach_dxf() {
    for (id, group, layer) in [
        ("q053", 0, 3),
        ("q059", 0, 3),
        ("r020", 2, 0),
        ("r021", 0, 3),
        ("r018", 2, 0),
        ("r019", 2, 0),
    ] {
        let source = fixture(id);
        let g = &source.header.layer_groups[group];
        let l = &g.layers[layer];
        let out = convert_jwc_document(&source, JwcConvertOptions::default()).unwrap();
        let d = &out.document.layers[group * 16 + layer];
        assert_eq!(d.frozen, !g.state.visible || !l.state.visible);
        assert_eq!(
            d.locked,
            g.state.protected || l.state.protected || !g.state.editable || !l.state.editable
        );
    }
    let source = fixture("q057");
    let out = convert_jwc_document(&source, JwcConvertOptions::default()).unwrap();
    assert_eq!(out.document.layers[0].name, "検証層A");
    let DxfEntity::Line(line) = &out.document.entities[0] else {
        panic!()
    };
    assert_eq!(line.layer, out.document.layers[0].name);
}

#[test]
fn text_presets_width_rotation_and_missing_font_are_explicit() {
    let mut source = fixture("r014");
    source.header.layer_groups[0].scale = 100.;
    let out = normalize_jwc_document(&source, JwcCoordinateSpace::ModelMillimeters).unwrap();
    let Entity::Text(text) = &out.entities[0] else {
        panic!()
    };
    near(text.size_x, 300.);
    near(text.size_y, 420.);
    near(text.spacing, 50.);
    assert!(text.font_name.is_empty());
    let dxf = convert_jwc_document(
        &source,
        JwcConvertOptions {
            coordinates: JwcCoordinateSpace::ModelMillimeters,
            ..Default::default()
        },
    )
    .unwrap();
    near(dxf.text_width_factors[0], 3. / 4.2);
    assert!(dxf
        .report
        .notices
        .iter()
        .any(|n| n.field == "text.spacing" && n.kind == "dxf_limitation"));
    let text = dxf.to_dxf_string(DxfTargetVersion::Ac1015);
    let lines: Vec<_> = text.lines().collect();
    let start = lines.iter().position(|line| *line == "TEXT").unwrap() + 1;
    let width = lines[start..]
        .chunks_exact(2)
        .find(|pair| pair[0].trim() == "41")
        .unwrap()[1]
        .parse::<f64>()
        .unwrap();
    near(width, 3. / 4.2);
    let rotated = convert_jwc_document(&fixture("q039"), JwcConvertOptions::default()).unwrap();
    let DxfEntity::Text(t) = &rotated.document.entities[0] else {
        panic!()
    };
    assert!((t.rotation - 30.).abs() < 0.001);
    let mut source = fixture("q030");
    if let JwcEntityData::Text {
        start,
        end,
        content,
        ..
    } = &mut source.entities[0].data
    {
        // A setting-like string is still user text on the JWC path.
        content.clone_from(&"Printer_Orientation=1".to_owned());
        start.y = -433.;
        end.y = -433.;
    }
    let dxf = convert_jwc_document(&source, JwcConvertOptions::default()).unwrap();
    assert_eq!(dxf.document.entities.len(), 1);
}

#[test]
fn unsupported_options_and_mutated_documents_return_errors_not_panics() {
    let source = fixture("q001");
    assert!(matches!(
        convert_jwc_document(
            &source,
            JwcConvertOptions {
                dxf: ConvertOptions {
                    explode_inserts: true,
                    max_block_nesting: 0
                },
                ..Default::default()
            }
        ),
        Err(JwcConversionError::InvalidOptions(_))
    ));
    let mut bad = source.clone();
    if let JwcEntityData::Line { start, .. } = &mut bad.entities[0].data {
        start.x = f64::NAN;
    }
    assert!(convert_jwc_document(&bad, JwcConvertOptions::default()).is_err());
    let mut bad = source.clone();
    if let JwcEntityData::Line { attributes, .. } = &mut bad.entities[0].data {
        attributes.layer = JwcLayerAddress {
            group: 16,
            layer: 0,
        };
    }
    assert!(convert_jwc_document(&bad, JwcConvertOptions::default()).is_err());
    let mut bad = fixture("q030");
    if let JwcEntityData::Text { text_preset, .. } = &mut bad.entities[0].data {
        *text_preset = 255;
    }
    assert!(convert_jwc_document(&bad, JwcConvertOptions::default()).is_err());
    let mut bad = source;
    bad.header.layer_groups[0].scale = f64::INFINITY;
    assert!(convert_jwc_document(&bad, JwcConvertOptions::default()).is_err());
}

#[test]
fn both_dxf_versions_and_block_options_preserve_supported_geometry() {
    for id in ["q000", "q070", "r003", "r013"] {
        let source = fixture(id);
        let normal = convert_jwc_document(&source, JwcConvertOptions::default()).unwrap();
        let exploded = convert_jwc_document(
            &source,
            JwcConvertOptions {
                dxf: ConvertOptions {
                    explode_inserts: true,
                    max_block_nesting: 1,
                },
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(normal.document, exploded.document);
        for target in [DxfTargetVersion::Ac1015, DxfTargetVersion::Ac1024] {
            let text = normal.to_dxf_string(target);
            assert!(text.contains(target.acad_version()));
            let lines: Vec<_> = text.lines().map(str::trim).collect();
            let units = lines.iter().position(|line| *line == "$INSUNITS").unwrap();
            assert_eq!(&lines[units + 1..units + 3], &["70", "4"]);
            assert!(text.ends_with("0\nEOF\n"));
        }
    }
}
