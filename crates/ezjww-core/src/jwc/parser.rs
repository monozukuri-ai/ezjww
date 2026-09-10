use std::path::Path;

use super::header::read_jwc_bytes;
use super::model::{
    JwcDocument, JwcDocumentProfile, JwcEntity, JwcEntityData, JwcEntitySource, JwcLayerAddress,
    JwcStrokeAttributes,
};
use super::reader::Reader;
use super::unverified::UnverifiedCollector;
use super::{parse_jwc_header, JwcError, JwcHeader, JwcHeaderProfile, JwcSection};
use crate::diagnostics::{JWC_ATTRIBUTE_UNVERIFIED, JWC_CURVE_MARKERS_UNVERIFIED};
use crate::Coord2D;

/// Bits of the line flag word that encode curve-group boundaries
/// (`0x40` start, `0x80` member, `0xC0` end or singleton).
pub const JWC_CURVE_MARKER_MASK: u16 = 0xc0;

fn xy(reader: &Reader<'_>, offset: usize, field: &str) -> Result<Coord2D, JwcError> {
    Ok(Coord2D::new(
        f64::from(reader.finite_f32(offset, field)?),
        f64::from(reader.finite_f32(offset + 4, field)?),
    ))
}

/// Spare bytes were always zero in the reference corpus; real files carry
/// attribute bits there. They are retained in the record's raw bytes and reported.
fn spare(
    reader: &Reader<'_>,
    offset: usize,
    length: usize,
    field: &str,
    unverified: &mut UnverifiedCollector,
) -> Result<(), JwcError> {
    let bytes = reader.bytes(offset, length, field)?;
    if bytes.iter().any(|&b| b != 0) {
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        unverified.note(JWC_ATTRIBUTE_UNVERIFIED, field, offset, format!("0x{hex}"));
    }
    Ok(())
}

fn color(value: u16, offset: usize) -> Result<(), JwcError> {
    if !(1..=9).contains(&value) {
        return Err(JwcError::unsupported(
            offset,
            "entity.pen_color",
            "unverified color number",
        ));
    }
    Ok(())
}

/// The low nibble is the line style number (1–9, JWW numbering); the high
/// nibble carries attribute bits observed only in real files (e.g. `0x10`).
fn style(value: u8, offset: usize, unverified: &mut UnverifiedCollector) -> Result<(), JwcError> {
    if !(1..=9).contains(&(value & 0x0f)) {
        return Err(JwcError::unsupported(
            offset,
            "entity.pen_style",
            "unverified line style",
        ));
    }
    if value & 0xf0 != 0 {
        unverified.note(
            JWC_ATTRIBUTE_UNVERIFIED,
            "entity.pen_style",
            offset,
            value.to_string(),
        );
    }
    Ok(())
}

fn stroke(
    reader: &Reader<'_>,
    offset: usize,
    unverified: &mut UnverifiedCollector,
) -> Result<JwcStrokeAttributes, JwcError> {
    let bytes = reader.bytes(offset, 6, "entity.attributes")?;
    style(bytes[0], offset, unverified)?;
    color(u16::from(bytes[1]), offset + 1)?;
    spare(reader, offset + 3, 1, "entity.spare", unverified)?;
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

fn arc(
    reader: &Reader<'_>,
    offset: usize,
    unverified: &mut UnverifiedCollector,
) -> Result<JwcEntityData, JwcError> {
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
    // Equal angles denote a closed curve. The reference corpus only stored 0/0;
    // real files may store any equal pair.
    let is_full_circle = start_angle_degrees == end_angle_degrees;
    if is_full_circle && start_angle_degrees != 0. {
        unverified.note(
            JWC_ATTRIBUTE_UNVERIFIED,
            "arc.angles",
            offset + 14,
            format!("start=end={start_angle_degrees}"),
        );
    }
    let attributes = stroke(reader, offset + 26, unverified)?;
    spare(reader, offset + 30, 2, "arc.flags", unverified)?;
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
    unverified: &mut UnverifiedCollector,
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
    spare(reader, offset + 22, 2, "text.flags", unverified)?;
    // P2 has already validated reference ranges, contiguity, bounded NULs,
    // and the entire file layout. Scan only this pool, never the name slots.
    let reference = reader.u32(offset + 16, "text.string_reference")?;
    let relative = reference.wrapping_sub(header.layout.string_pool_start()) as usize;
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

/// Decode the bounded record profile atomically. No partial result, DXF
/// conversion, paper/model-unit scaling or guessed font is returned. Values
/// outside the reference corpus (style bits, flags, spare bytes) are retained
/// and reported as diagnostics; structural inconsistencies still fail.
pub fn parse_jwc_document(data: &[u8]) -> Result<JwcDocument, JwcError> {
    let header = parse_jwc_header(data)?;
    let reader = Reader::new(data);
    let counts = header.counts;
    // P2 checks the combined count, all byte spans and file size before this allocation.
    let count = counts.lines + counts.arcs + counts.texts + counts.points + counts.temporary_points;
    let mut entities = Vec::with_capacity(count as usize);
    let mut diagnostics = header.diagnostics.clone();
    let mut unverified = UnverifiedCollector::new();
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
    let mut curve_open = false;
    let mut last_flags_offset = None;
    for index in 0..counts.lines as usize {
        let offset = header.layout.lines.byte_offset + index * 22;
        let start = xy(&reader, offset, "line.start")?;
        let end = xy(&reader, offset + 8, "line.end")?;
        let attributes = stroke(&reader, offset + 16, &mut unverified)?;
        let marker = attributes.flags_raw & JWC_CURVE_MARKER_MASK;
        let other = attributes.flags_raw & !JWC_CURVE_MARKER_MASK;
        if other != 0 {
            unverified.note(
                JWC_ATTRIBUTE_UNVERIFIED,
                "line.flags",
                offset + 20,
                format!("{other:#06x}"),
            );
        }
        match (marker, curve_open) {
            (0, false) => {}
            (0x40, false) => curve_open = true,
            (0x80, true) => {}
            (0xc0, _) => curve_open = false, // End of a group, or a singleton.
            _ => {
                unverified.note(
                    JWC_CURVE_MARKERS_UNVERIFIED,
                    "line.flags",
                    offset + 20,
                    format!("{marker:#04x}"),
                );
                curve_open = marker == 0x40;
            }
        }
        last_flags_offset = Some(offset + 20);
        entities.push(JwcEntity {
            source: source(&reader, &[(offset, 22)])?,
            data: JwcEntityData::Line {
                start,
                end,
                attributes,
            },
        });
    }
    if curve_open {
        unverified.note(
            JWC_CURVE_MARKERS_UNVERIFIED,
            "line.flags",
            last_flags_offset.unwrap_or(header.layout.lines.byte_offset),
            "unterminated",
        );
    }
    for index in 0..counts.arcs as usize {
        let offset = header.layout.arcs.byte_offset + index * 32;
        entities.push(JwcEntity {
            source: source(&reader, &[(offset, 32)])?,
            data: arc(&reader, offset, &mut unverified)?,
        });
    }
    for index in 0..counts.texts as usize {
        let offset = header.layout.text_records.byte_offset + index * 24;
        let record = text(
            &reader,
            &header,
            offset,
            entities.len(),
            &mut diagnostics,
            &mut unverified,
        )?;
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
        spare(&reader, offset + 10, 2, "point.flags", &mut unverified)?;
        entities.push(JwcEntity {
            source: source(&reader, &[(offset, 12)])?,
            data: JwcEntityData::Point {
                position,
                layer: JwcLayerAddress::from_packed(bytes[0]),
                pen_color: bytes[1],
                flags_raw: reader.u16(offset + 10, "point.flags")?,
            },
        });
    }
    diagnostics.extend(unverified.into_diagnostics());
    Ok(JwcDocument {
        profile_id: match header.profile_id {
            JwcHeaderProfile::Fixed2421Csv32V1 => JwcDocumentProfile::Fixed2421BasicV1,
            JwcHeaderProfile::Fixed2389U16ScaleV1 => JwcDocumentProfile::Fixed2389BasicV1,
        },
        header,
        entities,
        diagnostics,
    })
}

pub fn read_jwc_document_from_file(path: impl AsRef<Path>) -> Result<JwcDocument, JwcError> {
    parse_jwc_document(&read_jwc_bytes(path)?)
}
