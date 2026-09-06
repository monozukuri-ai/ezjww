//! One source-to-output coordinate transform and explicit rendering defaults.
use std::error::Error;
use std::fmt;

use serde::Serialize;

use super::{JwcDocument, JwcEntityData, JwcError, JwcLayerAddress, JwcSection};
use crate::{Arc, Coord2D, Diagnostic, Entity, EntityBase, Line, Point, Text};

#[derive(Debug)]
#[non_exhaustive]
pub enum JwcConversionError {
    Input(JwcError),
    InvalidOptions(String),
    InvalidDocument {
        entity_index: Option<usize>,
        detail: String,
    },
}

impl fmt::Display for JwcConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(e) => e.fmt(f),
            Self::InvalidOptions(s) => write!(f, "invalid JWC conversion options: {s}"),
            Self::InvalidDocument {
                entity_index,
                detail,
            } => write!(
                f,
                "invalid JWC document at entity {entity_index:?}: {detail}"
            ),
        }
    }
}
impl Error for JwcConversionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(e) => Some(e),
            _ => None,
        }
    }
}
impl From<JwcError> for JwcConversionError {
    fn from(e: JwcError) -> Self {
        Self::Input(e)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JwcCoordinateSpace {
    /// Sheet center at (0,0), +Y up; compatible with existing common JWW entities.
    #[default]
    PaperMillimeters,
    /// Paper coordinates multiplied once by each entity's own group scale.
    ModelMillimeters,
}

impl std::str::FromStr for JwcCoordinateSpace {
    type Err = JwcConversionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "paper_millimeters" => Ok(Self::PaperMillimeters),
            "model_millimeters" => Ok(Self::ModelMillimeters),
            _ => Err(JwcConversionError::InvalidOptions(
                "jwc_coordinates must be paper_millimeters or model_millimeters".into(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JwcNormalizedLayer {
    pub name: String,
    pub frozen: bool,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwcEntityMapping {
    pub source_entity_index: usize,
    pub source_spans: Vec<JwcSection>,
    pub applied_group_scale: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwcConversionNotice {
    pub entity_index: Option<usize>,
    pub field: String,
    /// "default", "derived", or "dxf_limitation"; separate from parse diagnostics.
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwcConversionReport {
    pub coordinate_space: JwcCoordinateSpace,
    /// Rendering policy, never claimed to be a palette embedded in the JWC.
    pub rendering_policy: String,
    pub aci_colors: [i32; 5],
    pub mappings: Vec<JwcEntityMapping>,
    pub notices: Vec<JwcConversionNotice>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwcNormalizedDocument {
    pub entities: Vec<Entity>,
    pub layers: [[JwcNormalizedLayer; 16]; 16],
    pub report: JwcConversionReport,
}

pub(super) fn invalid(index: Option<usize>, detail: &str) -> JwcConversionError {
    JwcConversionError::InvalidDocument {
        entity_index: index,
        detail: detail.into(),
    }
}

fn notice(
    index: Option<usize>,
    field: &str,
    kind: &str,
    detail: impl Into<String>,
) -> JwcConversionNotice {
    JwcConversionNotice {
        entity_index: index,
        field: field.into(),
        kind: kind.into(),
        detail: detail.into(),
    }
}

/// Normalize checked source records to common Entity values. Coordinates are
/// transformed here only; the downstream DXF converter applies no extra scale.
pub fn normalize_jwc_document(
    doc: &JwcDocument,
    space: JwcCoordinateSpace,
) -> Result<JwcNormalizedDocument, JwcConversionError> {
    if doc.entities.len() > super::JWC_MAX_ENTITIES || doc.header.coordinate_extent != 518. {
        return Err(invalid(
            None,
            "unverified coordinate extent or excessive record count",
        ));
    }
    let counts = doc.header.counts;
    if [
        counts.lines,
        counts.arcs,
        counts.texts,
        counts.points,
        counts.temporary_points,
    ]
    .into_iter()
    .map(u64::from)
    .sum::<u64>()
        != doc.entities.len() as u64
    {
        return Err(invalid(None, "header count does not match source records"));
    }
    let (width, height) = doc.header.paper.dimensions_mm();
    let layers = std::array::from_fn(|g| {
        std::array::from_fn(|l| {
            let group = &doc.header.layer_groups[g];
            let layer = &group.layers[l];
            JwcNormalizedLayer {
                name: layer.name.text.clone(),
                frozen: !group.state.visible || !layer.state.visible,
                locked: group.state.protected
                    || layer.state.protected
                    || !group.state.editable
                    || !layer.state.editable,
            }
        })
    });
    let mut report = JwcConversionReport {
        coordinate_space: space,
        rendering_policy: "native_10021_reference_v1".into(),
        aci_colors: [132, 18, 92, 52, 212],
        mappings: Vec::new(), diagnostics: doc.diagnostics.clone(),
        notices: vec![
            notice(None, "palette", "default", "JWC RGB palette is not verified as stored. Pen 1..5 use ACI 132/18/92/52/212 from the native reference DXF configuration."),
            notice(None, "line_patterns", "default", "Source style 1=continuous, 2=equal dash/gap 1.25, 3=equal dash/gap 2.5 output mm. Pattern lengths are rendering defaults, not recovered source lengths."),
            notice(None, "pen_width", "default", "No source line width is available; common pen_width=0 and DTO lineweight=-3. The DXF writer omits the negative lineweight field, so the file uses BYLAYER and the default layer width."),
            notice(None, "font", "default", "No source font is available; common font_name is empty and DXF uses STANDARD/txt with viewer font substitution."),
            notice(None, "layer_style", "default", "No source layer color or line pattern was recovered; DXF layer tables use neutral ACI 7 and CONTINUOUS. Entities carry explicit color and style."),
            notice(None, "non_stored_attributes", "default", "Non-line group IDs are unavailable and use 0. Text and point pen_style use continuous; these are adapter values, not recovered source attributes."),
            notice(None, "blocks", "derived", "The accepted profile completely consumes fixed entity arrays and name slots; it has no block-definition section. No block definitions are generated."),
        ],
    };
    let mut entities = Vec::with_capacity(doc.entities.len());
    let mut next_group = 0;
    let mut open_group = None;
    for (index, record) in doc.entities.iter().enumerate() {
        let layer = match &record.data {
            JwcEntityData::Line { attributes, .. } | JwcEntityData::Arc { attributes, .. } => {
                attributes.layer
            }
            JwcEntityData::Point { layer, .. }
            | JwcEntityData::TemporaryPoint { layer, .. }
            | JwcEntityData::Text { layer, .. } => *layer,
        };
        let JwcLayerAddress {
            group,
            layer: layer_number,
        } = layer;
        let group_header = doc
            .header
            .layer_groups
            .get(usize::from(group))
            .ok_or_else(|| invalid(Some(index), "layer group out of range"))?;
        if layer_number > 15 || !group_header.scale.is_finite() || group_header.scale <= 0. {
            return Err(invalid(Some(index), "invalid layer or group scale"));
        }
        let scale = if space == JwcCoordinateSpace::ModelMillimeters {
            group_header.scale
        } else {
            1.
        };
        let xy = |p: Coord2D| -> Result<Coord2D, JwcConversionError> {
            let result = Coord2D::new(
                (p.x * width / 518. - width / 2.) * scale,
                (p.y * width / 518. - height / 2.) * scale,
            );
            if !result.x.is_finite() || !result.y.is_finite() {
                return Err(invalid(Some(index), "nonfinite coordinate"));
            }
            Ok(result)
        };
        let mut base = EntityBase {
            layer: u16::from(layer_number),
            layer_group: u16::from(group),
            pen_style: 1,
            pen_color: 2,
            ..EntityBase::default()
        };
        match &record.data {
            JwcEntityData::Line { attributes, .. } | JwcEntityData::Arc { attributes, .. } => {
                base.pen_style = match attributes.pen_style {
                    1 => 1,
                    2 => 2,
                    3 => 6,
                    _ => return Err(invalid(Some(index), "unknown stroke style")),
                };
                base.pen_color = u16::from(attributes.pen_color);
                base.flag = attributes.flags_raw;
            }
            JwcEntityData::Point {
                pen_color,
                flags_raw,
                ..
            } => {
                if *flags_raw != 0 {
                    return Err(invalid(Some(index), "unknown point flag"));
                }
                base.pen_color = u16::from(*pen_color);
            }
            JwcEntityData::TemporaryPoint { .. } => {
                report.notices.push(notice(Some(index), "point_style", "default", "Temporary point color is not stored; use pen 2 from the native reference policy. Marker code=0, angle=0, scale=1 are rendering defaults."));
            }
            JwcEntityData::Text { text_preset, .. } => {
                let preset = doc
                    .header
                    .text_presets
                    .get(usize::from(*text_preset))
                    .filter(|_| (1..=10).contains(text_preset))
                    .ok_or_else(|| invalid(Some(index), "unknown text preset"))?;
                base.pen_color = preset.pen_color;
            }
        }
        if !(1..=5).contains(&base.pen_color) {
            return Err(invalid(Some(index), "unknown pen color"));
        }
        if !matches!(record.data, JwcEntityData::Line { .. }) && open_group.is_some() {
            return Err(invalid(Some(index), "unterminated curve group"));
        }
        let entity = match &record.data {
            JwcEntityData::Line { start, end, .. } => {
                match (base.flag, open_group) {
                    (0, None) => {}
                    (0x40, None) => {
                        next_group += 1;
                        open_group = Some(next_group);
                        base.group = next_group;
                    }
                    (0x80, Some(id)) => base.group = id,
                    (0xc0, current) => {
                        base.group = current.unwrap_or_else(|| {
                            next_group += 1;
                            next_group
                        });
                        open_group = None;
                    }
                    _ => return Err(invalid(Some(index), "unknown curve marker sequence")),
                }
                if base.flag != 0 {
                    report.notices.push(notice(Some(index), "group", "derived", "Curve group is a sequential ID derived from boundary markers, not the original authoring ID."));
                }
                let a = xy(*start)?;
                let b = xy(*end)?;
                Entity::Line(Line {
                    base,
                    start_x: a.x,
                    start_y: a.y,
                    end_x: b.x,
                    end_y: b.y,
                })
            }
            JwcEntityData::Arc {
                center,
                radius,
                flatness,
                start_angle_degrees,
                end_angle_degrees,
                tilt_angle_degrees,
                is_full_circle,
                ..
            } => {
                if base.flag != 0
                    || !radius.is_finite()
                    || *radius <= 0.
                    || !flatness.is_finite()
                    || *flatness <= 0.
                    || *flatness > 1.
                    || [
                        *start_angle_degrees,
                        *end_angle_degrees,
                        *tilt_angle_degrees,
                    ]
                    .iter()
                    .any(|a| !a.is_finite() || !(0. ..360.).contains(a))
                    || *is_full_circle != (*start_angle_degrees == 0. && *end_angle_degrees == 0.)
                    || (!is_full_circle
                        && (start_angle_degrees == end_angle_degrees || *flatness != 1.))
                    || (*flatness == 1. && *tilt_angle_degrees != 0.)
                {
                    return Err(invalid(Some(index), "unverified arc geometry"));
                }
                let p = xy(*center)?;
                let radius = radius * width / 518. * scale;
                if !radius.is_finite() {
                    return Err(invalid(Some(index), "radius overflow"));
                }
                Entity::Arc(Arc {
                    base,
                    center_x: p.x,
                    center_y: p.y,
                    radius,
                    flatness: *flatness,
                    start_angle: start_angle_degrees.to_radians(),
                    arc_angle: if *is_full_circle {
                        std::f64::consts::TAU
                    } else {
                        (end_angle_degrees - start_angle_degrees)
                            .rem_euclid(360.)
                            .to_radians()
                    },
                    tilt_angle: tilt_angle_degrees.to_radians(),
                    is_full_circle: *is_full_circle,
                })
            }
            JwcEntityData::Point { position, .. }
            | JwcEntityData::TemporaryPoint { position, .. } => {
                let p = xy(*position)?;
                if matches!(record.data, JwcEntityData::Point { .. }) {
                    report.notices.push(notice(Some(index), "point_symbol", "default", "Source has position/layer/color only; marker code=0, angle=0, scale=1 and continuous pen_style are adapter defaults."));
                }
                Entity::Point(Point {
                    base,
                    x: p.x,
                    y: p.y,
                    is_temporary: matches!(record.data, JwcEntityData::TemporaryPoint { .. }),
                    code: 0,
                    angle: 0.,
                    scale: 1.,
                })
            }
            JwcEntityData::Text {
                start,
                end,
                text_preset,
                content,
                ..
            } => {
                let a = xy(*start)?;
                let b = xy(*end)?;
                let preset = &doc.header.text_presets[usize::from(*text_preset)];
                let size_x = f64::from(preset.width_tenths) / 10. * scale;
                let size_y = f64::from(preset.height_tenths) / 10. * scale;
                let spacing = f64::from(preset.spacing_tenths) / 10. * scale;
                if a == b
                    || content.is_empty()
                    || size_x <= 0.
                    || size_y <= 0.
                    || ![size_x, size_y, spacing].iter().all(|v| v.is_finite())
                {
                    return Err(invalid(Some(index), "invalid text geometry"));
                }
                report.notices.push(notice(Some(index), "text.spacing", "dxf_limitation", format!("Common text retains spacing {spacing}; DXF TEXT has no exact character-spacing field. Font metrics and spacing are not claimed as a native glyph-outline match.")));
                Entity::Text(Text {
                    base,
                    start_x: a.x,
                    start_y: a.y,
                    end_x: b.x,
                    end_y: b.y,
                    text_type: u32::from(*text_preset),
                    size_x,
                    size_y,
                    spacing,
                    angle: (end.y - start.y).atan2(end.x - start.x).to_degrees(),
                    font_name: String::new(),
                    content: content.clone(),
                })
            }
        };
        let mut spans = record.source.spans.clone();
        if let JwcEntityData::Text { string_source, .. } = record.data {
            spans.push(string_source);
        }
        report.mappings.push(JwcEntityMapping {
            source_entity_index: index,
            source_spans: spans,
            applied_group_scale: scale,
        });
        entities.push(entity);
    }
    if open_group.is_some() {
        return Err(invalid(None, "unterminated curve group"));
    }
    Ok(JwcNormalizedDocument {
        entities,
        layers,
        report,
    })
}
