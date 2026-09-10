use ezjww_core::schema::cad_document_to_dto;
use ezjww_core::{
    detect_format, parse_cad_document, read_cad_document_from_file, CadDocument, CadError,
    CadFormat,
};
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../jwc_samples/generated")
        .join(name)
}

#[test]
fn common_reader_preserves_source_format_and_typed_payloads() {
    for (name, format) in [("q032.jwc", "jwc"), ("q032rt.jww", "jww")] {
        let data = fs::read(fixture(name)).unwrap();
        let doc = parse_cad_document(&data).unwrap();
        let value = serde_json::to_value(cad_document_to_dto(&doc)).unwrap();
        assert_eq!(value["format"], format);
        assert!(value["document"]["entity_counts"]["TEXT"].as_u64().unwrap() >= 1);
        if let CadDocument::Jwc(doc) = doc {
            assert!(doc.header.source_version.is_none());
            assert!(value["document"]["header"].get("version").is_none());
            assert_eq!(value["document"]["entities"][0]["type"], "text");
            assert_eq!(value["document"]["entities"][0]["content"], "日本語");
        }
    }
}

#[test]
fn common_detection_is_not_validation_and_unknown_formats_fail() {
    assert!(matches!(
        parse_cad_document(b"unknown"),
        Err(CadError::UnknownFormat)
    ));
    let data = b"jw_cad(c)data";
    assert_eq!(detect_format(data), Some(CadFormat::Jwc));
    assert!(matches!(parse_cad_document(data), Err(CadError::Jwc(_))));
    let Err(CadError::Jwc(error)) = read_cad_document_from_file(fixture("r080.jwc")) else {
        panic!()
    };
    assert_eq!(error.byte_offset(), Some(2483));
    // Unverified attribute bits are retained with a diagnostic, not rejected.
    let Ok(CadDocument::Jwc(flagged)) = read_cad_document_from_file(fixture("r011.jwc")) else {
        panic!()
    };
    assert_eq!(flagged.diagnostics[0].code, "JWC_ATTRIBUTE_UNVERIFIED");
}

#[test]
fn common_file_reader_uses_content_and_retains_io_errors() {
    let renamed = std::env::temp_dir().join(format!("ezjww-p5-{}.bin", std::process::id()));
    fs::copy(fixture("q001.jwc"), &renamed).unwrap();
    assert!(matches!(
        read_cad_document_from_file(&renamed),
        Ok(CadDocument::Jwc(_))
    ));
    fs::remove_file(&renamed).unwrap();
    assert!(matches!(
        read_cad_document_from_file(&renamed),
        Err(CadError::Io(_))
    ));
}
