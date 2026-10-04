use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::diagnostics::DecodeDiagnostic;
use crate::error::JwwError;
use crate::reader::Reader;

pub const JWW_SIGNATURE: &[u8; 8] = b"JwwData.";

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LayerHeader {
    pub state: u32,
    pub protect: u32,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LayerGroupHeader {
    pub state: u32,
    pub write_layer: u32,
    pub scale: f64,
    pub protect: u32,
    pub layers: [LayerHeader; 16],
    pub name: String,
}

/// Screen colors recorded in the JWW header.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JwwPalette {
    /// Screen color for pen color numbers 0..=9, normalized to `0x00BBGGRR`.
    ///
    /// Index 0 is the screen background rather than a pen:
    /// it is white on files saved with a white background and black on the rest.
    /// Index 9 is the construction line color. Only 1..=8 are real pen colors.
    pub pen_colors: [u32; 10],
    /// Screen colors for extended pen color numbers 100..=356, normalized to
    /// `0x00BBGGRR`. Number 100 is a spare, 101..=116 are the SXF standard
    /// colors, and 117..=356 are user-defined colors.
    ///
    /// `None` before JWW version 420, which does not store the extended palette.
    pub extended_colors: Option<Box<[u32]>>,
}

impl JwwPalette {
    /// Screen color for a pen color number, or `None` when the palette does not define it.
    ///
    /// Pen color 0 is deliberately excluded: `pen_colors[0]` holds the background,
    /// so treating it as a pen would paint entities in the invisible color.
    pub fn screen_color(&self, pen_color: u16) -> Option<u32> {
        match pen_color {
            1..=9 => Some(self.pen_colors[pen_color as usize]),
            100..=356 => self
                .extended_colors
                .as_ref()
                .and_then(|colors| colors.get((pen_color - 100) as usize).copied()),
            _ => None,
        }
    }
}

/// One pattern bit prints as `printer_pitch * PRINTER_PITCH_UNIT_MM` millimetres.
///
/// Jw_cad stores, for its SXF-compatible line types, both the bit pattern with
/// its printer pitch and the segment lengths in millimetres. The two agree at
/// this ratio (within the rounding of the integer pitch) in every header inspected.
pub const PRINTER_PITCH_UNIT_MM: f64 = 1.0 / 32.0;

/// Line type number (entity pen style) of the first standard dashed line type.
pub const STANDARD_LINE_TYPE_BASE: u32 = 2;
/// Line type number of the first random ("hand-drawn") line type.
pub const RANDOM_LINE_TYPE_BASE: u32 = 11;
/// Line type number of the first double-length line type.
pub const DOUBLE_LENGTH_LINE_TYPE_BASE: u32 = 16;
/// Line type number offset of the SXF-compatible line types (`30 + SXF code`).
pub const SXF_LINE_TYPE_BASE: u32 = 30;
/// SXF-compatible line types from this index (line type number 47) on are user-defined.
pub const SXF_USER_DEFINED_FIRST_INDEX: usize = 17;

/// Dash pattern and pitches of one Jw_cad line type.
///
/// The lowest `unit_dots` bits of `pattern` are one repetition; a set bit draws
/// and a clear bit is a gap. `pitch` scales the pattern on screen and
/// `printer_pitch` on paper.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineTypePattern {
    /// Line type number as stored in the entity pen style.
    pub number: u32,
    pub pattern: u32,
    pub unit_dots: u32,
    pub pitch: u32,
    pub printer_pitch: u32,
}

impl LineTypePattern {
    /// Run lengths in pattern bits, alternating dash and gap and starting with the longest dash.
    ///
    /// The pattern repeats, so the runs are taken around the unit:
    /// a dash that wraps from the end of the unit to its start is one run.
    /// Empty for a solid pattern, an all-gap pattern or an unusable `unit_dots`.
    pub fn runs(&self) -> Vec<u32> {
        let unit = self.unit_dots as usize;
        if !(1..=32).contains(&unit) {
            return Vec::new();
        }
        let bit = |index: usize| (self.pattern >> (index % unit)) & 1 == 1;
        // Start where a dash begins, so that every run is whole.
        let Some(start) = (0..unit).find(|&i| bit(i) && !bit(i + unit - 1)) else {
            return Vec::new();
        };
        let mut runs = Vec::new();
        let mut current = true;
        let mut length = 0u32;
        for offset in 0..unit {
            let value = bit(start + offset);
            if value == current {
                length += 1;
            } else {
                runs.push(length);
                current = value;
                length = 1;
            }
        }
        runs.push(length);
        // A chain line reads "long dash, gap, short dash, gap" wherever the unit happens to start.
        let longest = (0..runs.len())
            .step_by(2)
            .rev()
            .max_by_key(|&index| runs[index])
            .unwrap_or(0);
        runs.rotate_left(longest);
        runs
    }

    /// Printed dash and gap lengths in millimetres, alternating and starting with a dash.
    pub fn printed_segments_mm(&self) -> Vec<f64> {
        let unit = f64::from(self.printer_pitch) * PRINTER_PITCH_UNIT_MM;
        self.runs()
            .into_iter()
            .map(|run| f64::from(run) * unit)
            .collect()
    }
}

/// Settings of one random ("hand-drawn") line type, numbers 11..=15.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RandomLineType {
    /// Line type number as stored in the entity pen style.
    pub number: u32,
    pub pattern: u32,
    /// Screen amplitude.
    pub width: u32,
    pub pitch: u32,
    /// Printer output amplitude.
    pub printer_width: u32,
    pub printer_pitch: u32,
}

/// One SXF-compatible line type, numbers 30..=62 (`30 + index`).
///
/// Indices 1..=16 are the SXF predefined line types and 17..=32 are user-defined.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct SxfLineType {
    #[serde(flatten)]
    pub pattern: LineTypePattern,
    pub name: String,
    /// Dash and gap lengths in millimetres on paper, alternating and starting
    /// with a dash. Empty for an undefined slot or a solid line.
    pub segments_mm: Vec<f64>,
}

/// Line type settings recorded in the JWW header.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JwwLineTypes {
    /// Line types 2..=9: dashed 1-3, chain 1-2, double-dot chain 1-2 and the
    /// construction line type.
    pub standard: Vec<LineTypePattern>,
    /// Random line types 11..=15.
    pub random: Vec<RandomLineType>,
    /// Double-length line types 16..=19.
    pub double_length: Vec<LineTypePattern>,
    /// SXF-compatible line types 30..=62.
    ///
    /// `None` before JWW version 420, which does not store them, and when the
    /// section could not be read.
    pub sxf: Option<Vec<SxfLineType>>,
}

impl JwwLineTypes {
    /// Pattern settings for a line type number, or `None` when the header does not define it.
    ///
    /// Random line types have no dash pattern and are not reported here.
    pub fn pattern(&self, number: u32) -> Option<&LineTypePattern> {
        self.standard
            .iter()
            .chain(&self.double_length)
            .chain(
                self.sxf
                    .iter()
                    .flatten()
                    .map(|line_type| &line_type.pattern),
            )
            .find(|pattern| pattern.number == number)
    }

    /// Printed dash and gap lengths in millimetres for a line type number.
    ///
    /// SXF-compatible line types report the lengths stored with their definition;
    /// the others derive them from the bit pattern and the printer pitch.
    /// Empty for a solid, undefined or random line type.
    pub fn printed_segments_mm(&self, number: u32) -> Vec<f64> {
        if let Some(line_type) = self
            .sxf
            .iter()
            .flatten()
            .find(|line_type| line_type.pattern.number == number)
        {
            return line_type.segments_mm.clone();
        }
        self.pattern(number)
            .map(LineTypePattern::printed_segments_mm)
            .unwrap_or_default()
    }
}

/// Character size presets 1..=10, in paper millimetres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextPreset {
    pub size_x: f64,
    pub size_y: f64,
    pub spacing: f64,
    pub pen_color: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwwHeader {
    pub version: u32,
    pub memo: String,
    pub paper_size: u32,
    pub write_layer_group: u32,
    pub layer_groups: [LayerGroupHeader; 16],
    /// Only available when the layer name section was parsed successfully.
    pub palette: Option<JwwPalette>,
    /// Only available when the layer name section was parsed successfully.
    pub line_types: Option<JwwLineTypes>,
    pub text_presets: Option<Vec<TextPreset>>,
}

pub fn is_jww_signature(data: &[u8]) -> bool {
    data.len() >= JWW_SIGNATURE.len() && &data[..JWW_SIGNATURE.len()] == JWW_SIGNATURE
}

pub fn parse_header(data: &[u8]) -> Result<JwwHeader, JwwError> {
    parse_header_with_diagnostics(data).map(|(header, _)| header)
}

pub(crate) fn parse_header_with_diagnostics(
    data: &[u8],
) -> Result<(JwwHeader, Vec<DecodeDiagnostic>), JwwError> {
    if !is_jww_signature(data) {
        let head = data
            .iter()
            .take(JWW_SIGNATURE.len())
            .map(|&byte| byte as char)
            .collect::<String>();
        if head.len() == JWW_SIGNATURE.len() && head.chars().all(|c| (' '..='~').contains(&c)) {
            return Err(JwwError::InvalidSignatureFound(head));
        }
        return Err(JwwError::InvalidSignature);
    }

    let mut reader = Reader::new(data);
    reader.skip(JWW_SIGNATURE.len())?;

    let version = reader.read_u32()?;
    let memo = reader.read_cstring_with_context("header.memo")?;
    let paper_size = reader.read_u32()?;
    let write_layer_group = reader.read_u32()?;

    let mut layer_groups = std::array::from_fn(|_| LayerGroupHeader {
        layers: std::array::from_fn(|_| LayerHeader::default()),
        ..LayerGroupHeader::default()
    });
    for group in &mut layer_groups {
        group.state = reader.read_u32()?;
        group.write_layer = reader.read_u32()?;
        group.scale = reader.read_f64()?;
        group.protect = reader.read_u32()?;

        for layer in &mut group.layers {
            layer.state = reader.read_u32()?;
            layer.protect = reader.read_u32()?;
        }
    }

    // Layer names and group names are stored later in the header block.
    // If this optional extraction fails, keep deterministic default names.
    let diagnostic_checkpoint = reader.decode_diagnostic_count();
    let (palette, line_types) =
        if parse_layer_names(&mut reader, version, &mut layer_groups).is_err() {
            // This section is optional and layout-dependent. Discard diagnostics
            // collected while probing bytes that were not confirmed as names.
            reader.truncate_decode_diagnostics(diagnostic_checkpoint);
            apply_default_layer_names(&mut layer_groups);
            // The read position is already lost, so every later offset is unreliable.
            (None, None)
        } else {
            apply_default_layer_names_for_blanks(&mut layer_groups);
            match parse_display_settings(&mut reader, version) {
                Ok((palette, line_types)) => (Some(palette), line_types),
                Err(_) => (None, None),
            }
        };

    Ok((
        JwwHeader {
            version,
            memo,
            paper_size,
            write_layer_group,
            layer_groups,
            palette,
            line_types,
            text_presets: if (600..=700).contains(&version) {
                parse_text_presets(data).ok()
            } else {
                None
            },
        },
        reader.into_decode_diagnostics(),
    ))
}

fn parse_text_presets(data: &[u8]) -> Result<Vec<TextPreset>, JwwError> {
    let end = crate::header_layout::header_end_v700(data)?;
    let mut reader = Reader::new(data);
    reader.skip(end - (10 * 28 + 32 + 16 + 4 + 48))?;
    (0..10)
        .map(|_| {
            Ok(TextPreset {
                size_x: reader.read_f64()?,
                size_y: reader.read_f64()?,
                spacing: reader.read_f64()?,
                pen_color: reader.read_u32()?,
            })
        })
        .collect()
}

/// Reads the screen color palette and the line type settings that follow the layer group names.
///
/// Everything between the names and the SXF color names is fixed width,
/// so plain skips are enough to get to the palette and the standard line types.
/// The SXF-compatible line types sit behind the user defined color names,
/// which are variable length strings.
fn parse_display_settings(
    reader: &mut Reader<'_>,
    version: u32,
) -> Result<(JwwPalette, Option<JwwLineTypes>), JwwError> {
    // Below version 300 the zoom and dummy sections have a different layout.
    if version < 300 {
        return Err(JwwError::UnexpectedEof("header.palette"));
    }

    reader.skip(
        8 + 8 + 4 + 8    // sunlight calc: level, latitude, 9-15 flag, wall level
        + 16             // sky factor diagram: level, radius*2 (version >= 300)
        + 4              // 2.5D calculation unit
        + 8 + 16         // saved screen zoom and origin (x,y)
        + 8 + 16         // stored-range zoom and base point (x,y)
        + 224            // 8 zoom slots x (zoom f64 + origin f64*2 + layer group u32)
        + 56             // dummies f64*3 + u32 + f64*2 + text background f64 + u32
        + 80             // parallel line spacing 10 x f64
        + 8, // stub length for two-sided parallel lines
    )?;

    // Screen color and pen width per pen color number.
    let mut pen_colors = [0u32; 10];
    for color in &mut pen_colors {
        *color = normalize_colorref(reader.read_u32()?);
        let _pen_width = reader.read_u32()?;
    }

    if version < 420 {
        // Older files end the color data here. The line types follow directly;
        // a short or damaged section only loses them, not the palette.
        let line_types = parse_line_type_patterns(reader).ok();
        return Ok((
            JwwPalette {
                pen_colors,
                extended_colors: None,
            },
            line_types.filter(JwwLineTypes::is_plausible),
        ));
    }

    let mut line_types = parse_line_type_patterns(reader)?;
    reader.skip(
        44           // dot drawing, reverse draw/search and print flags: u32 x 11
        + 20         // draw time, 2.5D start flag, horizontal eye angles: u32*5
        + 40         // 2.5D eye height, distance and vertical angle: f64 x 5
        + 32         // last used line length, box width/height, circle radius: f64 x 4
        + 8, // arbitrary solid color flag and its default value
    )?;
    // The file stores 257 entries for pen color numbers 100..=356.
    // Number 100 is a spare that duplicates black and carries no color name,
    // 101..=116 are the SXF standard colors, and 117..=356 are user defined.
    let mut colors = vec![0u32; 257].into_boxed_slice();
    for color in colors.iter_mut() {
        *color = normalize_colorref(reader.read_u32()?);
        let _pen_width = reader.read_u32()?;
    }

    // The SXF line types are optional for the caller: when the section is short or
    // implausible, keep the palette and the standard line types read so far.
    let diagnostic_checkpoint = reader.decode_diagnostic_count();
    match parse_sxf_line_types(reader) {
        Ok(sxf) => line_types.sxf = Some(sxf),
        Err(_) => reader.truncate_decode_diagnostics(diagnostic_checkpoint),
    }

    Ok((
        JwwPalette {
            pen_colors,
            extended_colors: Some(colors),
        },
        Some(line_types).filter(JwwLineTypes::is_plausible),
    ))
}

/// Reads the printer colors and the standard, random and double-length line types
/// that follow the screen pen colors.
fn parse_line_type_patterns(reader: &mut Reader<'_>) -> Result<JwwLineTypes, JwwError> {
    reader.skip(160)?; // printer color, width, dot radius per pen: 10 x (u32*2 + f64)

    let mut standard = Vec::with_capacity(8);
    for index in 0..8 {
        standard.push(read_line_type_pattern(
            reader,
            STANDARD_LINE_TYPE_BASE + index,
        )?);
    }
    let mut random = Vec::with_capacity(5);
    for index in 0..5 {
        random.push(RandomLineType {
            number: RANDOM_LINE_TYPE_BASE + index,
            pattern: reader.read_u32()?,
            width: reader.read_u32()?,
            pitch: reader.read_u32()?,
            printer_width: reader.read_u32()?,
            printer_pitch: reader.read_u32()?,
        });
    }
    let mut double_length = Vec::with_capacity(4);
    for index in 0..4 {
        double_length.push(read_line_type_pattern(
            reader,
            DOUBLE_LENGTH_LINE_TYPE_BASE + index,
        )?);
    }
    Ok(JwwLineTypes {
        standard,
        random,
        double_length,
        sxf: None,
    })
}

fn read_line_type_pattern(
    reader: &mut Reader<'_>,
    number: u32,
) -> Result<LineTypePattern, JwwError> {
    Ok(LineTypePattern {
        number,
        pattern: reader.read_u32()?,
        unit_dots: reader.read_u32()?,
        pitch: reader.read_u32()?,
        printer_pitch: reader.read_u32()?,
    })
}

/// Reads the SXF-compatible line types, starting right after the extended screen colors.
fn parse_sxf_line_types(reader: &mut Reader<'_>) -> Result<Vec<SxfLineType>, JwwError> {
    // Color name, printer color, printer width and dot radius per extended color.
    for index in 0..257 {
        let field = format!("header.extended_colors[{index}].name");
        let _name = reader.read_cstring_with_context(&field)?;
        reader.skip(4 + 4 + 8)?;
    }

    let mut line_types = Vec::with_capacity(33);
    for index in 0..33 {
        line_types.push(SxfLineType {
            pattern: read_line_type_pattern(reader, SXF_LINE_TYPE_BASE + index)?,
            ..SxfLineType::default()
        });
    }
    for (index, line_type) in line_types.iter_mut().enumerate() {
        let field = format!("header.line_types.sxf[{index}].name");
        line_type.name = reader.read_cstring_with_context(&field)?;
        let segments = reader.read_u32()?;
        let mut lengths = [0f64; 10];
        for length in &mut lengths {
            *length = reader.read_f64()?;
        }
        if segments as usize > lengths.len() || lengths.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Err(JwwError::UnexpectedEof("header.line_types.sxf"));
        }
        line_type.segments_mm = lengths[..segments as usize].to_vec();
    }
    Ok(line_types)
}

impl JwwLineTypes {
    /// A header whose read position drifted yields arbitrary numbers here.
    /// Every real file keeps the unit within the 32 bits of the pattern.
    fn is_plausible(&self) -> bool {
        self.standard
            .iter()
            .chain(&self.double_length)
            .all(|pattern| (1..=32).contains(&pattern.unit_dots))
    }
}

/// Jw_cad may set the Win32 `PALETTERGB` marker (`0x02` in the high byte).
/// The low three bytes still contain the COLORREF components, so the semantic
/// palette model strips the GDI marker and exposes a stable `0x00BBGGRR` value.
const fn normalize_colorref(color: u32) -> u32 {
    color & 0x00FF_FFFF
}

fn parse_layer_names(
    reader: &mut Reader<'_>,
    version: u32,
    layer_groups: &mut [LayerGroupHeader; 16],
) -> Result<(), JwwError> {
    // Only version >= 300 layout is currently supported for this section.
    if version < 300 {
        return Err(JwwError::UnexpectedEof("layer names"));
    }

    // Skip fields defined before layer names in jwdatafmt:
    // 14 dummy DWORD + 5 dimension DWORD + 1 dummy DWORD + max-draw-width DWORD.
    reader.skip((14 + 5 + 1 + 1) * 4)?;

    // Printer/memory settings before names:
    // printer origin(x,y) [16]
    // printer scale [8]
    // printer set [4]
    // memori mode [4]
    // memori min [8]
    // memori x/y [16]
    // memori origin x/y [16]
    reader.skip(16 + 8 + 4 + 4 + 8 + 16 + 16)?;

    for (group_index, group) in layer_groups.iter_mut().enumerate() {
        for (layer_index, layer) in group.layers.iter_mut().enumerate() {
            let field = format!("header.layer_groups[{group_index}].layers[{layer_index}].name");
            layer.name = reader.read_cstring_with_context(&field)?;
        }
    }

    for (group_index, group) in layer_groups.iter_mut().enumerate() {
        let field = format!("header.layer_groups[{group_index}].name");
        group.name = reader.read_cstring_with_context(&field)?;
    }

    Ok(())
}

fn apply_default_layer_names(layer_groups: &mut [LayerGroupHeader; 16]) {
    for (g_idx, group) in layer_groups.iter_mut().enumerate() {
        group.name = format!("Group{:X}", g_idx);
        for (l_idx, layer) in group.layers.iter_mut().enumerate() {
            layer.name = format!("{:X}-{:X}", g_idx, l_idx);
        }
    }
}

fn apply_default_layer_names_for_blanks(layer_groups: &mut [LayerGroupHeader; 16]) {
    for (g_idx, group) in layer_groups.iter_mut().enumerate() {
        if group.name.is_empty() {
            group.name = format!("Group{:X}", g_idx);
        }
        for (l_idx, layer) in group.layers.iter_mut().enumerate() {
            if layer.name.is_empty() {
                layer.name = format!("{:X}-{:X}", g_idx, l_idx);
            }
        }
    }
}

pub fn read_header_from_file(path: impl AsRef<Path>) -> Result<JwwHeader, JwwError> {
    let data = fs::read(path)?;
    parse_header(&data)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{
        is_jww_signature, normalize_colorref, parse_header, read_header_from_file, JwwError,
        JwwPalette, LineTypePattern,
    };

    fn jww_samples_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../jww_samples")
    }

    #[test]
    fn signature_check() {
        assert!(is_jww_signature(b"JwwData.\x00\x00"));
        assert!(!is_jww_signature(b"NotJwwData"));
    }

    #[test]
    fn palette_is_read_from_real_sample() {
        let path = jww_samples_dir().join("Ａマンション平面例.jww");
        let header = read_header_from_file(&path).expect("sample header");
        let palette = header.palette.expect("palette");

        // COLORREF is 0x00BBGGRR, so #00C0C0 is stored as 0x00C0C000.
        assert_eq!(
            palette.pen_colors,
            [
                0x00FF_FFFF, // 0: background (white)
                0x00C0_C000, // 1: #00C0C0 cyan
                0x0000_0000, // 2: #000000 black
                0x0000_C000, // 3: #00C000 green
                0x0000_C0C0, // 4: #C0C000 yellow
                0x00C0_00C0, // 5: #C000C0 magenta
                0x00FF_0000, // 6: #0000FF blue
                0x0080_8000, // 7: #008080 teal
                0x0080_00FF, // 8: #FF0080 pink
                0x00C0_C0C0, // 9: #C0C0C0 light gray
            ]
        );

        // The extended palette starts at pen color 100. The SXF standard colors
        // occupy 101..=116, followed by user-defined colors through 356.
        let extended = palette.extended_colors.expect("extended palette");
        assert_eq!(extended[0], 0x0000_0000); // 100 = spare
        assert_eq!(extended[1], 0x0000_0000); // 101 = black
        assert_eq!(extended[2], 0x0000_00FF); // 102 = red
        assert_eq!(extended[5], 0x0000_FFFF); // 105 = yellow
        assert_eq!(extended[8], 0x00FF_FFFF); // 108 = white
        assert_eq!(extended[15], 0x00C0_C0C0); // 115 = lightgray
        assert_eq!(extended[16], 0x0080_8080); // 116 = darkgray
        assert_eq!(extended[17], 0x00C0_C0C0); // 117 = first user-defined color
        assert_eq!(extended.len(), 257);
    }

    #[test]
    fn palette_lookup_covers_basic_and_all_extended_numbers() {
        let palette = JwwPalette {
            pen_colors: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            extended_colors: Some((100u32..=356).collect::<Vec<_>>().into_boxed_slice()),
        };
        assert_eq!(palette.screen_color(3), Some(3));
        assert_eq!(palette.screen_color(9), Some(9));
        assert_eq!(palette.screen_color(100), Some(100));
        assert_eq!(palette.screen_color(101), Some(101));
        assert_eq!(palette.screen_color(102), Some(102));
        assert_eq!(palette.screen_color(116), Some(116));
        assert_eq!(palette.screen_color(117), Some(117));
        assert_eq!(palette.screen_color(257), Some(257));
        assert_eq!(palette.screen_color(356), Some(356));

        // Pen color 0 is the background, not a pen.
        assert_eq!(palette.screen_color(0), None);
        // Numbers outside both ranges.
        assert_eq!(palette.screen_color(10), None);
        assert_eq!(palette.screen_color(99), None);
        assert_eq!(palette.screen_color(357), None);

        // Files below version 420 carry no SXF extended colors.
        let no_extended = JwwPalette {
            extended_colors: None,
            ..palette
        };
        assert_eq!(no_extended.screen_color(102), None);
        assert_eq!(no_extended.screen_color(257), None);

        // A manually constructed public value with an invalid length remains
        // safe to query, even though parsed version-420 files always hold 257 entries.
        let truncated = JwwPalette {
            extended_colors: Some(vec![100].into_boxed_slice()),
            ..no_extended
        };
        assert_eq!(truncated.screen_color(100), Some(100));
        assert_eq!(truncated.screen_color(101), None);
    }

    #[test]
    fn colorref_normalization_strips_the_palettergb_marker() {
        assert_eq!(normalize_colorref(0x02BF_00FF), 0x00BF_00FF);
        assert_eq!(normalize_colorref(0x00C0_C000), 0x00C0_C000);
    }

    fn line_type(pattern: u32, unit_dots: u32, printer_pitch: u32) -> LineTypePattern {
        LineTypePattern {
            number: 0,
            pattern,
            unit_dots,
            pitch: 1,
            printer_pitch,
        }
    }

    #[test]
    fn line_type_runs_follow_the_pattern_bits() {
        // The Jw_cad default patterns of line types 2..=9.
        assert_eq!(line_type(0x9999_9999, 4, 10).runs(), [2, 2]);
        assert_eq!(line_type(0xC3C3_C3C3, 8, 10).runs(), [4, 4]);
        assert_eq!(line_type(0xE7E7_E7E7, 8, 10).runs(), [6, 2]);
        assert_eq!(line_type(0xF99F_F99F, 16, 10).runs(), [10, 2, 2, 2]);
        assert_eq!(line_type(0xFFF9_9FFF, 32, 10).runs(), [26, 2, 2, 2]);
        assert_eq!(line_type(0xF24F_F24F, 16, 10).runs(), [8, 2, 1, 2, 1, 2]);
        assert_eq!(line_type(0xFFF2_4FFF, 32, 10).runs(), [24, 2, 1, 2, 1, 2]);
        assert_eq!(line_type(0x2222_2222, 4, 10).runs(), [1, 3]);
        // Only the lowest `unit_dots` bits count.
        assert_eq!(line_type(0xFFFF_FF0F, 8, 10).runs(), [4, 4]);

        // A solid pattern, an empty pattern and an unusable unit have no runs.
        assert!(line_type(0xFFFF_FFFF, 32, 10).runs().is_empty());
        assert!(line_type(0, 32, 10).runs().is_empty());
        assert!(line_type(0x9999_9999, 0, 10).runs().is_empty());
        assert!(line_type(0x9999_9999, 33, 10).runs().is_empty());
    }

    #[test]
    fn line_type_printed_lengths_scale_with_the_printer_pitch() {
        // One pattern bit is printer pitch / 32 millimetres.
        assert_eq!(
            line_type(0xF99F_F99F, 16, 10).printed_segments_mm(),
            [3.125, 0.625, 0.625, 0.625]
        );
        assert_eq!(
            line_type(0xF99F_F99F, 16, 5).printed_segments_mm(),
            [1.5625, 0.3125, 0.3125, 0.3125]
        );
        assert!(line_type(0xFFFF_FFFF, 32, 10)
            .printed_segments_mm()
            .is_empty());
    }

    #[test]
    fn line_types_are_read_from_real_sample() {
        let path = jww_samples_dir().join("Test1.jww");
        let header = read_header_from_file(&path).expect("sample header");
        let line_types = header.line_types.expect("line types");

        let numbers = |patterns: &[LineTypePattern]| -> Vec<u32> {
            patterns.iter().map(|pattern| pattern.number).collect()
        };
        assert_eq!(numbers(&line_types.standard), [2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(numbers(&line_types.double_length), [16, 17, 18, 19]);
        assert_eq!(
            line_types
                .random
                .iter()
                .map(|line_type| line_type.number)
                .collect::<Vec<_>>(),
            [11, 12, 13, 14, 15]
        );
        // Chain line 1 with the Jw_cad default settings.
        assert_eq!(
            line_types.standard[3],
            LineTypePattern {
                number: 5,
                pattern: 0xF99F_F99F,
                unit_dots: 16,
                pitch: 1,
                printer_pitch: 10,
            }
        );
        assert_eq!(line_types.double_length[0].printer_pitch, 20);

        // 33 SXF-compatible slots: line type numbers 30..=62.
        let sxf = line_types.sxf.as_ref().expect("SXF line types");
        assert_eq!(sxf.len(), 33);
        assert_eq!(sxf[0].pattern.number, 30);
        assert_eq!(sxf[32].pattern.number, 62);
        assert_eq!(sxf[1].name, "continuous");
        assert!(sxf[1].segments_mm.is_empty());
        assert_eq!(sxf[2].name, "dashed");
        assert_eq!(sxf[2].segments_mm, [6.0, 1.5]);
        assert_eq!(sxf[8].name, "chain");
        assert_eq!(sxf[8].segments_mm, [12.0, 1.5, 3.5, 1.5]);
        // The sample defines no user line types.
        assert!(sxf[17..]
            .iter()
            .all(|line_type| line_type.segments_mm.is_empty()));

        // Lookup by line type number: bit pattern types derive their lengths,
        // SXF types report the stored ones.
        assert_eq!(line_types.pattern(5), Some(&line_types.standard[3]));
        assert_eq!(line_types.pattern(32), Some(&sxf[2].pattern));
        assert_eq!(line_types.pattern(11), None);
        assert_eq!(
            line_types.printed_segments_mm(5),
            [3.125, 0.625, 0.625, 0.625]
        );
        assert_eq!(line_types.printed_segments_mm(32), [6.0, 1.5]);
        assert!(line_types.printed_segments_mm(1).is_empty());
        assert!(line_types.printed_segments_mm(11).is_empty());
    }

    /// Header bytes of a file with empty names, up to the screen pen colors.
    fn header_through_pen_colors(version: u32) -> Vec<u8> {
        let mut data = Vec::<u8>::new();
        data.extend_from_slice(b"JwwData.");
        data.extend_from_slice(&version.to_le_bytes());
        data.push(0); // memo
        data.extend_from_slice(&[0u8; 8]); // paper size, write layer group
        for _ in 0..16 {
            data.extend_from_slice(&[0u8; 8]); // state, write layer
            data.extend_from_slice(&1.0f64.to_le_bytes()); // scale
            data.extend_from_slice(&[0u8; 4]); // protect
            data.extend_from_slice(&[0u8; 16 * 8]); // layer state, protect
        }
        data.extend_from_slice(&[0u8; (14 + 5 + 1 + 1) * 4]);
        data.extend_from_slice(&[0u8; 16 + 8 + 4 + 4 + 8 + 16 + 16]);
        data.extend_from_slice(&[0u8; 16 * 16 + 16]); // layer and layer group names
        data.extend_from_slice(&[0u8; 464]); // sunlight settings .. parallel line stub
        for index in 0..10u32 {
            data.extend_from_slice(&index.to_le_bytes()); // screen color
            data.extend_from_slice(&1u32.to_le_bytes()); // pen width
        }
        data
    }

    fn push_u32s(data: &mut Vec<u8>, values: &[u32]) {
        for value in values {
            data.extend_from_slice(&value.to_le_bytes());
        }
    }

    /// Printer colors and the standard, random and double-length line types.
    fn append_line_type_patterns(data: &mut Vec<u8>, unit_dots: u32, printer_pitch: u32) {
        data.extend_from_slice(&[0u8; 160]); // printer colors
        for _ in 2..=9 {
            push_u32s(data, &[0x9999_9999, unit_dots, 1, printer_pitch]);
        }
        for _ in 11..=15 {
            push_u32s(data, &[0x1234_5678, 2, 3, 4, 5]);
        }
        for _ in 16..=19 {
            push_u32s(data, &[0xFFFE_7FFF, 32, 2, 20]);
        }
    }

    /// Print flags .. solid color, the extended screen colors and their names.
    fn append_extended_colors(data: &mut Vec<u8>) {
        data.extend_from_slice(&[0u8; 44 + 20 + 40 + 32 + 8]);
        data.extend_from_slice(&[0u8; 257 * 8]); // screen color, width
        for _ in 0..257 {
            data.push(0); // color name
            data.extend_from_slice(&[0u8; 4 + 4 + 8]); // printer color, width, dot radius
        }
    }

    fn append_sxf_line_types(data: &mut Vec<u8>, user_name: &[u8], user_segments: &[f64]) {
        for _ in 0..33 {
            push_u32s(data, &[0xFFFF_FFFF, 32, 1, 10]);
        }
        for index in 0..33 {
            let (name, segments): (&[u8], &[f64]) = if index == 17 {
                (user_name, user_segments)
            } else {
                (b"", &[])
            };
            data.push(name.len() as u8);
            data.extend_from_slice(name);
            data.extend_from_slice(&(segments.len() as u32).to_le_bytes());
            for slot in 0..10 {
                let value = segments.get(slot).copied().unwrap_or(0.0);
                data.extend_from_slice(&value.to_le_bytes());
            }
        }
    }

    #[test]
    fn line_types_keep_the_settings_of_the_file() {
        let mut data = header_through_pen_colors(600);
        append_line_type_patterns(&mut data, 4, 5);
        append_extended_colors(&mut data);
        append_sxf_line_types(&mut data, b"user dash", &[2.0, 1.0, 0.5, 1.0]);

        let header = parse_header(&data).expect("header");
        assert!(header.palette.expect("palette").extended_colors.is_some());
        let line_types = header.line_types.expect("line types");
        // A printer pitch of 5 halves the printed pattern of the default 10.
        assert_eq!(line_types.standard[0].printer_pitch, 5);
        assert_eq!(line_types.printed_segments_mm(2), [0.3125, 0.3125]);
        assert_eq!(line_types.random[0].printer_pitch, 5);
        assert_eq!(line_types.random[4].number, 15);

        // The first user-defined SXF line type is line type number 47.
        let sxf = line_types.sxf.as_ref().expect("SXF line types");
        assert_eq!(sxf[17].pattern.number, 47);
        assert_eq!(sxf[17].name, "user dash");
        assert_eq!(sxf[17].segments_mm, [2.0, 1.0, 0.5, 1.0]);
        assert_eq!(line_types.printed_segments_mm(47), [2.0, 1.0, 0.5, 1.0]);
    }

    #[test]
    fn truncated_sxf_line_types_keep_palette_and_standard_line_types() {
        let mut data = header_through_pen_colors(600);
        append_line_type_patterns(&mut data, 4, 10);
        append_extended_colors(&mut data);
        append_sxf_line_types(&mut data, b"user dash", &[2.0, 1.0]);
        data.truncate(data.len() - 40);

        let header = parse_header(&data).expect("header");
        assert!(header.palette.is_some());
        let line_types = header.line_types.expect("line types");
        assert_eq!(line_types.standard.len(), 8);
        assert_eq!(line_types.sxf, None);
    }

    #[test]
    fn sxf_line_types_with_too_many_segments_are_dropped() {
        let mut data = header_through_pen_colors(600);
        append_line_type_patterns(&mut data, 4, 10);
        append_extended_colors(&mut data);
        // 11 segments cannot be stored in the 10 length slots.
        append_sxf_line_types(&mut data, b"bad", &[1.0; 11]);

        let line_types = parse_header(&data)
            .expect("header")
            .line_types
            .expect("line types");
        assert_eq!(line_types.sxf, None);
    }

    #[test]
    fn implausible_line_types_are_not_reported() {
        // A unit of 77 bits does not fit the 32-bit pattern: the read position drifted.
        let mut data = header_through_pen_colors(600);
        append_line_type_patterns(&mut data, 77, 10);
        append_extended_colors(&mut data);
        append_sxf_line_types(&mut data, b"", &[]);

        let header = parse_header(&data).expect("header");
        assert!(header.palette.is_some());
        assert_eq!(header.line_types, None);
    }

    #[test]
    fn line_types_before_version_420_have_no_sxf_section() {
        let mut data = header_through_pen_colors(351);
        append_line_type_patterns(&mut data, 4, 10);

        let header = parse_header(&data).expect("header");
        assert_eq!(header.palette.expect("palette").extended_colors, None);
        let line_types = header.line_types.expect("line types");
        assert_eq!(line_types.standard[7].number, 9);
        assert_eq!(line_types.sxf, None);

        // A file that ends right after the pen colors still has its palette.
        let data = header_through_pen_colors(351);
        let header = parse_header(&data).expect("header");
        assert!(header.palette.is_some());
        assert_eq!(header.line_types, None);
    }

    #[test]
    fn invalid_signature_is_rejected() {
        let err = parse_header(b"NotJwwData").unwrap_err();
        assert!(matches!(err, JwwError::InvalidSignatureFound(_)));
        let err = parse_header(b"\x00\x01\x02\x03\x04\x05\x06\x07").unwrap_err();
        assert!(matches!(err, JwwError::InvalidSignature));
    }

    #[test]
    fn parse_all_jww_sample_headers() {
        let dir = jww_samples_dir();
        assert!(
            dir.exists(),
            "jww_samples directory is required for this test: {}",
            dir.display()
        );

        let mut files = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().map(|ext| ext == "jww").unwrap_or(false))
            .collect::<Vec<_>>();
        files.sort();

        assert!(
            !files.is_empty(),
            "no .jww files found in {}",
            dir.display()
        );

        for path in files {
            let header = read_header_from_file(&path)
                .unwrap_or_else(|e| panic!("failed parsing {}: {e}", path.display()));
            assert_eq!(
                header.version,
                600,
                "unexpected version in {}",
                path.display()
            );
            assert_eq!(header.layer_groups.len(), 16);
            for group in &header.layer_groups {
                assert_eq!(group.layers.len(), 16);
                assert!(
                    !group.name.is_empty(),
                    "group name should not be empty in {}",
                    path.display()
                );
                for layer in &group.layers {
                    assert!(
                        !layer.name.is_empty(),
                        "layer name should not be empty in {}",
                        path.display()
                    );
                }
            }
        }
    }

    #[test]
    fn extracts_non_default_layer_names_when_present() {
        let path = jww_samples_dir().join("Ａマンション平面例.jww");
        if !path.exists() {
            return;
        }

        let header = read_header_from_file(&path).unwrap();
        let group0 = &header.layer_groups[0];
        let layer0 = &group0.layers[0];

        assert_ne!(group0.name, "Group0");
        assert_ne!(layer0.name, "0-0");
    }
}
