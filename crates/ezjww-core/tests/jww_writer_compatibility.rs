use ezjww_core::{parse_document_with_diagnostics, to_jww_bytes};
#[path = "../examples/support/writer_cases.rs"]
mod writer_cases;
#[path = "../examples/support/writer_compatibility_cases.rs"]
mod writer_compatibility_cases;

#[test]
fn native_matrix_inputs_roundtrip_without_loss() {
    for &name in writer_compatibility_cases::CASES {
        let doc = writer_compatibility_cases::drawing(name).unwrap();
        let bytes = to_jww_bytes(&doc).unwrap();
        let parsed = parse_document_with_diagnostics(&bytes).unwrap();
        assert!(parsed.diagnostics.is_empty(), "{name}");
        assert_eq!(parsed.document.entities, doc.entities, "{name}");
        assert_eq!(bytes, to_jww_bytes(&doc).unwrap(), "{name}");
    }
}
