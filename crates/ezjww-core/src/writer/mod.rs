//! New version-700 JWW drawings. This is not a lossless editor for parsed documents.
//!
//! The first stage supports empty drawings and ordinary lines using the default
//! pen on layer 0 of group 0. Other entities and attributes are rejected.

mod archive;
mod entities;
mod header;
mod validate;

use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::model::{Coord2D, Entity, EntityBase, Line};

/// Settings supported by the initial writer. Other header settings use the
/// embedded native template; all sixteen layer-group scales are 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JwwWriteOptions {
    pub version: u32,
    pub memo: String,
    /// 0..=4: A0..A4; 8..=14: enlarged paper sizes from jwdatafmt.txt.
    pub paper_size: u32,
}

impl Default for JwwWriteOptions {
    fn default() -> Self {
        Self {
            version: 700,
            memo: String::new(),
            paper_size: 3,
        }
    }
}

/// Input for new drawings. Coordinates are paper millimeters, +Y up.
/// Use this separate type instead of assuming a parsed `JwwDocument` retains
/// every original setting, archive record and embedded image.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JwwWriteDocument {
    pub options: JwwWriteOptions,
    pub entities: Vec<Entity>,
}

impl JwwWriteDocument {
    /// Add a solid, default-pen line on layer 0 in group 0.
    /// Coordinates and attributes are validated by `to_jww_bytes`.
    pub fn add_line(&mut self, start: Coord2D, end: Coord2D) {
        self.entities.push(Entity::Line(Line {
            base: EntityBase {
                pen_style: 1,
                pen_color: 1,
                ..EntityBase::default()
            },
            start_x: start.x,
            start_y: start.y,
            end_x: end.x,
            end_y: end.y,
        }));
    }
}

/// Writer errors include the input field that cannot be encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum JwwWriteError {
    UnsupportedVersion(u32),
    InvalidInput { field: String, reason: String },
    LimitExceeded { field: String, maximum: u64 },
}

impl JwwWriteError {
    pub(super) fn invalid(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidInput {
            field: field.into(),
            reason: reason.into(),
        }
    }
}

impl Display for JwwWriteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported JWW output version {version}; expected 700")
            }
            Self::InvalidInput { field, reason } => write!(f, "{field}: {reason}"),
            Self::LimitExceeded { field, maximum } => {
                write!(f, "{field}: exceeds format limit {maximum}")
            }
        }
    }
}

impl Error for JwwWriteError {}

/// Validate and encode a new JWW drawing, without touching the filesystem.
/// Identical input produces identical bytes.
///
/// ```
/// use ezjww_core::{to_jww_bytes, Coord2D, JwwWriteDocument};
/// let mut drawing = JwwWriteDocument::default();
/// drawing.add_line(Coord2D::new(0.0, 0.0), Coord2D::new(100.0, 0.0));
/// let bytes = to_jww_bytes(&drawing)?;
/// assert!(bytes.starts_with(b"JwwData."));
/// # Ok::<(), ezjww_core::JwwWriteError>(())
/// ```
pub fn to_jww_bytes(document: &JwwWriteDocument) -> Result<Vec<u8>, JwwWriteError> {
    validate::document(document)?;
    let mut archive = archive::ArchiveWriter::new();
    header::write(&mut archive, &document.options)?;
    archive.count(document.entities.len(), "entities")?;
    for entity in &document.entities {
        entities::write(&mut archive, entity)?;
    }
    archive.count(0, "block_defs")?;
    archive.u32(0); // version-700 embedded image count
    Ok(archive.into_bytes())
}
