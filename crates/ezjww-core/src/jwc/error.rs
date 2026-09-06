use std::error::Error;
use std::fmt::{Display, Formatter};

/// Structural errors use absolute offsets in the original file. I/O errors
/// have no meaningful byte offset. Unsupported layouts are not called corrupt.
#[derive(Debug)]
#[non_exhaustive]
pub enum JwcError {
    Io(std::io::Error),
    InvalidSignature {
        offset: usize,
    },
    UnexpectedEof {
        offset: usize,
        field: String,
        needed: usize,
        available: usize,
    },
    InvalidValue {
        offset: usize,
        field: String,
        detail: String,
    },
    UnsupportedLayout {
        offset: usize,
        field: String,
        detail: String,
    },
    LimitExceeded {
        offset: usize,
        field: String,
        detail: String,
    },
}

impl JwcError {
    pub fn byte_offset(&self) -> Option<usize> {
        match self {
            Self::Io(_) => None,
            Self::InvalidSignature { offset }
            | Self::UnexpectedEof { offset, .. }
            | Self::InvalidValue { offset, .. }
            | Self::UnsupportedLayout { offset, .. }
            | Self::LimitExceeded { offset, .. } => Some(*offset),
        }
    }

    pub(crate) fn invalid(offset: usize, field: &str, detail: impl Into<String>) -> Self {
        Self::InvalidValue {
            offset,
            field: field.into(),
            detail: detail.into(),
        }
    }

    pub(crate) fn unsupported(offset: usize, field: &str, detail: impl Into<String>) -> Self {
        Self::UnsupportedLayout {
            offset,
            field: field.into(),
            detail: detail.into(),
        }
    }

    pub(crate) fn limit(offset: usize, field: &str, detail: impl Into<String>) -> Self {
        Self::LimitExceeded {
            offset,
            field: field.into(),
            detail: detail.into(),
        }
    }
}

impl Display for JwcError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "JWC I/O error: {error}"),
            Self::InvalidSignature { offset } => write!(f, "invalid JWC signature at byte {offset}"),
            Self::UnexpectedEof { offset, field, needed, available } => write!(
                f, "unexpected JWC EOF at byte {offset} ({field}): need {needed} bytes, have {available}"
            ),
            Self::InvalidValue { offset, field, detail } => write!(
                f, "invalid JWC value at byte {offset} ({field}): {detail}"
            ),
            Self::UnsupportedLayout { offset, field, detail } => write!(
                f, "unsupported JWC layout at byte {offset} ({field}): {detail}"
            ),
            Self::LimitExceeded { offset, field, detail } => write!(
                f, "JWC limit exceeded at byte {offset} ({field}): {detail}"
            ),
        }
    }
}

impl Error for JwcError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for JwcError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
