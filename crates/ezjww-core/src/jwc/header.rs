use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::Serialize;

use super::layout::{validate_layout, JwcLayout};
use super::reader::Reader;
use super::{
    JwcError, FIXED_SIGNATURE, JWC_FAMILY_SIGNATURE, JWC_FIXED_HEADER_SIZE, JWC_MAX_ENTITIES,
    JWC_MAX_FILE_SIZE,
};
use crate::diagnostics::Diagnostic;

/// An ezjww header/framing profile, not a Jw_cad application or DOS version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum JwcHeaderProfile {
    #[serde(rename = "fixed2421_csv32_v1")]
    Fixed2421Csv32V1,
}

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
    /// Unknown in the investigated files; never fabricated from a profile ID.
    pub source_version: Option<String>,
    pub counts: JwcEntityCounts,
    pub paper: JwcPaper,
    pub coordinate_extent: f64,
    pub write_layer_group: u8,
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

    fn expect(&self, expected: &str, name: &str) -> Result<(), JwcError> {
        if self.text != expected {
            return Err(JwcError::unsupported(
                self.offset,
                name,
                format!("unverified setting; expected {expected:?}"),
            ));
        }
        Ok(())
    }
}

fn csv<'a>(reader: &Reader<'a>, start: usize, count: usize) -> Result<Vec<CsvField<'a>>, JwcError> {
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
    if fields.len() != count {
        return Err(JwcError::unsupported(
            start,
            "header.csv",
            format!("expected {count} fields, found {}", fields.len()),
        ));
    }
    Ok(fields)
}

fn layer_state(reader: &Reader<'_>, index: usize) -> Result<JwcLayerState, JwcError> {
    let edit = reader.bytes(1861 + index, 1, "layer.edit")?[0];
    let visible = reader.bytes(2133 + index, 1, "layer.visible")?[0];
    if visible > 1 {
        return Err(JwcError::unsupported(
            2133 + index,
            "layer.visible",
            "unverified visibility flags",
        ));
    }
    if !matches!((edit, visible), (0, 0) | (0, 1) | (1, 1) | (3, 1)) {
        return Err(JwcError::unsupported(
            1861 + index,
            "layer.edit",
            "unverified edit/protection combination",
        ));
    }
    Ok(JwcLayerState {
        editable: edit & 1 != 0,
        visible: visible != 0,
        protected: edit & 2 != 0,
    })
}

/// Read a complete file because names follow the entity arrays and string pool.
/// Validates the fixed2421_csv32_v1 header and framing, but does NOT validate
/// line/arc/text/point geometry, entity attributes, or document convertibility.
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
    let signature = reader.bytes(0, FIXED_SIGNATURE.len(), "signature")?;
    if let Some(offset) = signature
        .iter()
        .zip(FIXED_SIGNATURE)
        .position(|(a, b)| a != b)
    {
        return Err(JwcError::unsupported(
            offset,
            "signature",
            "unverified JWC signature variant",
        ));
    }
    if data.len() > JWC_MAX_FILE_SIZE {
        return Err(JwcError::limit(0, "file", "file exceeds 64 MiB"));
    }
    let fixed = reader.bytes(0, JWC_FIXED_HEADER_SIZE, "header")?;
    for offset in [199, 399, 599, 799] {
        if fixed[offset] != b'\n' {
            return Err(JwcError::unsupported(
                offset,
                "header",
                "missing fixed-position LF",
            ));
        }
    }
    let fields = csv(&reader, 200, 32)?;
    let settings = csv(&reader, 400, 28)?;
    let storage = csv(&reader, 600, 24)?;

    // Opaque settings are pinned to the native corpus. This is a deliberate
    // narrow profile, not acceptance of unknown configuration combinations.
    for (index, expected) in [
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
    ] {
        fields[index].expect(expected, &format!("header.csv0[{index}]"))?;
    }
    const SETTINGS: &str = "0.5,0.0,3.0,15.0,1,1,0,0,0,2,0.0,0.0,0.0,0.0,0.0,0.0,0,0,0,1000.000,100.000,200.000,300.000,400.000,500.000,0,0,1";
    for (index, expected) in SETTINGS.split(',').enumerate() {
        settings[index].expect(expected, &format!("header.csv1[{index}]"))?;
    }
    storage[0].expect("4000:0000", "header.string_pool_start")?;
    const STORAGE_SUFFIX: &str = "0,0,1,0,0,1,0,0,1,0,0,1,0,0,1,36,-23.45,1.5,0,-1,14,0";
    for (index, expected) in STORAGE_SUFFIX.split(',').enumerate() {
        storage[index + 2].expect(expected, &format!("header.csv2[{}]", index + 2))?;
    }
    let end = storage[1]
        .text
        .strip_prefix("4000:")
        .filter(|s| s.len() == 4 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| {
            JwcError::unsupported(
                storage[1].offset,
                "header.string_pool_end",
                "unverified pool address form",
            )
        })?;
    let pool_length = u16::from_str_radix(end, 16).map_err(|_| {
        JwcError::invalid(
            storage[1].offset,
            "header.string_pool_end",
            "invalid hexadecimal offset",
        )
    })? as usize;
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
    let (width, height) = paper.dimensions_mm();
    if (fields[28].number("header.view_y")? - height * 259. / width).abs() > 0.0005 {
        return Err(JwcError::unsupported(
            fields[28].offset,
            "header.view_y",
            "unverified view origin",
        ));
    }
    fields[18].integer("header.csv0[18]")?; // Varies during repeated native runs; no semantic label.
    let write = fields[10].integer("header.write_selection")?;
    if write > 255 || write & 15 != 0 {
        return Err(JwcError::unsupported(
            fields[10].offset,
            "header.write_selection",
            "unverified write selection bits",
        ));
    }
    let write_layer_group = (write >> 4) as u8;
    let layout = validate_layout(data, counts, pool_length)?;
    let mut diagnostics = Vec::new();
    let mut layer_groups: [JwcLayerGroup; 16] = std::array::from_fn(|_| JwcLayerGroup::default());
    for (g, group) in layer_groups.iter_mut().enumerate() {
        group.state = layer_state(&reader, g)?;
        let scale = reader.finite_f32(1797 + 4 * g, "header.group.scale")?;
        if scale <= 0. {
            return Err(JwcError::invalid(
                1797 + 4 * g,
                "header.group.scale",
                "scale must be positive",
            ));
        }
        group.scale = f64::from(scale);
        let write = fixed[2405 + g];
        if write >> 4 != g as u8 {
            return Err(JwcError::unsupported(
                2405 + g,
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
            layer.state = layer_state(&reader, 16 + 16 * g + l)?;
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
    if write_scale <= 0.
        || write_scale as f32 != layer_groups[usize::from(write_layer_group)].scale as f32
    {
        return Err(JwcError::invalid(
            fields[9].offset,
            "header.write_scale",
            "write scale does not match selected group",
        ));
    }
    let mut text_presets = [JwcTextPreset::default(); 11];
    for (index, preset) in text_presets.iter_mut().enumerate() {
        *preset = JwcTextPreset {
            pen_color: reader.u16(1709 + 2 * index, "header.text_preset.color")?,
            width_tenths: reader.u16(1731 + 2 * index, "header.text_preset.width")?,
            height_tenths: reader.u16(1753 + 2 * index, "header.text_preset.height")?,
            spacing_tenths: reader.u16(1775 + 2 * index, "header.text_preset.spacing")?,
        };
        if index == 0 && *preset != JwcTextPreset::default() {
            return Err(JwcError::unsupported(
                1709,
                "header.text_preset[0]",
                "nonzero spare preset",
            ));
        }
        if index != 0 && (preset.width_tenths == 0 || preset.height_tenths == 0) {
            return Err(JwcError::unsupported(
                1731 + 2 * index,
                "header.text_preset",
                "zero text dimensions are unverified",
            ));
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
    Ok(JwcHeader {
        profile_id: JwcHeaderProfile::Fixed2421Csv32V1,
        source_version: None,
        counts,
        paper,
        coordinate_extent: 518.,
        write_layer_group,
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
