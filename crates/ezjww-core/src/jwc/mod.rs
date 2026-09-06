//! JWC header support for the bounded layout established by the P0 corpus.
//!
//! Header success validates metadata and record framing. Document parsing adds
//! bounded entity validation, retaining source units and JWC attributes. Shared
//! Entity normalization and DXF conversion use a separate adapter with explicit
//! coordinate-space and rendering-default reports.

pub mod convert;
pub mod error;
pub mod header;
mod layout;
pub mod model;
pub mod normalize;
pub mod parser;
mod reader;

pub use convert::{
    convert_jwc_document, read_jwc_dxf_from_file, JwcConvertOptions, JwcDxfConversion,
};
pub use error::JwcError;
pub use header::{
    parse_jwc_header, read_jwc_header_from_file, JwcEntityCounts, JwcHeader, JwcHeaderProfile,
    JwcLayer, JwcLayerGroup, JwcLayerState, JwcName, JwcPaper, JwcTemporaryPoint, JwcTextPreset,
};
pub use layout::{JwcLayout, JwcSection};
pub use model::{
    JwcDocument, JwcDocumentProfile, JwcEntity, JwcEntityData, JwcEntitySource, JwcLayerAddress,
    JwcStrokeAttributes,
};
pub use normalize::{
    normalize_jwc_document, JwcConversionError, JwcConversionNotice, JwcConversionReport,
    JwcCoordinateSpace, JwcEntityMapping, JwcNormalizedDocument, JwcNormalizedLayer,
};
pub use parser::{parse_jwc_document, read_jwc_document_from_file};

pub const JWC_FAMILY_SIGNATURE: &[u8; 13] = b"jw_cad(c)data";
pub(crate) const FIXED_SIGNATURE: &[u8; 40] = b"jw_cad(c)data.......a.f.m...............";
pub const JWC_FIXED_HEADER_SIZE: usize = 2421;
pub const JWC_NAMES_SIZE: usize = 2304;
/// Implementation resource limits, not claims about every JWC writer.
pub const JWC_MAX_FILE_SIZE: usize = 64 * 1024 * 1024;
pub const JWC_MAX_ENTITIES: usize = 100_000;
pub const JWC_MAX_TEXT_BYTES: usize = 256;

/// Recognize the 13-byte family prefix, including unsupported or short files.
pub fn is_jwc_signature(data: &[u8]) -> bool {
    data.starts_with(JWC_FAMILY_SIGNATURE)
}
