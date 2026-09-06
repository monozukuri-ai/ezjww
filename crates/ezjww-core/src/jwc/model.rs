//! Decoded JWC records. Coordinates remain in the source coordinate system;
//! color/style/flag numbers remain JWC values. P4 supplies the shared Entity
//! adapter and paper/model-unit conversion without fabricating source metadata.

use std::collections::BTreeMap;

use serde::Serialize;

use super::{JwcHeader, JwcSection};
use crate::{Coord2D, Diagnostic};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum JwcDocumentProfile {
    #[serde(rename = "fixed2421_basic_v1")]
    Fixed2421BasicV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct JwcLayerAddress {
    pub group: u8,
    pub layer: u8,
}

impl JwcLayerAddress {
    pub(super) fn from_packed(value: u8) -> Self {
        Self {
            group: value >> 4,
            layer: value & 15,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JwcEntitySource {
    /// Absolute source spans. Temporary points have separate X/Y/layer spans;
    /// other records have one fixed span. Text strings have their own span.
    pub spans: Vec<JwcSection>,
    /// Concatenation of these spans, not a copy of the entire input file.
    pub raw_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct JwcStrokeAttributes {
    pub pen_style: u8,
    pub pen_color: u8,
    pub layer: JwcLayerAddress,
    /// Source markers retained without inventing JWW curve-group identifiers.
    pub flags_raw: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JwcEntityData {
    Line {
        start: Coord2D,
        end: Coord2D,
        attributes: JwcStrokeAttributes,
    },
    /// Full circles and full ellipses use start=end=0. Partial elliptical arcs
    /// and other equal-angle representations are not supported by this profile.
    Arc {
        center: Coord2D,
        radius: f64,
        flatness: f64,
        start_angle_degrees: f64,
        end_angle_degrees: f64,
        tilt_angle_degrees: f64,
        is_full_circle: bool,
        attributes: JwcStrokeAttributes,
    },
    Point {
        position: Coord2D,
        layer: JwcLayerAddress,
        pen_color: u8,
        flags_raw: u16,
    },
    TemporaryPoint {
        position: Coord2D,
        layer: JwcLayerAddress,
        array_index: u8,
    },
    Text {
        start: Coord2D,
        end: Coord2D,
        layer: JwcLayerAddress,
        /// Index into header.text_presets. No font name is inferred.
        text_preset: u8,
        content: String,
        /// Original CP932 bytes, excluding the NUL terminator.
        raw_content: Vec<u8>,
        /// Absolute string span, including the NUL terminator.
        string_source: JwcSection,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwcEntity {
    pub source: JwcEntitySource,
    #[serde(flatten)]
    pub data: JwcEntityData,
}

impl JwcEntity {
    pub fn entity_type(&self) -> &'static str {
        match self.data {
            JwcEntityData::Line { .. } => "LINE",
            JwcEntityData::Arc {
                is_full_circle: true,
                ..
            } => "CIRCLE",
            JwcEntityData::Arc { .. } => "ARC",
            JwcEntityData::Point { .. } => "POINT",
            JwcEntityData::TemporaryPoint { .. } => "TEMPORARY_POINT",
            JwcEntityData::Text { .. } => "TEXT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwcDocument {
    /// An implementation profile, not an application/version identification.
    pub profile_id: JwcDocumentProfile,
    pub header: JwcHeader,
    /// Temporary points, lines, arcs, texts, then normal points, preserving
    /// order within each source array. Original authoring order is unknown.
    pub entities: Vec<JwcEntity>,
    /// All header and entity diagnostics. Structural failure returns an error
    /// instead of a partial document, even after some records have been read.
    pub diagnostics: Vec<Diagnostic>,
}

impl JwcDocument {
    pub fn entity_counts(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for entity in &self.entities {
            *counts.entry(entity.entity_type()).or_default() += 1;
        }
        counts
    }
}
