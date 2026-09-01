use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::diagnostics::DecodeDiagnostic;
use crate::error::JwwError;
use crate::reader::Reader;

pub const JWW_SIGNATURE: &[u8; 8] = b"JwwData.";

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct LayerHeader {
    pub state: u32,
    pub protect: u32,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct LayerGroupHeader {
    pub state: u32,
    pub write_layer: u32,
    pub scale: f64,
    pub protect: u32,
    pub layers: [LayerHeader; 16],
    pub name: String,
}

/// Screen colors recorded in the JWW header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JwwHeader {
    pub version: u32,
    pub memo: String,
    pub paper_size: u32,
    pub write_layer_group: u32,
    pub layer_groups: [LayerGroupHeader; 16],
    /// Only available when the layer name section was parsed successfully.
    pub palette: Option<JwwPalette>,
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
    let palette = if parse_layer_names(&mut reader, version, &mut layer_groups).is_err() {
        // This section is optional and layout-dependent. Discard diagnostics
        // collected while probing bytes that were not confirmed as names.
        reader.truncate_decode_diagnostics(diagnostic_checkpoint);
        apply_default_layer_names(&mut layer_groups);
        // The read position is already lost, so every later offset is unreliable.
        None
    } else {
        apply_default_layer_names_for_blanks(&mut layer_groups);
        parse_palette(&mut reader, version).ok()
    };

    Ok((
        JwwHeader {
            version,
            memo,
            paper_size,
            write_layer_group,
            layer_groups,
            palette,
        },
        reader.into_decode_diagnostics(),
    ))
}

/// Advances from just after the layer group names to the screen color palette.
///
/// Everything between the names and the palette is fixed width,
/// and the only variable length strings (the user defined color names) live after the printer color block,
/// so plain skips are enough to get there.
fn parse_palette(reader: &mut Reader<'_>, version: u32) -> Result<JwwPalette, JwwError> {
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

    let extended_colors = if version >= 420 {
        reader.skip(
            160          // printer color, width, dot radius per pen: 10 x (u32*2 + f64)
            + 128        // line type patterns 2-9: 8 x u32*4
            + 100        // random line patterns 1-5: 5 x u32*5
            + 64         // double length line type patterns 6-9: 4 x u32*4
            + 44         // dot drawing, reverse draw/search and print flags: u32 x 11
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
        Some(colors)
    } else {
        None
    };

    Ok(JwwPalette {
        pen_colors,
        extended_colors,
    })
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
        JwwPalette,
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
