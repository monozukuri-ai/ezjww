use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum JwwError {
    Io(std::io::Error),
    InvalidSignature,
    /// The signature check failed and the file head is printable ASCII (another
    /// CAD's native format renamed to `.jww`, e.g. HO_CAD).
    InvalidSignatureFound(String),
    UnexpectedEof(&'static str),
    EntityListNotFound,
    UnknownClassPid(u32),
    UnknownEntityClass(String),
    /// A serialized reference to an already-loaded object (MFC object tag), which the
    /// entity list never contains in files written by Jw_cad.
    UnsupportedObjectReference(u32),
}

impl Display for JwwError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::InvalidSignature => write!(f, "invalid JWW signature: expected \"JwwData.\""),
            Self::InvalidSignatureFound(head) => write!(
                f,
                "invalid JWW signature: file starts with {head:?}, expected \"JwwData.\""
            ),
            Self::UnexpectedEof(ctx) => write!(f, "unexpected EOF while reading {ctx}"),
            Self::EntityListNotFound => write!(
                f,
                "could not find entity list in file (empty drawing, or not written by Jw_cad)"
            ),
            Self::UnknownClassPid(pid) => write!(f, "unknown class PID: {pid}"),
            Self::UnknownEntityClass(name) => write!(f, "unknown entity class: {name}"),
            Self::UnsupportedObjectReference(tag) => {
                write!(f, "unsupported object reference tag: {tag}")
            }
        }
    }
}

impl Error for JwwError {}

impl From<std::io::Error> for JwwError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Error envelope for format-independent readers. Existing JWW readers
/// keep returning `JwwError`; they do not acquire new variants or conversions.
#[derive(Debug)]
#[non_exhaustive]
pub enum CadError {
    Io(std::io::Error),
    Jww(JwwError),
    Jwc(crate::jwc::JwcError),
    UnknownFormat,
}

impl Display for CadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => Display::fmt(error, f),
            Self::Jww(error) => Display::fmt(error, f),
            Self::Jwc(error) => Display::fmt(error, f),
            Self::UnknownFormat => f.write_str("unrecognized CAD format"),
        }
    }
}

impl Error for CadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Jww(error) => Some(error),
            Self::Jwc(error) => Some(error),
            Self::UnknownFormat => None,
        }
    }
}

impl From<JwwError> for CadError {
    fn from(error: JwwError) -> Self {
        Self::Jww(error)
    }
}

impl From<crate::jwc::JwcError> for CadError {
    fn from(error: crate::jwc::JwcError) -> Self {
        Self::Jwc(error)
    }
}
