use std::path::Path;

use super::header::read_jwc_bytes;
use super::model::{
    JwcDocument, JwcDocumentProfile, JwcEntity, JwcEntityData, JwcEntitySource, JwcLayerAddress,
    JwcStrokeAttributes,
};
use super::reader::Reader;
use super::{parse_jwc_header, JwcError, JwcHeader, JwcSection};
use crate::Coord2D;

fn xy(reader: &Reader<'_>, offset: usize, field: &str) -> Result<Coord2D, JwcError> {
    Ok(Coord2D::new(
        f64::from(reader.finite_f32(offset, field)?),
        f64::from(reader.finite_f32(offset + 4, field)?),
    ))
}

fn zero(reader: &Reader<'_>, offset: usize, length: usize, field: &str) -> Result<(), JwcError> {
    if let Some(index) = reader
        .bytes(offset, length, field)?
        .iter()
        .position(|&b| b != 0)
    {
        return Err(JwcError::unsupported(
            offset + index,
            field,
            "unverified attribute bits",
        ));
    }
    Ok(())
}

fn color(value: u16, offset: usize) -> Result<(), JwcError> {
    if !(1..=5).contains(&value) {
        return Err(JwcError::unsupported(
            offset,
            "entity.pen_color",
            "unverified color number",
        ));
    }
    Ok(())
}

fn stroke(reader: &Reader<'_>, offset: usize) -> Result<JwcStrokeAttributes, JwcError> {
    let bytes = reader.bytes(offset, 6, "entity.attributes")?;
    if !(1..=3).contains(&bytes[0]) {
        return Err(JwcError::unsupported(
            offset,
            "entity.pen_style",
            "unverified line style",
        ));
    }
    color(u16::from(bytes[1]), offset + 1)?;
    zero(reader, offset + 3, 1, "entity.attributes")?;
    Ok(JwcStrokeAttributes {
        pen_style: bytes[0],
        pen_color: bytes[1],
        layer: JwcLayerAddress::from_packed(bytes[2]),
        flags_raw: reader.u16(offset + 4, "entity.flags")?,
    })
}

fn source(reader: &Reader<'_>, spans: &[(usize, usize)]) -> Result<JwcEntitySource, JwcError> {
    let mut result = JwcEntitySource {
        spans: Vec::new(),
        raw_bytes: Vec::new(),
    };
    for &(byte_offset, byte_length) in spans {
        result.raw_bytes.extend_from_slice(reader.bytes(
            byte_offset,
            byte_length,
            "entity.source",
        )?);
        result.spans.push(JwcSection {
            byte_offset,
            byte_length,
        });
    }
    Ok(result)
}

fn angle(reader: &Reader<'_>, offset: usize) -> Result<f64, JwcError> {
    let raw = reader.u32(offset, "arc.angle")? as i32;
    if !(0..360 * 65536).contains(&raw) {
        return Err(JwcError::unsupported(
            offset,
            "arc.angle",
            "angle outside verified [0, 360) degrees",
        ));
    }
    Ok(f64::from(raw) / 65536.)
}

fn arc(reader: &Reader<'_>, offset: usize) -> Result<JwcEntityData, JwcError> {
    let center = xy(reader, offset, "arc.center")?;
    let radius = f64::from(reader.finite_f32(offset + 8, "arc.radius")?);
    if radius <= 0. {
        return Err(JwcError::invalid(
            offset + 8,
            "arc.radius",
            "radius must be positive",
        ));
    }
    let flatness_raw = reader.u16(offset + 12, "arc.flatness")?;
    if flatness_raw == 0 {
        return Err(JwcError::invalid(
            offset + 12,
            "arc.flatness",
            "flatness must be positive",
        ));
    }
    if flatness_raw > 10000 {
        return Err(JwcError::unsupported(
            offset + 12,
            "arc.flatness",
            "flatness above one is unverified",
        ));
    }
    let start_angle_degrees = angle(reader, offset + 14)?;
    let end_angle_degrees = angle(reader, offset + 18)?;
    let tilt_angle_degrees = angle(reader, offset + 22)?;
    let is_full_circle = start_angle_degrees == 0. && end_angle_degrees == 0.;
    if !is_full_circle && start_angle_degrees == end_angle_degrees {
        return Err(JwcError::unsupported(
            offset + 14,
            "arc.angles",
            "nonzero equal angles are unverified",
        ));
    }
    if (!is_full_circle && flatness_raw != 10000)
        || (flatness_raw == 10000 && tilt_angle_degrees != 0.)
    {
        return Err(JwcError::unsupported(
            offset + 12,
            "arc.geometry",
            "elliptical arcs and tilted circular arcs are unverified",
        ));
    }
    let attributes = stroke(reader, offset + 26)?;
    zero(reader, offset + 30, 2, "arc.flags")?;
    Ok(JwcEntityData::Arc {
        center,
        radius,
        flatness: f64::from(flatness_raw) / 10000.,
        start_angle_degrees,
        end_angle_degrees,
        tilt_angle_degrees,
        is_full_circle,
        attributes,
    })
}

fn text(
    reader: &Reader<'_>,
    header: &JwcHeader,
    offset: usize,
    entity_index: usize,
    diagnostics: &mut Vec<crate::Diagnostic>,
) -> Result<JwcEntityData, JwcError> {
    let start = xy(reader, offset, "text.start")?;
    let end = xy(reader, offset + 8, "text.end")?;
    if start == end {
        return Err(JwcError::unsupported(
            offset,
            "text.baseline",
            "zero-length text baseline is unverified",
        ));
    }
    let preset = reader.bytes(offset + 20, 1, "text.preset")?[0];
    if !(1..=10).contains(&preset) {
        return Err(JwcError::unsupported(
            offset + 20,
            "text.preset",
            "unverified text preset index",
        ));
    }
    color(
        header.text_presets[usize::from(preset)].pen_color,
        1709 + usize::from(preset) * 2,
    )?;
    zero(reader, offset + 22, 2, "text.attributes")?;
    // P2 has already validated reference flags, ranges, contiguity, bounded NULs,
    // and the entire file layout. Scan only this pool, never the name slots.
    let relative = (reader.u32(offset + 16, "text.string_reference")? & 0x3fff_ffff) as usize;
    let string_offset = header.layout.string_pool.byte_offset + relative;
    let remaining = header.layout.string_pool.byte_length - relative;
    let pool = reader.bytes(
        string_offset,
        remaining.min(super::JWC_MAX_TEXT_BYTES + 1),
        "text.content",
    )?;
    let length = pool.iter().position(|&b| b == 0).ok_or_else(|| {
        JwcError::invalid(
            string_offset,
            "text.content",
            "missing bounded NUL terminator",
        )
    })?;
    if length == 0 {
        return Err(JwcError::unsupported(
            string_offset,
            "text.content",
            "empty text records are unverified",
        ));
    }
    let content = reader.cp932(
        string_offset,
        length,
        &format!("entities[{entity_index}].content"),
        diagnostics,
    )?;
    Ok(JwcEntityData::Text {
        start,
        end,
        layer: JwcLayerAddress::from_packed(reader.bytes(offset + 21, 1, "text.layer")?[0]),
        text_preset: preset,
        content,
        raw_content: pool[..length].to_vec(),
        string_source: JwcSection {
            byte_offset: string_offset,
            byte_length: length + 1,
        },
    })
}

/// Decode the bounded fixed2421_basic_v1 record profile atomically. No partial
/// result, DXF conversion, paper/model-unit scaling or guessed font is returned.
pub fn parse_jwc_document(data: &[u8]) -> Result<JwcDocument, JwcError> {
    let header = parse_jwc_header(data)?;
    let reader = Reader::new(data);
    let counts = header.counts;
    // P2 checks the combined count, all byte spans and file size before this allocation.
    let count = counts.lines + counts.arcs + counts.texts + counts.points + counts.temporary_points;
    let mut entities = Vec::with_capacity(count as usize);
    let mut diagnostics = header.diagnostics.clone();
    for point in &header.temporary_points {
        let index = usize::from(point.array_index);
        entities.push(JwcEntity {
            source: source(
                &reader,
                &[
                    (800 + index * 4, 4),
                    (1204 + index * 4, 4),
                    (1608 + index, 1),
                ],
            )?,
            data: JwcEntityData::TemporaryPoint {
                position: Coord2D::new(f64::from(point.raw_x), f64::from(point.raw_y)),
                layer: JwcLayerAddress {
                    group: point.layer_group,
                    layer: point.layer,
                },
                array_index: point.array_index,
            },
        });
    }
    let mut curve_start = None;
    for index in 0..counts.lines as usize {
        let offset = header.layout.lines.byte_offset + index * 22;
        let start = xy(&reader, offset, "line.start")?;
        let end = xy(&reader, offset + 8, "line.end")?;
        let attributes = stroke(&reader, offset + 16)?;
        match (attributes.flags_raw, curve_start.is_some()) {
            (0, false) => {}
            (0x40, false) => curve_start = Some(offset + 20),
            (0x80, true) => {}
            (0xc0, _) => curve_start = None, // End of a group, or a singleton.
            _ => {
                return Err(JwcError::unsupported(
                    offset + 20,
                    "line.flags",
                    "unverified flags or curve-marker sequence",
                ))
            }
        }
        entities.push(JwcEntity {
            source: source(&reader, &[(offset, 22)])?,
            data: JwcEntityData::Line {
                start,
                end,
                attributes,
            },
        });
    }
    if let Some(offset) = curve_start {
        return Err(JwcError::unsupported(
            offset,
            "line.flags",
            "unterminated curve-marker sequence",
        ));
    }
    for index in 0..counts.arcs as usize {
        let offset = header.layout.arcs.byte_offset + index * 32;
        entities.push(JwcEntity {
            source: source(&reader, &[(offset, 32)])?,
            data: arc(&reader, offset)?,
        });
    }
    for index in 0..counts.texts as usize {
        let offset = header.layout.text_records.byte_offset + index * 24;
        let record = text(&reader, &header, offset, entities.len(), &mut diagnostics)?;
        entities.push(JwcEntity {
            source: source(&reader, &[(offset, 24)])?,
            data: record,
        });
    }
    for index in 0..counts.points as usize {
        let offset = header.layout.points.byte_offset + index * 12;
        let position = xy(&reader, offset, "point.position")?;
        let bytes = reader.bytes(offset + 8, 4, "point.attributes")?;
        color(u16::from(bytes[1]), offset + 9)?;
        zero(&reader, offset + 10, 2, "point.flags")?;
        entities.push(JwcEntity {
            source: source(&reader, &[(offset, 12)])?,
            data: JwcEntityData::Point {
                position,
                layer: JwcLayerAddress::from_packed(bytes[0]),
                pen_color: bytes[1],
                flags_raw: 0,
            },
        });
    }
    Ok(JwcDocument {
        profile_id: JwcDocumentProfile::Fixed2421BasicV1,
        header,
        entities,
        diagnostics,
    })
}

pub fn read_jwc_document_from_file(path: impl AsRef<Path>) -> Result<JwcDocument, JwcError> {
    parse_jwc_document(&read_jwc_bytes(path)?)
}
