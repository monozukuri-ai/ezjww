//! New version-700 JWW drawings. This is not a lossless editor for parsed documents.
//!
//! Supports native geometry, dimensions and blocks, with editable layer, palette,
//! line type and text preset tables. Unsupported inputs are rejected.

mod archive;
mod convert;
pub use convert::to_write_document;
mod entities;
mod header;
pub mod input;
mod validate;

use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::header::{JwwLineTypes, JwwPalette, LayerGroupHeader, LayerHeader, TextPreset};
use crate::model::{Arc, BlockDef, Coord2D, Entity, EntityBase, Line, Point, Text};
use serde::Serialize;

/// Settings for a new drawing. Other header settings use the embedded template.
/// Layer names are stored verbatim; readers give empty names a display fallback.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwwWriteOptions {
    pub version: u32,
    pub memo: String,
    /// 0..=4: A0..A4; 8..=14: enlarged paper sizes from jwdatafmt.txt.
    pub paper_size: u32,
    pub write_layer_group: u32,
    /// Group/layer states: 0 hidden, 1 visible, 2 editable, 3 current.
    /// Each group's `write_layer` must be its sole state-3 layer, and
    /// `write_layer_group` must be the sole state-3 group. Scale is a positive
    /// denominator (50 means 1:50); changing it never rescales entity values.
    pub layer_groups: [LayerGroupHeader; 16],
    pub palette: Option<JwwPalette>,
    pub line_types: Option<JwwLineTypes>,
    pub text_presets: Option<Vec<TextPreset>>,
}

impl Default for JwwWriteOptions {
    fn default() -> Self {
        Self {
            version: 700,
            memo: String::new(),
            paper_size: 3,
            write_layer_group: 0,
            palette: None,
            line_types: None,
            text_presets: None,
            layer_groups: std::array::from_fn(|g| LayerGroupHeader {
                state: if g == 0 { 3 } else { 2 },
                write_layer: 0,
                scale: 1.0,
                protect: 0,
                name: String::new(),
                layers: std::array::from_fn(|l| LayerHeader {
                    state: if l == 0 { 3 } else { 2 },
                    protect: 0,
                    // Preserve the original native template defaults.
                    name: match (g, l) {
                        (0, 0) => "0".into(),
                        (0, 1) => "Defpoints".into(),
                        _ => String::new(),
                    },
                }),
            }),
        }
    }
}

/// Input for new drawings. Coordinates are paper millimeters, origin at the
/// paper center, +Y up. Builders use degrees; raw `Arc` fields use radians.
/// Use this separate type instead of assuming a parsed `JwwDocument` retains
/// every original setting, archive record and embedded image.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct JwwWriteDocument {
    pub options: JwwWriteOptions,
    pub entities: Vec<Entity>,
    pub block_defs: Vec<BlockDef>,
}

impl JwwWriteDocument {
    /// Add a line on layer 0 in group 0. Edit the returned entity for attributes.
    /// Coordinates and attributes are validated by `to_jww_bytes`.
    pub fn add_line(&mut self, start: Coord2D, end: Coord2D) -> &mut Line {
        self.entities.push(Entity::Line(Line {
            base: default_base(),
            start_x: start.x,
            start_y: start.y,
            end_x: end.x,
            end_y: end.y,
        }));
        let Some(Entity::Line(line)) = self.entities.last_mut() else {
            unreachable!()
        };
        line
    }

    /// Add a circle. Edit the returned entity to set layer/pen attributes.
    pub fn add_circle(&mut self, center: Coord2D, radius: f64) -> &mut Arc {
        let arc = self.add_arc(center, radius, 0.0, 360.0);
        arc.is_full_circle = true;
        arc
    }

    /// Add a counterclockwise circular arc. Angles are degrees: start in
    /// [0, 360), sweep in (0, 360). Use `add_circle` for a full circle.
    /// Values are validated at serialization, without silent normalization.
    pub fn add_arc(
        &mut self,
        center: Coord2D,
        radius: f64,
        start_degrees: f64,
        sweep_degrees: f64,
    ) -> &mut Arc {
        self.entities.push(Entity::Arc(Arc {
            base: default_base(),
            center_x: center.x,
            center_y: center.y,
            radius,
            start_angle: start_degrees.to_radians(),
            arc_angle: sweep_degrees.to_radians(),
            tilt_angle: 0.0,
            flatness: 1.0,
            is_full_circle: false,
        }));
        let Some(Entity::Arc(arc)) = self.entities.last_mut() else {
            unreachable!()
        };
        arc
    }

    /// Add an ordinary permanent point (no marker code or temporary flag).
    pub fn add_point(&mut self, position: Coord2D) -> &mut Point {
        self.entities.push(Entity::Point(Point {
            base: default_base(),
            x: position.x,
            y: position.y,
            is_temporary: false,
            code: 0,
            angle: 0.0,
            scale: 0.0,
        }));
        let Some(Entity::Point(point)) = self.entities.last_mut() else {
            unreachable!()
        };
        point
    }

    /// Add plain text with explicit baseline endpoints, 3 mm width/height,
    /// zero spacing/rotation and MS Gothic. No font measurement is performed.
    /// Edit the returned `Text` to set sizes, font and angle (degrees).
    pub fn add_text(
        &mut self,
        start: Coord2D,
        end: Coord2D,
        content: impl Into<String>,
    ) -> &mut Text {
        self.entities.push(Entity::Text(Text {
            base: default_base(),
            start_x: start.x,
            start_y: start.y,
            end_x: end.x,
            end_y: end.y,
            text_type: 0,
            size_x: 3.0,
            size_y: 3.0,
            spacing: 0.0,
            angle: 0.0,
            font_name: "ＭＳ ゴシック".into(),
            content: content.into(),
        }));
        let Some(Entity::Text(text)) = self.entities.last_mut() else {
            unreachable!()
        };
        text
    }
}

fn default_base() -> EntityBase {
    EntityBase {
        pen_style: 1,
        pen_color: 1,
        ..EntityBase::default()
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
    archive.count(document.block_defs.len(), "block_defs")?;
    for definition in &document.block_defs {
        entities::block_definition(&mut archive, definition)?;
    }
    archive.u32(0); // version-700 embedded image count
    Ok(archive.into_bytes())
}
