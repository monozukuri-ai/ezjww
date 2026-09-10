use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::Serialize;

use super::layout::{validate_layout, JwcLayout};
use super::reader::Reader;
use super::unverified::UnverifiedCollector;
use super::{
    JwcError, FIXED_SIGNATURE, JWC_FAMILY_SIGNATURE, JWC_FIXED_HEADER_SIZE,
    JWC_FIXED_HEADER_SIZE_U16_SCALES, JWC_MAX_ENTITIES, JWC_MAX_FILE_SIZE,
    JWC_SIGNATURE_VARIANT_OFFSET,
};
use crate::diagnostics::{
    Diagnostic, JWC_ATTRIBUTE_UNVERIFIED, JWC_GROUP_SCALE_DEFAULTED,
    JWC_HEADER_SETTINGS_UNVERIFIED, JWC_WRITE_SCALE_MISMATCH,
};

/// An ezjww header/framing profile, not a Jw_cad application or DOS version.
///
/// Both profiles share the CSV blocks, temporary-point arrays and text preset
/// tables. They differ in how the 16 layer-group scales are stored, which moves
/// every later offset by 32 bytes. Signature byte 22 selects the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum JwcHeaderProfile {
    /// 2,421-byte fixed header; layer-group scales are `f32 × 16` (signature byte 22 = `f`).
    #[serde(rename = "fixed2421_csv32_v1")]
    Fixed2421Csv32V1,
    /// 2,389-byte fixed header; layer-group scales are `u16 × 16` (signature byte 22 = `.`).
    #[serde(rename = "fixed2389_u16scale_v1")]
    Fixed2389U16ScaleV1,
}

impl JwcHeaderProfile {
    pub fn fixed_header_size(self) -> usize {
        match self {
            Self::Fixed2421Csv32V1 => JWC_FIXED_HEADER_SIZE,
            Self::Fixed2389U16ScaleV1 => JWC_FIXED_HEADER_SIZE_U16_SCALES,
        }
    }

    fn scales_are_u16(self) -> bool {
        matches!(self, Self::Fixed2389U16ScaleV1)
    }

    fn scale_table_length(self) -> usize {
        if self.scales_are_u16() {
            32
        } else {
            64
        }
    }

    fn edit_flags_offset(self) -> usize {
        SCALES_OFFSET + self.scale_table_length()
    }

    fn visible_flags_offset(self) -> usize {
        self.edit_flags_offset() + 272
    }

    fn write_layers_offset(self) -> usize {
        self.visible_flags_offset() + 272
    }
}

const SCALES_OFFSET: usize = 1797;
const TEXT_PRESETS_OFFSET: usize = 1709;

/// csv0 fields whose values were constant in the reference corpus. Differences
/// are reported as `JWC_HEADER_SETTINGS_UNVERIFIED`; they no longer reject a file.
const CSV0_REFERENCE: &[(usize, &str)] = &[
    (5, "3"),
    (6, "2"),
    (7, "1"),
    (8, "1"),
    (12, "2"),
    (13, "1"),
    (14, "0"),
    (15, "0"),
    (16, "5"),
    (17, "10"),
    (19, "1"),
    (20, "2"),
    (21, "3"),
    (22, "4"),
    (23, "5"),
    (24, "6"),
    (25, "5"),
    (26, "5"),
    (27, "259"),
    (29, "0"),
    (30, "518"),
    (31, "0"),
];
const SETTINGS_REFERENCE: &str = "0.5,0.0,3.0,15.0,1,1,0,0,0,2,0.0,0.0,0.0,0.0,0.0,0.0,0,0,0,1000.000,100.000,200.000,300.000,400.000,500.000,0,0,1";
const STORAGE_SUFFIX_REFERENCE: &str = "0,0,1,0,0,1,0,0,1,0,0,1,0,0,1,36,-23.45,1.5,0,-1,14,0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct JwcEntityCounts {
    pub lines: u32,
    pub arcs: u32,
    pub texts: u32,
    pub points: u32,
    pub temporary_points: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum JwcPaper {
    A0,
    A1,
    A2,
    A3,
    A4,
}

impl JwcPaper {
    pub fn dimensions_mm(self) -> (f64, f64) {
        match self {
            Self::A0 => (1189., 841.),
            Self::A1 => (841., 594.),
            Self::A2 => (594., 420.),
            Self::A3 => (420., 297.),
            Self::A4 => (297., 210.),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct JwcName {
    pub text: String,
    /// Entire fixed-width slot, including padding and any undecodable bytes.
    pub raw_bytes: Vec<u8>,
    pub byte_offset: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct JwcLayerState {
    pub editable: bool,
    pub visible: bool,
    pub protected: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct JwcLayer {
    pub state: JwcLayerState,
    pub name: JwcName,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct JwcLayerGroup {
    pub state: JwcLayerState,
    pub scale: f64,
    pub write_layer: u8,
    pub name: JwcName,
    pub layers: [JwcLayer; 16],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct JwcTextPreset {
    pub pen_color: u16,
    pub width_tenths: u16,
    pub height_tenths: u16,
    pub spacing_tenths: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwcTemporaryPoint {
    pub array_index: u8,
    /// Coordinates in the source f32 system, not converted to paper or model units.
    pub raw_x: f32,
    pub raw_y: f32,
    pub layer_group: u8,
    pub layer: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwcHeader {
    pub profile_id: JwcHeaderProfile,
    /// Bytes before the first entity record (2,421 or 2,389 depending on the profile).
    pub fixed_header_size: usize,
    /// Unknown in the investigated files; never fabricated from a profile ID.
    pub source_version: Option<String>,
    pub counts: JwcEntityCounts,
    pub paper: JwcPaper,
    pub coordinate_extent: f64,
    pub write_layer_group: u8,
    /// Low nibble of the write selection; the layer currently written within the group.
    pub write_layer: u8,
    pub layer_groups: [JwcLayerGroup; 16],
    /// Slot zero is retained as the observed zero-valued spare.
    pub text_presets: [JwcTextPreset; 11],
    pub temporary_points: Vec<JwcTemporaryPoint>,
    pub layout: JwcLayout,
    /// Includes opaque memo/settings/inactive slots. No inferred memo capacity,
    /// application version, palette or font is supplied as decoded metadata.
    pub raw_fixed_header: Vec<u8>,
    pub diagnostics: Vec<Diagnostic>,
}

struct CsvField<'a> {
    text: &'a str,
    offset: usize,
}

impl CsvField<'_> {
    fn integer(&self, name: &str) -> Result<u32, JwcError> {
        if self.text.is_empty() || !self.text.bytes().all(|b| b.is_ascii_digit()) {
            return Err(JwcError::invalid(
                self.offset,
                name,
                "expected an unsigned decimal integer",
            ));
        }
        self.text
            .parse()
            .map_err(|_| JwcError::limit(self.offset, name, "integer exceeds u32"))
    }

    fn number(&self, name: &str) -> Result<f64, JwcError> {
        let value: f64 = self
            .text
            .parse()
            .map_err(|_| JwcError::invalid(self.offset, name, "expected a number"))?;
        if !value.is_finite() {
            return Err(JwcError::invalid(
                self.offset,
                name,
                "expected a finite number",
            ));
        }
        Ok(value)
    }
}

/// Read one bounded CSV block. Framing (NUL terminator, NUL/space padding,
/// printable ASCII) is structural and still fails; the field count only has a
/// lower bound because real files carry 31 or 32 csv0 fields.
fn csv<'a>(
    reader: &Reader<'a>,
    start: usize,
    min_fields: usize,
) -> Result<Vec<CsvField<'a>>, JwcError> {
    let data = reader.bytes(start, 199, "header.csv")?;
    let end = data.iter().position(|&b| b == 0).ok_or_else(|| {
        JwcError::unsupported(start, "header.csv", "missing bounded CSV terminator")
    })?;
    if let Some(index) = data[end..].iter().position(|&b| b != 0 && b != b' ') {
        return Err(JwcError::unsupported(
            start + end + index,
            "header.csv",
            "unverified CSV padding",
        ));
    }
    if let Some(index) = data[..end]
        .iter()
        .position(|&b| !(b' '..=b'~').contains(&b))
    {
        return Err(JwcError::invalid(
            start + index,
            "header.csv",
            "non-printable ASCII in CSV",
        ));
    }
    let text = std::str::from_utf8(&data[..end])
        .map_err(|_| JwcError::invalid(start, "header.csv", "invalid ASCII"))?;
    let mut offset = start;
    let fields: Vec<_> = text
        .split(',')
        .map(|raw| {
            let field = CsvField {
                text: raw.trim(),
                offset: offset + raw.len() - raw.trim_start().len(),
            };
            offset += raw.len() + 1;
            field
        })
        .collect();
    if fields.len() < min_fields {
        return Err(JwcError::unsupported(
            start,
            "header.csv",
            format!(
                "expected at least {min_fields} fields, found {}",
                fields.len()
            ),
        ));
    }
    Ok(fields)
}

/// `SSSS:OOOO` — a DOS far pointer (segment:offset, hexadecimal). The reference
/// corpus always used segment `4000`; real files use other segments.
fn far_pointer(field: &CsvField<'_>, name: &str) -> Result<u32, JwcError> {
    let parse = |part: &str| -> Option<u32> {
        (part.len() == 4 && part.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| u32::from_str_radix(part, 16).ok())
            .flatten()
    };
    field
        .text
        .split_once(':')
        .and_then(|(segment, offset)| Some((parse(segment)? << 16) | parse(offset)?))
        .ok_or_else(|| JwcError::unsupported(field.offset, name, "unverified pool address form"))
}

fn compare_reference(
    fields: &[CsvField<'_>],
    reference: impl IntoIterator<Item = (usize, &'static str)>,
    name: &str,
    unverified: &mut UnverifiedCollector,
) {
    for (index, expected) in reference {
        if let Some(field) = fields.get(index) {
            if field.text != expected {
                unverified.note(
                    JWC_HEADER_SETTINGS_UNVERIFIED,
                    name,
                    field.offset,
                    format!("[{index}]={}", field.text),
                );
            }
        }
    }
}

fn layer_state(
    reader: &Reader<'_>,
    profile: JwcHeaderProfile,
    index: usize,
    unverified: &mut UnverifiedCollector,
) -> Result<JwcLayerState, JwcError> {
    let edit_offset = profile.edit_flags_offset() + index;
    let visible_offset = profile.visible_flags_offset() + index;
    let edit = reader.bytes(edit_offset, 1, "layer.edit")?[0];
    let visible = reader.bytes(visible_offset, 1, "layer.visible")?[0];
    if edit & !3 != 0 {
        unverified.note(
            JWC_ATTRIBUTE_UNVERIFIED,
            "layer.edit",
            edit_offset,
            format!("{edit:#04x}"),
        );
    }
    if visible & !1 != 0 {
        unverified.note(
            JWC_ATTRIBUTE_UNVERIFIED,
            "layer.visible",
            visible_offset,
            format!("{visible:#04x}"),
        );
    }
    if !matches!((edit & 3, visible & 1), (0, 0) | (0, 1) | (1, 1) | (3, 1)) {
        unverified.note(
            JWC_ATTRIBUTE_UNVERIFIED,
            "layer.state",
            edit_offset,
            format!("edit={edit} visible={visible}"),
        );
    }
    Ok(JwcLayerState {
        editable: edit & 1 != 0,
        visible: visible & 1 != 0,
        protected: edit & 2 != 0,
    })
}

fn signature_profile(reader: &Reader<'_>) -> Result<JwcHeaderProfile, JwcError> {
    let signature = reader.bytes(0, FIXED_SIGNATURE.len(), "signature")?;
    let profile = match signature[JWC_SIGNATURE_VARIANT_OFFSET] {
        b'f' => JwcHeaderProfile::Fixed2421Csv32V1,
        b'.' => JwcHeaderProfile::Fixed2389U16ScaleV1,
        _ => {
            return Err(JwcError::unsupported(
                JWC_SIGNATURE_VARIANT_OFFSET,
                "signature",
                "unverified JWC signature variant",
            ))
        }
    };
    if let Some(offset) = signature
        .iter()
        .zip(FIXED_SIGNATURE)
        .enumerate()
        .position(|(index, (a, b))| index != JWC_SIGNATURE_VARIANT_OFFSET && a != b)
    {
        return Err(JwcError::unsupported(
            offset,
            "signature",
            "unverified JWC signature variant",
        ));
    }
    Ok(profile)
}

/// Read a complete file because names follow the entity arrays and string pool.
/// Validates the header framing (fixed LFs, CSV blocks, pool addresses, record
/// spans, temporary-point slots, write-layer packing) but does NOT validate
/// line/arc/text/point geometry, entity attributes, or document convertibility.
/// Settings that merely differ from the reference corpus are retained and
/// reported as diagnostics instead of rejecting the file.
pub fn parse_jwc_header(data: &[u8]) -> Result<JwcHeader, JwcError> {
    let matched = data.len().min(JWC_FAMILY_SIGNATURE.len());
    if let Some(offset) = data[..matched]
        .iter()
        .zip(JWC_FAMILY_SIGNATURE)
        .position(|(a, b)| a != b)
    {
        return Err(JwcError::InvalidSignature { offset });
    }
    let reader = Reader::new(data);
    let profile = signature_profile(&reader)?;
    if data.len() > JWC_MAX_FILE_SIZE {
        return Err(JwcError::limit(0, "file", "file exceeds 64 MiB"));
    }
    let fixed_header_size = profile.fixed_header_size();
    let fixed = reader.bytes(0, fixed_header_size, "header")?;
    for offset in [199, 399, 599, 799] {
        if fixed[offset] != b'\n' {
            return Err(JwcError::unsupported(
                offset,
                "header",
                "missing fixed-position LF",
            ));
        }
    }
    let mut unverified = UnverifiedCollector::new();
    let fields = csv(&reader, 200, 12)?;
    let settings = csv(&reader, 400, 0)?;
    let storage = csv(&reader, 600, 2)?;
    compare_reference(
        &fields,
        CSV0_REFERENCE.iter().copied(),
        "header.csv0",
        &mut unverified,
    );
    compare_reference(
        &settings,
        SETTINGS_REFERENCE.split(',').enumerate(),
        "header.csv1",
        &mut unverified,
    );
    compare_reference(
        &storage,
        STORAGE_SUFFIX_REFERENCE
            .split(',')
            .enumerate()
            .map(|(index, value)| (index + 2, value)),
        "header.csv2",
        &mut unverified,
    );
    let pool_start = far_pointer(&storage[0], "header.string_pool_start")?;
    let pool_end = far_pointer(&storage[1], "header.string_pool_end")?;
    let pool_length = pool_end.checked_sub(pool_start).ok_or_else(|| {
        JwcError::unsupported(
            storage[1].offset,
            "header.string_pool_end",
            "pool end precedes pool start",
        )
    })? as usize;
    if pool_length > 0xffff {
        return Err(JwcError::limit(
            storage[1].offset,
            "header.string_pool_end",
            "text pool exceeds 65535 bytes",
        ));
    }
    let mut counts = [0; 5];
    let mut total = 0_u64;
    for (index, count) in counts.iter_mut().enumerate() {
        *count = fields[index].integer("header.entity_count")?;
        total += u64::from(*count);
        if total > JWC_MAX_ENTITIES as u64 {
            return Err(JwcError::limit(
                fields[index].offset,
                "header.entity_count",
                "combined count exceeds 100000",
            ));
        }
    }
    if counts[4] > 100 {
        return Err(JwcError::limit(
            fields[4].offset,
            "header.temporary_points",
            "temporary point count exceeds 100",
        ));
    }
    let counts = JwcEntityCounts {
        lines: counts[0],
        arcs: counts[1],
        texts: counts[2],
        points: counts[3],
        temporary_points: counts[4],
    };
    let paper = match fields[11].integer("header.paper")? {
        0 => JwcPaper::A0,
        1 => JwcPaper::A1,
        2 => JwcPaper::A2,
        3 => JwcPaper::A3,
        4 => JwcPaper::A4,
        _ => {
            return Err(JwcError::unsupported(
                fields[11].offset,
                "header.paper",
                "unverified paper code",
            ))
        }
    };
    let coordinate_extent = match fields.get(30) {
        Some(field) => field.number("header.coordinate_extent")?,
        None => 518.,
    };
    let write = fields[10].integer("header.write_selection")?;
    if write > 255 {
        return Err(JwcError::unsupported(
            fields[10].offset,
            "header.write_selection",
            "unverified write selection bits",
        ));
    }
    let write_layer_group = (write >> 4) as u8;
    let write_layer = (write & 15) as u8;
    let layout = validate_layout(data, counts, pool_start, pool_length, fixed_header_size)?;
    let mut diagnostics = Vec::new();
    let mut layer_groups: [JwcLayerGroup; 16] = std::array::from_fn(|_| JwcLayerGroup::default());
    let write_layers_offset = profile.write_layers_offset();
    for (g, group) in layer_groups.iter_mut().enumerate() {
        group.state = layer_state(&reader, profile, g, &mut unverified)?;
        group.scale = if profile.scales_are_u16() {
            let offset = SCALES_OFFSET + 2 * g;
            let scale = reader.u16(offset, "header.group.scale")?;
            if scale == 0 {
                unverified.note(
                    JWC_GROUP_SCALE_DEFAULTED,
                    "header.group.scale",
                    offset,
                    g.to_string(),
                );
                1.
            } else {
                f64::from(scale)
            }
        } else {
            let offset = SCALES_OFFSET + 4 * g;
            let scale = reader.finite_f32(offset, "header.group.scale")?;
            if scale <= 0. {
                return Err(JwcError::invalid(
                    offset,
                    "header.group.scale",
                    "scale must be positive",
                ));
            }
            f64::from(scale)
        };
        let write = fixed[write_layers_offset + g];
        if write >> 4 != g as u8 {
            return Err(JwcError::unsupported(
                write_layers_offset + g,
                "header.group.write_layer",
                "packed group does not match its array index",
            ));
        }
        group.write_layer = write & 15;
        group.name = reader.fixed_cp932(
            layout.names.byte_offset + 2048 + 16 * g,
            16,
            14,
            &format!("header.layer_groups[{g}].name"),
            &mut diagnostics,
        )?;
        for (l, layer) in group.layers.iter_mut().enumerate() {
            layer.state = layer_state(&reader, profile, 16 + 16 * g + l, &mut unverified)?;
            layer.name = reader.fixed_cp932(
                layout.names.byte_offset + 8 * (16 * g + l),
                8,
                7,
                &format!("header.layer_groups[{g}].layers[{l}].name"),
                &mut diagnostics,
            )?;
        }
    }
    let write_scale = fields[9].number("header.write_scale")?;
    let selected_scale = layer_groups[usize::from(write_layer_group)].scale;
    if write_scale <= 0. || write_scale as f32 != selected_scale as f32 {
        unverified.note(
            JWC_WRITE_SCALE_MISMATCH,
            "header.write_scale",
            fields[9].offset,
            format!("write_scale={write_scale} group[{write_layer_group}]={selected_scale}"),
        );
    }
    let mut text_presets = [JwcTextPreset::default(); 11];
    for (index, preset) in text_presets.iter_mut().enumerate() {
        *preset = JwcTextPreset {
            pen_color: reader.u16(TEXT_PRESETS_OFFSET + 2 * index, "header.text_preset.color")?,
            width_tenths: reader.u16(
                TEXT_PRESETS_OFFSET + 22 + 2 * index,
                "header.text_preset.width",
            )?,
            height_tenths: reader.u16(
                TEXT_PRESETS_OFFSET + 44 + 2 * index,
                "header.text_preset.height",
            )?,
            spacing_tenths: reader.u16(
                TEXT_PRESETS_OFFSET + 66 + 2 * index,
                "header.text_preset.spacing",
            )?,
        };
        if index == 0 && *preset != JwcTextPreset::default() {
            unverified.note(
                JWC_ATTRIBUTE_UNVERIFIED,
                "header.text_preset[0]",
                TEXT_PRESETS_OFFSET,
                format!("{preset:?}"),
            );
        }
        if index != 0 && (preset.width_tenths == 0 || preset.height_tenths == 0) {
            unverified.note(
                JWC_ATTRIBUTE_UNVERIFIED,
                "header.text_preset",
                TEXT_PRESETS_OFFSET + 22 + 2 * index,
                format!("preset {index} has zero width or height"),
            );
        }
    }
    let mut temporary_points = Vec::with_capacity(counts.temporary_points as usize);
    for index in 0..101 {
        if index == 0 || index > counts.temporary_points as usize {
            for (offset, length) in [
                (800 + index * 4, 4),
                (1204 + index * 4, 4),
                (1608 + index, 1),
            ] {
                if fixed[offset..offset + length].iter().any(|&b| b != 0) {
                    return Err(JwcError::unsupported(
                        offset,
                        "header.temporary_points",
                        "nonzero inactive temporary point slot",
                    ));
                }
            }
            continue;
        }
        let packed = fixed[1608 + index];
        temporary_points.push(JwcTemporaryPoint {
            array_index: index as u8,
            raw_x: reader.finite_f32(800 + index * 4, "header.temporary_points.x")?,
            raw_y: reader.finite_f32(1204 + index * 4, "header.temporary_points.y")?,
            layer_group: packed >> 4,
            layer: packed & 15,
        });
    }
    diagnostics.extend(unverified.into_diagnostics());
    Ok(JwcHeader {
        profile_id: profile,
        fixed_header_size,
        source_version: None,
        counts,
        paper,
        coordinate_extent,
        write_layer_group,
        write_layer,
        layer_groups,
        text_presets,
        temporary_points,
        layout,
        raw_fixed_header: fixed.to_vec(),
        diagnostics,
    })
}

/// Bounded file read; metadata length is checked before allocating file contents.
pub fn read_jwc_header_from_file(path: impl AsRef<Path>) -> Result<JwcHeader, JwcError> {
    parse_jwc_header(&read_jwc_bytes(path)?)
}

pub(super) fn read_jwc_bytes(path: impl AsRef<Path>) -> Result<Vec<u8>, JwcError> {
    let file = File::open(path)?;
    if file.metadata()?.len() > JWC_MAX_FILE_SIZE as u64 {
        return Err(JwcError::limit(0, "file", "file exceeds 64 MiB"));
    }
    let mut data = Vec::new();
    file.take(JWC_MAX_FILE_SIZE as u64 + 1)
        .read_to_end(&mut data)?;
    if data.len() > JWC_MAX_FILE_SIZE {
        return Err(JwcError::limit(
            0,
            "file",
            "file grew beyond 64 MiB while reading",
        ));
    }
    Ok(data)
}
