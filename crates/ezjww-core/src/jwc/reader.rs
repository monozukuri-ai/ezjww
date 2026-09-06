//! Checked JWC reads; deliberately independent of MFC CString and JwwError.

use encoding_rs::SHIFT_JIS;

use super::error::JwcError;
use super::header::JwcName;
use crate::diagnostics::Diagnostic;

pub(super) struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub fn bytes(&self, offset: usize, length: usize, field: &str) -> Result<&'a [u8], JwcError> {
        let end = offset
            .checked_add(length)
            .ok_or_else(|| JwcError::limit(offset, field, "byte range overflow"))?;
        self.data
            .get(offset..end)
            .ok_or_else(|| JwcError::UnexpectedEof {
                offset,
                field: field.into(),
                needed: length,
                available: self.data.len().saturating_sub(offset),
            })
    }

    pub fn u16(&self, offset: usize, field: &str) -> Result<u16, JwcError> {
        let b = self.bytes(offset, 2, field)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&self, offset: usize, field: &str) -> Result<u32, JwcError> {
        let b = self.bytes(offset, 4, field)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn finite_f32(&self, offset: usize, field: &str) -> Result<f32, JwcError> {
        let value = f32::from_bits(self.u32(offset, field)?);
        if !value.is_finite() {
            return Err(JwcError::invalid(offset, field, "expected a finite f32"));
        }
        Ok(value)
    }

    pub fn fixed_cp932(
        &self,
        offset: usize,
        width: usize,
        content_limit: usize,
        field: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<JwcName, JwcError> {
        let raw = self.bytes(offset, width, field)?;
        let length = raw.iter().position(|&b| b == 0).ok_or_else(|| {
            JwcError::invalid(offset, field, "missing NUL terminator in name slot")
        })?;
        if length > content_limit {
            return Err(JwcError::unsupported(
                offset,
                field,
                "name exceeds observed slot capacity",
            ));
        }
        if raw[length..].iter().any(|&b| b != 0) {
            return Err(JwcError::unsupported(
                offset + length,
                field,
                "nonzero name padding",
            ));
        }
        let text = self.cp932(offset, length, field, diagnostics)?;
        Ok(JwcName {
            text,
            raw_bytes: raw.to_vec(),
            byte_offset: offset,
        })
    }

    pub fn cp932(
        &self,
        offset: usize,
        length: usize,
        field: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<String, JwcError> {
        // JWC strings must not switch to UTF-8/UTF-16 on BOM-like bytes.
        let raw = self.bytes(offset, length, field)?;
        let (decoded, had_errors) = SHIFT_JIS.decode_without_bom_handling(raw);
        if had_errors {
            diagnostics.push(Diagnostic::cp932_replaced(
                field,
                offset,
                length,
                decoded.matches(char::REPLACEMENT_CHARACTER).count(),
            ));
        }
        Ok(decoded.into_owned())
    }
}
