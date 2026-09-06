//! Content-based format candidates, separate from header/document validation.

use serde::Serialize;

use crate::header::is_jww_signature;
use crate::jwc::is_jwc_signature;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CadFormat {
    Jww,
    Jwc,
}

/// Detect a format family from bytes. A match can still be truncated or use an
/// unsupported layout; it is not permission to treat the input as a drawing.
pub fn detect_format(data: &[u8]) -> Option<CadFormat> {
    if is_jww_signature(data) {
        Some(CadFormat::Jww)
    } else if is_jwc_signature(data) {
        Some(CadFormat::Jwc)
    } else {
        None
    }
}
