use ezjww_core::{parse_document, to_jww_bytes, writer::input, Coord2D, Entity, JwwWriteDocument};
use serde_json::json;

fn line() -> serde_json::Value {
    let mut d = JwwWriteDocument::default();
    d.add_line(Coord2D::new(1.0, 2.0), Coord2D::new(4.0, 8.0));
    serde_json::to_value(&d.entities[0]).unwrap()
}

#[test]
fn palette_line_tables_and_text_presets_survive_variable_length_headers() {
    let mut value = input::new_document();
    value["options"]["memo"] = json!("図面𠮷");
    value["options"]["layer_groups"][15]["name"] = json!("非常に長いグループ名".repeat(50));
    value["options"]["palette"]["extended_colors"][256] = json!(0x123456);
    value["options"]["line_types"]["sxf"][17]["name"] = json!("線種𠮷".repeat(100));
    value["options"]["line_types"]["sxf"][17]["segments_mm"] = json!([6.0, 2.0]);
    value["options"]["text_presets"][9]["size_x"] = json!(13.0);
    for style in 30..=62 {
        let mut l = line();
        l["base"]["pen_style"] = json!(style);
        l["base"]["pen_color"] = json!(356);
        value["entities"].as_array_mut().unwrap().push(l);
    }
    let document = input::from_value(value).unwrap();
    let bytes = to_jww_bytes(&document).unwrap();
    let parsed = parse_document(&bytes).unwrap();
    assert_eq!(parsed.entities, document.entities);
    assert_eq!(parsed.header.palette, document.options.palette);
    assert_eq!(parsed.header.line_types, document.options.line_types);
    assert_eq!(parsed.header.text_presets, document.options.text_presets);
}

#[test]
fn ellipse_fields_are_not_approximated() {
    let mut d = JwwWriteDocument::default();
    let arc = d.add_arc(Coord2D::new(10.0, 20.0), 30.0, 20.0, 250.0);
    arc.flatness = 0.3;
    arc.tilt_angle = 30_f64.to_radians();
    assert_eq!(
        parse_document(&to_jww_bytes(&d).unwrap()).unwrap().entities,
        d.entities
    );
}

#[test]
fn nested_blocks_share_archive_class_ids_and_reject_cycles() {
    let mut v = input::new_document();
    let base = line()["base"].clone();
    let reference = |number| {
        json!({"type":"BLOCK","base":base,"ref_x":1.0,"ref_y":2.0,
        "scale_x":0.01,"scale_y":0.01,"rotation":0.5,"def_number":number})
    };
    v["entities"] = json!([line(), reference(1)]);
    v["block_defs"] = json!([
        {"base":base,"number":0,"is_referenced":true,"name":"part","entities":[line()]},
        {"base":base,"number":1,"is_referenced":true,"name":"assembly","entities":[reference(0),line()]}
    ]);
    let d = input::from_value(v.clone()).unwrap();
    let parsed = parse_document(&to_jww_bytes(&d).unwrap()).unwrap();
    assert_eq!(parsed.entities, d.entities);
    assert_eq!(parsed.block_defs, d.block_defs);
    v["block_defs"][0]["entities"] = json!([reference(1)]);
    let d = input::from_value(v).unwrap();
    assert!(to_jww_bytes(&d).unwrap_err().to_string().contains("cyclic"));
}

#[test]
fn invalid_color_tables_and_rgb_fail_without_panics() {
    for color in [json!(-1), json!(true), json!(16777216), json!("red")] {
        let mut v = input::new_document();
        let mut l = line();
        l["color"] = color;
        v["entities"] = json!([l]);
        assert!(input::from_value(v).is_err());
    }
    let mut v = input::new_document();
    v["options"]["palette"]["extended_colors"] = json!([]);
    assert!(input::from_value(v).is_err());
}

#[test]
fn solid_payload_order_and_conditional_color_match_reader() {
    let mut v = input::new_document();
    let mut base = line()["base"].clone();
    base["pen_color"] = json!(10);
    v["entities"] = json!([{"type":"SOLID","base":base,"point1_x":1.0,"point1_y":2.0,
        "point2_x":5.0,"point2_y":8.0,"point3_x":1.0,"point3_y":8.0,
        "point4_x":5.0,"point4_y":2.0,"color":0xabcdef}]);
    let d = input::from_value(v).unwrap();
    let parsed = parse_document(&to_jww_bytes(&d).unwrap()).unwrap();
    assert_eq!(parsed.entities, d.entities);
    assert!(matches!(parsed.entities[0], Entity::Solid(_)));
}
