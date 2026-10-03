//! Structural traversal of the complete version-700 header in jwdatafmt.txt.
//! No class-tag search or fixed absolute offsets: every CString is length-prefixed.

use crate::{error::JwwError, reader::Reader};

pub(crate) fn header_end_v700(data: &[u8]) -> Result<usize, JwwError> {
    let mut reader = Reader::new(data);
    reader.skip(12)?; // signature and version (already checked by parse_header)
    skip_cstring(&mut reader)?; // memo
    reader.skip(8 + 16 * 148)?; // paper, current group, group/layer tables
    reader.skip(84 + 72)?; // dimension/dummy fields, printer and grid settings
    for _ in 0..272 {
        skip_cstring(&mut reader)?; // 256 layer names, 16 group names
    }
    reader.skip(
        28 + 16 + 4 // sunlight, sky diagram, 2.5D unit
        + 24 + 24 + 8 * 28 // view, stored range, zoom slots
        + 56 + 80 + 8 // text background, parallel lines
        + 10 * 8 + 10 * 16 // screen and printer pens
        + 8 * 16 + 5 * 20 + 4 * 16 // standard, random and double-length styles
        + 44 + 20 + 40 + 32 + 8 // display/print, 2.5D, last values, solid color
        + 257 * 8, // SXF screen pens
    )?;
    for _ in 0..257 {
        skip_cstring(&mut reader)?;
        reader.skip(16)?; // SXF printer color, width, point radius
    }
    reader.skip(33 * 16)?; // SXF line patterns
    for _ in 0..33 {
        skip_cstring(&mut reader)?;
        reader.skip(4 + 10 * 8)?; // segment count and ten pitches (always stored)
    }
    reader.skip(10 * 28 + 32 + 16 + 4 + 48)?; // text presets and alignment
    Ok(reader.bytes_read())
}

/// Skip either CP932 or UTF-16LE CStrings without allocating from an input length.
fn skip_cstring(reader: &mut Reader<'_>) -> Result<(), JwwError> {
    let first = reader.read_u8()?;
    let mut unicode = false;
    let length = if first < 0xff {
        usize::from(first)
    } else {
        let word = reader.read_u16()?;
        if word == 0xfffe {
            unicode = true;
            let inner = reader.read_u8()?;
            if inner < 0xff {
                usize::from(inner)
            } else {
                extended_length(reader)?
            }
        } else if word == 0xffff {
            reader.read_u32()? as usize
        } else {
            usize::from(word)
        }
    };
    let bytes = length
        .checked_mul(if unicode { 2 } else { 1 })
        .ok_or(JwwError::UnexpectedEof("cstring length"))?;
    reader.skip(bytes)
}

fn extended_length(reader: &mut Reader<'_>) -> Result<usize, JwwError> {
    let word = reader.read_u16()?;
    if word == 0xffff {
        Ok(reader.read_u32()? as usize)
    } else {
        Ok(usize::from(word))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_string_lengths_are_checked_without_allocating() {
        for bytes in [
            vec![255, 255, 255, 255, 255, 255, 255],
            vec![255, 254, 255, 255, 255, 255, 255, 255, 255, 255],
            vec![255, 254, 255, 2, 65, 0], // two UTF-16 units, only one present
        ] {
            assert!(skip_cstring(&mut Reader::new(&bytes)).is_err());
        }
    }
}
