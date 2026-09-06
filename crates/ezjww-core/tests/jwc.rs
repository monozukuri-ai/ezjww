use ezjww_core::{parse_cad_document, parse_jwc_document, CadDocument};
use std::{fs, path::PathBuf};

#[test]
fn every_truncated_prefix_fails_without_returning_a_partial_document() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../jwc_samples/generated");
    // Mixed geometry, multiple strings/points, and rotated full ellipses cover
    // both fixed records and the variable-length string/name boundaries.
    for name in ["q070", "r001", "r013"] {
        let data = fs::read(root.join(format!("{name}.jwc"))).unwrap();
        assert!(matches!(parse_cad_document(&data), Ok(CadDocument::Jwc(_))));
        for end in 0..data.len() {
            assert!(
                parse_jwc_document(&data[..end]).is_err(),
                "{name}: accepted a truncated prefix of {end} bytes"
            );
            assert!(parse_cad_document(&data[..end]).is_err(), "{name}: {end}");
        }
    }
}
