use serde::Serialize;

use super::header::JwcEntityCounts;
use super::reader::Reader;
use super::{JwcError, JWC_FIXED_HEADER_SIZE, JWC_MAX_TEXT_BYTES, JWC_NAMES_SIZE};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct JwcSection {
    pub byte_offset: usize,
    pub byte_length: usize,
}

/// Verified framing only. These spans do not certify geometry or attributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JwcLayout {
    pub lines: JwcSection,
    pub arcs: JwcSection,
    pub text_records: JwcSection,
    pub string_pool: JwcSection,
    pub points: JwcSection,
    pub names: JwcSection,
}

pub(super) fn validate_layout(
    data: &[u8],
    counts: JwcEntityCounts,
    pool_length: usize,
) -> Result<JwcLayout, JwcError> {
    let mut offset = JWC_FIXED_HEADER_SIZE;
    let mut section = |count: usize, stride: usize, field: &str| -> Result<JwcSection, JwcError> {
        let length = count
            .checked_mul(stride)
            .ok_or_else(|| JwcError::limit(offset, field, "record length overflow"))?;
        let start = offset;
        offset = offset
            .checked_add(length)
            .ok_or_else(|| JwcError::limit(offset, field, "record end overflow"))?;
        Ok(JwcSection {
            byte_offset: start,
            byte_length: length,
        })
    };
    let lines = section(counts.lines as usize, 22, "lines")?;
    let arcs = section(counts.arcs as usize, 32, "arcs")?;
    let text_records = section(counts.texts as usize, 24, "text_records")?;
    let string_pool = section(pool_length, 1, "string_pool")?;
    let points = section(counts.points as usize, 12, "points")?;
    let names = section(1, JWC_NAMES_SIZE, "names")?;
    if data.len() < offset {
        return Err(JwcError::UnexpectedEof {
            offset: data.len(),
            field: "file".into(),
            needed: offset - data.len(),
            available: 0,
        });
    }
    if data.len() > offset {
        return Err(JwcError::unsupported(
            offset,
            "file",
            "unexplained trailing bytes",
        ));
    }
    let reader = Reader::new(data);
    let pool = reader.bytes(string_pool.byte_offset, pool_length, "string_pool")?;
    let mut cursor = 0;
    for index in 0..counts.texts as usize {
        let reference_offset = text_records.byte_offset + 24 * index + 16;
        let reference = reader.u32(reference_offset, "text.string_reference")?;
        if reference & 0xc000_0000 != 0x4000_0000 {
            return Err(JwcError::unsupported(
                reference_offset,
                "text.string_reference",
                "unverified reference high bits",
            ));
        }
        let relative = (reference & 0x3fff_ffff) as usize;
        if relative >= pool_length || relative < cursor {
            return Err(JwcError::invalid(
                reference_offset,
                "text.string_reference",
                "out-of-range or overlapping string reference",
            ));
        }
        if relative != cursor {
            return Err(JwcError::unsupported(
                reference_offset,
                "text.string_reference",
                "unexplained gap in string pool",
            ));
        }
        // A scan is confined to the declared pool and the observed text limit.
        let search_end = pool_length.min(cursor + JWC_MAX_TEXT_BYTES + 1);
        let length = pool[cursor..search_end]
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| {
                if search_end - cursor > JWC_MAX_TEXT_BYTES {
                    JwcError::limit(
                        string_pool.byte_offset + cursor,
                        "text.content",
                        "text exceeds 256 bytes",
                    )
                } else {
                    JwcError::invalid(
                        string_pool.byte_offset + cursor,
                        "text.content",
                        "missing NUL terminator within string pool",
                    )
                }
            })?;
        cursor += length + 1;
    }
    if cursor != pool_length {
        return Err(JwcError::unsupported(
            string_pool.byte_offset + cursor,
            "string_pool",
            "unreferenced bytes in string pool",
        ));
    }
    Ok(JwcLayout {
        lines,
        arcs,
        text_records,
        string_pool,
        points,
        names,
    })
}
