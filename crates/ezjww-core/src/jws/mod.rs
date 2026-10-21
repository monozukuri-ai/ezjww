//! Jw_cad registered symbols. Source coordinates and insertion origin are retained.
mod convert;
mod header;
mod parser;

pub use convert::{convert_jws_document, JwsConversionReport, JwsDxfConversion};
pub use header::{is_jws_signature, parse_jws_header, read_jws_header_from_file, JwsHeader};
pub use parser::{parse_jws_document, read_jws_document_from_file};

use crate::{BlockDef, DecodeDiagnostic, EmbeddedImage, Entity, JwwError};
use serde::Serialize;
use std::{
    error::Error,
    fmt::{Display, Formatter},
};

#[derive(Debug)]
pub enum JwsError {
    Io(std::io::Error),
    Invalid {
        byte_offset: usize,
        reason: String,
    },
    Archive {
        byte_offset: usize,
        source: JwwError,
    },
}
impl JwsError {
    pub(crate) fn invalid(offset: usize, reason: impl Into<String>) -> Self {
        Self::Invalid {
            byte_offset: offset,
            reason: reason.into(),
        }
    }
}
impl Display for JwsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => Display::fmt(e, f),
            Self::Invalid {
                byte_offset,
                reason,
            } => write!(f, "invalid JWS at byte {byte_offset}: {reason}"),
            Self::Archive {
                byte_offset,
                source,
            } => write!(f, "invalid JWS at byte {byte_offset}: {source}"),
        }
    }
}
impl Error for JwsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Archive { source, .. } => Some(source),
            _ => None,
        }
    }
}
impl From<std::io::Error> for JwsError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwsDocument {
    pub header: JwsHeader,
    pub entities: Vec<Entity>,
    pub block_defs: Vec<BlockDef>,
    pub images: Vec<EmbeddedImage>,
    pub diagnostics: Vec<DecodeDiagnostic>,
    /// How the ANSI strings were decoded (see `ParsedJwwDocument::text_encoding`).
    pub text_encoding: crate::reader::TextEncoding,
}
impl JwsDocument {
    pub fn validate_block_references(&self) -> crate::BlockReferenceValidation {
        crate::parser::validate_entity_block_references(&self.entities, &self.block_defs)
    }
}
