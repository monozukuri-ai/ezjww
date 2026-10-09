use ezjww_core::{
    image_reference_content, parse_document, parse_document_with_diagnostics,
    parse_image_reference, to_jww_bytes, writer::input, Coord2D, Entity, JwwWriteDocument,
    IMAGE_DEFAULT_EXTRA, IMAGE_LIST_TRUNCATED,
};
use serde_json::json;

fn gzip_like(payload: &[u8]) -> Vec<u8> {
    // The archive stores whatever bytes Jw_cad wrote; the writer only checks the
    // gzip magic for `.gz` names and never decompresses.
    let mut bytes = vec![0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0b];
    bytes.extend_from_slice(payload);
    bytes
}

fn base() -> serde_json::Value {
    json!({"group": 0, "pen_style": 1, "pen_color": 1, "pen_width": 0, "layer": 0, "layer_group": 0, "flag": 0})
}

#[test]
fn embedded_images_round_trip_through_the_archive() {
    let mut d = JwwWriteDocument::default();
    d.add_image("logo.bmp.gz", gzip_like(b"BM compressed bitmap"));
    d.add_image("raw.png", b"\x89PNG\r\n\x1a\nraw".to_vec());
    let text = d.add_image_reference("logo.bmp", Coord2D::new(10.0, 20.0), 100.0, 64.5161);
    text.base.layer = 3;
    d.add_image_reference("raw.png", Coord2D::new(-5.0, 0.0), 30.0, 20.0);
    d.add_text(Coord2D::new(0.0, 0.0), Coord2D::new(10.0, 0.0), "plain");

    let bytes = to_jww_bytes(&d).unwrap();
    let parsed = parse_document(&bytes).unwrap();
    assert_eq!(parsed.images, d.images);
    assert_eq!(parsed.entities, d.entities);
    assert!(parsed.images[0].is_compressed());
    assert_eq!(parsed.images[0].reference_name(), "logo.bmp");
    assert!(!parsed.images[1].is_compressed());
    assert_eq!(parsed.images[1].reference_name(), "raw.png");

    let Entity::Text(text) = &parsed.entities[0] else {
        panic!("expected the image placement text")
    };
    assert_eq!(
        text.content,
        "^@BM%temp%logo.bmp,100,64.5161,0,0,1,0,255,255,255"
    );
    assert_eq!(text.base.layer, 3);
    assert_eq!((text.size_x, text.size_y), (2.0, 2.0));
    assert_eq!((text.end_x, text.end_y), (110.0, 20.0));
    let reference = text.image_reference().unwrap();
    assert_eq!(reference.path, "%temp%logo.bmp");
    assert_eq!(reference.file_name, "logo.bmp");
    assert_eq!((reference.width, reference.height), (100.0, 64.5161));
    assert_eq!(reference.extra, IMAGE_DEFAULT_EXTRA);
    assert!(reference.is_embedded());
    let Entity::Text(plain) = &parsed.entities[2] else {
        panic!()
    };
    assert!(plain.image_reference().is_none());
}

#[test]
fn files_without_an_archive_still_parse_with_no_images() {
    let mut d = JwwWriteDocument::default();
    d.add_line(Coord2D::new(0.0, 0.0), Coord2D::new(10.0, 0.0));
    let bytes = to_jww_bytes(&d).unwrap();
    // Same trailer as 0.5: a zero image count.
    assert!(bytes.ends_with(&[0, 0, 0, 0]));
    assert!(parse_document(&bytes).unwrap().images.is_empty());
    // Files that stop right after the block list (older writers) still read.
    let parsed = parse_document_with_diagnostics(&bytes[..bytes.len() - 4]).unwrap();
    assert!(parsed.document.images.is_empty());
    assert!(parsed.diagnostics.is_empty());
}

#[test]
fn truncated_archive_keeps_the_images_read_before_the_error() {
    let mut d = JwwWriteDocument::default();
    d.add_image("a.bmp", vec![1, 2, 3]);
    d.add_image("b.bmp", vec![4, 5, 6, 7, 8]);
    let bytes = to_jww_bytes(&d).unwrap();
    let parsed = parse_document_with_diagnostics(&bytes[..bytes.len() - 2]).unwrap();
    assert_eq!(parsed.document.images.len(), 1);
    assert_eq!(parsed.document.images[0].name, "a.bmp");
    let diagnostic = parsed
        .diagnostics
        .iter()
        .find(|d| d.code == IMAGE_LIST_TRUNCATED)
        .expect("image list diagnostic");
    assert_eq!(diagnostic.severity, "warning");
    let details = diagnostic.image_list_details().unwrap();
    assert_eq!((details.expected_images, details.parsed_images), (2, 1));
    assert!(details.byte_offset > 0);
}

#[test]
fn validation_covers_references_names_and_payloads() {
    let mut d = JwwWriteDocument::default();
    d.add_image_reference("missing.bmp", Coord2D::new(0.0, 0.0), 10.0, 10.0);
    let error = to_jww_bytes(&d).unwrap_err().to_string();
    assert!(
        error.starts_with("entities[0].content") && error.contains("not embedded"),
        "{error}"
    );

    // Links to files outside the archive are kept as the user wrote them.
    let mut d = JwwWriteDocument::default();
    d.add_text(
        Coord2D::new(0.0, 0.0),
        Coord2D::new(10.0, 0.0),
        image_reference_content("C:\\images\\logo.bmp", 10.0, 5.0, &[]),
    );
    to_jww_bytes(&d).unwrap();

    for (content, reason) in [
        ("^@BMlogo.bmp", "expected ^@BM"),
        ("^@BMlogo.bmp,0,10,0", "positive image width"),
        ("^@BZsomething", "control text"),
    ] {
        let mut d = JwwWriteDocument::default();
        d.add_text(Coord2D::new(0.0, 0.0), Coord2D::new(10.0, 0.0), content);
        let error = to_jww_bytes(&d).unwrap_err().to_string();
        assert!(error.contains(reason), "{content}: {error}");
    }

    let mut d = JwwWriteDocument::default();
    d.add_image("x.bmp.gz", vec![1, 2, 3]);
    assert!(to_jww_bytes(&d).unwrap_err().to_string().contains("gzip"));
    let mut d = JwwWriteDocument::default();
    d.add_image("dup.bmp", vec![1]);
    d.add_image("DUP.bmp", vec![2]);
    assert!(to_jww_bytes(&d)
        .unwrap_err()
        .to_string()
        .contains("duplicate"));
    let mut d = JwwWriteDocument::default();
    d.add_image("dir/x.bmp", vec![1]);
    assert!(to_jww_bytes(&d)
        .unwrap_err()
        .to_string()
        .contains("bare file name"));
    let mut d = JwwWriteDocument::default();
    d.add_image("empty.bmp", Vec::new());
    assert!(to_jww_bytes(&d)
        .unwrap_err()
        .to_string()
        .contains("image bytes"));
}

#[test]
fn json_input_takes_base64_images_and_conversion_keeps_them() {
    let mut value = input::new_document();
    assert_eq!(value["images"], json!([]));
    value["images"] = json!([{"name": "logo.bmp", "data": ezjww_core::base64::encode(b"BM12345")}]);
    value["entities"] = json!([{
        "type": "TEXT", "base": base(), "start_x": 0.0, "start_y": 0.0, "end_x": 10.0, "end_y": 0.0,
        "text_type": 0, "size_x": 2.0, "size_y": 2.0, "spacing": 0.0, "angle": 0.0,
        "font_name": "ＭＳ ゴシック", "content": "^@BM%temp%logo.bmp,10,5,0,0,1,0,255,255,255"
    }]);
    let d = input::from_value(value).unwrap();
    assert_eq!(d.images[0].data, b"BM12345");

    let parsed = parse_document(&to_jww_bytes(&d).unwrap()).unwrap();
    let dto = serde_json::to_value(ezjww_core::jww_document_to_dto(&parsed)).unwrap();
    assert_eq!(dto["images"][0]["name"], "logo.bmp");
    assert_eq!(
        dto["images"][0]["data"],
        ezjww_core::base64::encode(b"BM12345")
    );
    assert_eq!(dto["images"][0]["compressed"], false);

    let converted = ezjww_core::writer::to_write_document(dto, false).unwrap();
    assert!(converted["document"].is_object(), "{converted}");
    let again = input::from_value(converted["document"].clone()).unwrap();
    assert_eq!(again.images, d.images);
    assert_eq!(again.entities, d.entities);

    let mut bad = input::new_document();
    bad["images"] = json!([{"name": "a.bmp", "data": "QUJD", "compressed": true}]);
    assert!(input::from_value(bad).is_err());
    let mut bad = input::new_document();
    bad["images"] = json!([{"name": "a.bmp", "data": "not base64!"}]);
    assert!(input::from_value(bad).is_err());
}

#[test]
fn image_reference_parsing_keeps_unknown_parameters() {
    let reference =
        parse_image_reference("^@BMC:\\pictures\\site plan.png,100,129.032,0,0,1,0,255,255,255")
            .unwrap();
    assert_eq!(reference.path, "C:\\pictures\\site plan.png");
    assert_eq!(reference.file_name, "site plan.png");
    assert_eq!((reference.width, reference.height), (100.0, 129.032));
    assert_eq!(reference.extra, ["0", "0", "1", "0", "255", "255", "255"]);
    assert!(!reference.is_embedded());
    assert!(parse_image_reference("%TEMP%X.bmp,1,1").is_none());
    assert!(parse_image_reference("^@BM%TEMP%X.bmp,1,1")
        .unwrap()
        .is_embedded());
    for content in ["plain", "^@BMx", "^@BMx,abc,1", "^@BM,1,1", "^@BMx,1,inf"] {
        assert!(parse_image_reference(content).is_none(), "{content}");
    }
    assert_eq!(
        image_reference_content("%temp%a.bmp", 12.5, 7.0, &["0", "0"]),
        "^@BM%temp%a.bmp,12.5,7,0,0"
    );
}
