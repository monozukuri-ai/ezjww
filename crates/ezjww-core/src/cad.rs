//! Format-aware readers. The existing JWW-only APIs retain their contracts.
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::{
    detect_format, parse_document_with_diagnostics, parse_jwc_document,
    read_document_from_file_with_diagnostics, read_jwc_document_from_file, CadError, CadFormat,
    JwcDocument, ParsedJwwDocument,
};

#[derive(Debug)]
pub enum CadDocument {
    Jww(Box<ParsedJwwDocument>),
    Jwc(Box<JwcDocument>),
}

pub fn parse_cad_document(data: &[u8]) -> Result<CadDocument, CadError> {
    match detect_format(data) {
        Some(CadFormat::Jww) => Ok(CadDocument::Jww(Box::new(parse_document_with_diagnostics(
            data,
        )?))),
        Some(CadFormat::Jwc) => Ok(CadDocument::Jwc(Box::new(parse_jwc_document(data)?))),
        None => Err(CadError::UnknownFormat),
    }
}

/// Read only the signature for detection, then use the format-specific reader
/// (including JWC's bounded file-size checks before allocation).
pub fn detect_file_format(path: impl AsRef<Path>) -> Result<Option<CadFormat>, std::io::Error> {
    let mut signature = Vec::with_capacity(13);
    File::open(path)?.take(13).read_to_end(&mut signature)?;
    Ok(detect_format(&signature))
}

pub fn read_cad_document_from_file(path: impl AsRef<Path>) -> Result<CadDocument, CadError> {
    let path = path.as_ref();
    match detect_file_format(path).map_err(CadError::Io)? {
        Some(CadFormat::Jww) => Ok(CadDocument::Jww(Box::new(
            read_document_from_file_with_diagnostics(path)?,
        ))),
        Some(CadFormat::Jwc) => Ok(CadDocument::Jwc(Box::new(read_jwc_document_from_file(
            path,
        )?))),
        None => Err(CadError::UnknownFormat),
    }
}
