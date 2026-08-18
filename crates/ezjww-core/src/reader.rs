use std::io::Cursor;

use encoding_rs::SHIFT_JIS;

use crate::diagnostics::DecodeDiagnostic;
use crate::error::JwwError;

pub struct Reader<'a> {
    cursor: Cursor<&'a [u8]>,
    base_offset: usize,
    decode_diagnostics: Vec<DecodeDiagnostic>,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self::with_base_offset(data, 0)
    }

    pub fn with_base_offset(data: &'a [u8], base_offset: usize) -> Self {
        Self {
            cursor: Cursor::new(data),
            base_offset,
            decode_diagnostics: Vec::new(),
        }
    }

    pub fn bytes_read(&self) -> usize {
        self.cursor.position() as usize
    }

    pub fn skip(&mut self, len: usize) -> Result<(), JwwError> {
        let pos = self.bytes_read();
        let new_pos = pos
            .checked_add(len)
            .ok_or(JwwError::UnexpectedEof("offset"))?;
        if new_pos > self.cursor.get_ref().len() {
            return Err(JwwError::UnexpectedEof("bytes"));
        }
        self.cursor.set_position(new_pos as u64);
        Ok(())
    }

    pub fn read_u8(&mut self) -> Result<u8, JwwError> {
        Ok(self.read_exact::<1>()?[0])
    }

    pub fn read_u16(&mut self) -> Result<u16, JwwError> {
        Ok(u16::from_le_bytes(self.read_exact::<2>()?))
    }

    pub fn read_u32(&mut self) -> Result<u32, JwwError> {
        Ok(u32::from_le_bytes(self.read_exact::<4>()?))
    }

    pub fn read_f64(&mut self) -> Result<f64, JwwError> {
        Ok(f64::from_le_bytes(self.read_exact::<8>()?))
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, JwwError> {
        let mut buf = vec![0_u8; len];
        self.read_exact_into(&mut buf)?;
        Ok(buf)
    }

    pub fn read_cstring(&mut self) -> Result<String, JwwError> {
        self.read_cstring_with_context("cstring")
    }

    pub fn read_cstring_with_context(&mut self, field: &str) -> Result<String, JwwError> {
        // MFC `AfxReadStringLength`: BYTE length; 0xFF escapes to WORD; the WORD
        // 0xFFFE is the Unicode marker (written as FF FE FF) after which the length
        // is encoded again and the payload is UTF-16LE. Jw_cad's Unicode builds
        // (JWW version 700 files) write every CString this way.
        let mut unicode = false;
        let len_byte = self.read_u8()?;
        let len = if len_byte < 0xFF {
            len_byte as usize
        } else {
            let mut word_len = self.read_u16()?;
            if word_len == 0xFFFE {
                unicode = true;
                let inner = self.read_u8()?;
                if inner < 0xFF {
                    inner as usize
                } else {
                    word_len = self.read_u16()?;
                    if word_len < 0xFFFF {
                        word_len as usize
                    } else {
                        self.read_u32()? as usize
                    }
                }
            } else if word_len < 0xFFFF {
                word_len as usize
            } else {
                self.read_u32()? as usize
            }
        };

        if len == 0 {
            return Ok(String::new());
        }

        let byte_offset = self.base_offset.saturating_add(self.bytes_read());
        if unicode {
            let byte_len = len
                .checked_mul(2)
                .ok_or(JwwError::UnexpectedEof("cstring length"))?;
            let bytes = self.read_bytes(byte_len)?;
            let (decoded, had_errors) = decode_utf16le(&bytes);
            if had_errors {
                let replacement_characters = decoded.matches(char::REPLACEMENT_CHARACTER).count();
                self.decode_diagnostics
                    .push(DecodeDiagnostic::decode_replaced(
                        "utf-16le",
                        field,
                        byte_offset,
                        byte_len,
                        replacement_characters,
                    ));
            }
            return Ok(decoded.trim_end_matches('\0').to_string());
        }

        let bytes = self.read_bytes(len)?;
        let (decoded, _, had_errors) = SHIFT_JIS.decode(&bytes);
        if had_errors {
            let replacement_characters = decoded.matches(char::REPLACEMENT_CHARACTER).count();
            self.decode_diagnostics
                .push(DecodeDiagnostic::cp932_replaced(
                    field,
                    byte_offset,
                    len,
                    replacement_characters,
                ));
        }
        Ok(decoded.trim_end_matches('\0').to_string())
    }

    pub fn decode_diagnostic_count(&self) -> usize {
        self.decode_diagnostics.len()
    }

    pub fn truncate_decode_diagnostics(&mut self, len: usize) {
        self.decode_diagnostics.truncate(len);
    }

    pub fn into_decode_diagnostics(self) -> Vec<DecodeDiagnostic> {
        self.decode_diagnostics
    }

    fn read_exact<const N: usize>(&mut self) -> Result<[u8; N], JwwError> {
        let mut buf = [0_u8; N];
        self.read_exact_into(&mut buf)?;
        Ok(buf)
    }

    fn read_exact_into(&mut self, buf: &mut [u8]) -> Result<(), JwwError> {
        let pos = self.bytes_read();
        let end = pos
            .checked_add(buf.len())
            .ok_or(JwwError::UnexpectedEof("offset"))?;
        let src = self.cursor.get_ref();
        if end > src.len() {
            return Err(JwwError::UnexpectedEof("bytes"));
        }
        buf.copy_from_slice(&src[pos..end]);
        self.cursor.set_position(end as u64);
        Ok(())
    }
}

/// Decode UTF-16LE code units, replacing unpaired surrogates with U+FFFD.
fn decode_utf16le(bytes: &[u8]) -> (String, bool) {
    let units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let mut had_errors = !bytes.len().is_multiple_of(2);
    let decoded = char::decode_utf16(units.iter().copied())
        .map(|result| {
            result.unwrap_or_else(|_| {
                had_errors = true;
                char::REPLACEMENT_CHARACTER
            })
        })
        .collect::<String>();
    (decoded, had_errors)
}

#[cfg(test)]
mod tests {
    use super::Reader;

    #[test]
    fn read_numeric_values() {
        let data = [
            0x01, // u8
            0x02, 0x00, // u16
            0x58, 0x02, 0x00, 0x00, // u32 (600)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f, // f64 (1.0)
        ];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.read_u8().unwrap(), 1);
        assert_eq!(reader.read_u16().unwrap(), 2);
        assert_eq!(reader.read_u32().unwrap(), 600);
        assert_eq!(reader.read_f64().unwrap(), 1.0);
    }

    #[test]
    fn read_cstring_short() {
        let data = [4, b't', b'e', b's', b't'];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.read_cstring().unwrap(), "test");
    }

    #[test]
    fn read_cstring_empty() {
        let data = [0];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.read_cstring().unwrap(), "");
    }

    #[test]
    fn read_cstring_unicode_marker_decodes_utf16le() {
        // MFC AfxWriteStringLength(bUnicode=TRUE): FF FE FF, then BYTE length, then UTF-16LE.
        let mut data = vec![0xFF, 0xFE, 0xFF, 3];
        for unit in "図面A".encode_utf16() {
            data.extend_from_slice(&unit.to_le_bytes());
        }
        let mut reader = Reader::new(&data);
        assert_eq!(reader.read_cstring().unwrap(), "図面A");
        assert_eq!(reader.bytes_read(), data.len());
        assert!(reader.into_decode_diagnostics().is_empty());
    }

    #[test]
    fn read_cstring_unicode_marker_with_word_length() {
        let text = "x".repeat(300);
        let mut data = vec![0xFF, 0xFE, 0xFF, 0xFF];
        data.extend_from_slice(&300u16.to_le_bytes());
        for unit in text.encode_utf16() {
            data.extend_from_slice(&unit.to_le_bytes());
        }
        let mut reader = Reader::new(&data);
        assert_eq!(reader.read_cstring().unwrap(), text);
    }

    #[test]
    fn read_cstring_unicode_unpaired_surrogate_is_reported() {
        let data = [0xFF, 0xFE, 0xFF, 1, 0x00, 0xD8]; // lone high surrogate
        let mut reader = Reader::with_base_offset(&data, 10);
        assert_eq!(
            reader.read_cstring_with_context("header.memo").unwrap(),
            char::REPLACEMENT_CHARACTER.to_string()
        );
        let diagnostics = reader.into_decode_diagnostics();
        assert_eq!(diagnostics.len(), 1);
        let details = diagnostics[0].decode_details().expect("decode details");
        assert_eq!(details.encoding, "utf-16le");
        assert_eq!(details.byte_offset, 14);
        assert_eq!(details.byte_length, 2);
    }

    #[test]
    fn read_cstring_ansi_word_length_still_works() {
        let text = "y".repeat(300);
        let mut data = vec![0xFF];
        data.extend_from_slice(&300u16.to_le_bytes());
        data.extend_from_slice(text.as_bytes());
        let mut reader = Reader::new(&data);
        assert_eq!(reader.read_cstring().unwrap(), text);
    }

    #[test]
    fn read_cstring_reports_cp932_replacement_with_absolute_offset() {
        let data = [1, 0x81];
        let mut reader = Reader::with_base_offset(&data, 100);

        assert_eq!(
            reader
                .read_cstring_with_context("entity.text.content")
                .unwrap(),
            char::REPLACEMENT_CHARACTER.to_string()
        );

        let diagnostics = reader.into_decode_diagnostics();
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, "CP932_DECODE_REPLACED");
        let details = diagnostic.decode_details().expect("decode details");
        assert_eq!(details.field, "entity.text.content");
        assert_eq!(details.byte_offset, 101);
        assert_eq!(details.byte_length, 1);
        assert_eq!(details.replacement_characters, 1);
        assert!(details.had_errors);
    }
}
