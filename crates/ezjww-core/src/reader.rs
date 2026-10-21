use std::cell::RefCell;
use std::io::Cursor;

use encoding_rs::SHIFT_JIS;
use serde::Serialize;

use crate::diagnostics::DecodeDiagnostic;
use crate::error::JwwError;

/// How the ANSI (`BYTE length` + bytes) CStrings of a file are decoded.
///
/// Jw_cad writes them in the Windows ANSI code page, which is CP932 on a
/// Japanese Windows. Jw_cad running under Wine with a UTF-8 locale writes the
/// same strings as UTF-8; the reader detects that per file (see
/// [`EncodingEvidence::prefers_utf8`]). Unicode CStrings (MFC `FF FE FF`
/// marker, UTF-16LE) are not affected by this choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum TextEncoding {
    #[default]
    #[serde(rename = "cp932")]
    Cp932,
    #[serde(rename = "utf-8")]
    Utf8,
}

impl TextEncoding {
    pub fn as_str(self) -> &'static str {
        match self {
            TextEncoding::Cp932 => "cp932",
            TextEncoding::Utf8 => "utf-8",
        }
    }
}

/// What the ANSI strings of one file look like, collected while they are
/// decoded as CP932. Only strings with at least one byte >= 0x80 count; the
/// decision is made once per file after the whole file was read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct EncodingEvidence {
    /// ANSI strings with at least one byte >= 0x80.
    pub non_ascii_strings: usize,
    /// Of those, strings that are well-formed UTF-8 (a trailing incomplete
    /// sequence, as left by a fixed-size buffer, still counts as well-formed).
    pub utf8_valid_strings: usize,
    /// Multi-byte UTF-8 sequences in the well-formed strings.
    pub utf8_sequences: usize,
    /// Strings whose CP932 decoding needed a replacement character or produced
    /// a C1 control (U+0080..=U+009F) or a private-use character: what UTF-8
    /// continuation bytes turn into when they are read as CP932.
    pub cp932_anomalies: usize,
}

impl EncodingEvidence {
    /// True when every non-ASCII string is well-formed UTF-8 and the file
    /// contains enough multi-byte sequences (or a CP932 anomaly) that a CP932
    /// file would be very unlikely to look like this by chance.
    ///
    /// A CP932 two-byte character is valid UTF-8 only in narrow cases (a JIS
    /// level-2 kanji with a trail byte >= 0x80 followed by a lead byte, or a
    /// half-width katakana pair such as `ﾒｰ` = `D2 B0`), so three independent
    /// sequences, or one sequence plus an anomaly, keep the misdetection rate
    /// negligible while a short UTF-8 string like 女子 (two sequences, one
    /// anomaly) is still recognised.
    pub fn prefers_utf8(&self) -> bool {
        self.non_ascii_strings > 0
            && self.utf8_valid_strings == self.non_ascii_strings
            && (self.utf8_sequences >= 3 || self.cp932_anomalies > 0)
    }

    fn observe(&mut self, bytes: &[u8], cp932_decoded: &str, cp932_had_errors: bool) {
        if !bytes.iter().any(|byte| *byte >= 0x80) {
            return;
        }
        self.non_ascii_strings += 1;
        if let Some(valid_len) = utf8_shape(bytes) {
            self.utf8_valid_strings += 1;
            self.utf8_sequences += bytes[..valid_len]
                .iter()
                .filter(|byte| (0xC2..=0xF4).contains(*byte))
                .count();
        }
        if cp932_had_errors
            || cp932_decoded.chars().any(|c| {
                ('\u{80}'..='\u{9f}').contains(&c)
                    || ('\u{e000}'..='\u{f8ff}').contains(&c)
                    || c == char::REPLACEMENT_CHARACTER
            })
        {
            self.cp932_anomalies += 1;
        }
    }
}

/// Length of the well-formed UTF-8 prefix when `bytes` is UTF-8 apart from an
/// incomplete sequence at the very end (`Some(bytes.len())` when complete);
/// `None` when the bytes are not UTF-8.
fn utf8_shape(bytes: &[u8]) -> Option<usize> {
    match std::str::from_utf8(bytes) {
        Ok(_) => Some(bytes.len()),
        Err(error) if error.error_len().is_none() && error.valid_up_to() > 0 => {
            Some(error.valid_up_to())
        }
        Err(_) => None,
    }
}

/// The text encoding a parse runs with, plus the evidence it collects.
/// Shared by every [`Reader`] of one parse (header, entity list, blocks, images).
#[derive(Debug, Default)]
pub struct TextContext {
    encoding: TextEncoding,
    evidence: RefCell<EncodingEvidence>,
}

impl TextContext {
    pub fn new(encoding: TextEncoding) -> Self {
        Self {
            encoding,
            evidence: RefCell::new(EncodingEvidence::default()),
        }
    }

    pub fn encoding(&self) -> TextEncoding {
        self.encoding
    }

    pub fn evidence(&self) -> EncodingEvidence {
        *self.evidence.borrow()
    }

    /// Only meaningful after a CP932 pass: whether the file should be read again as UTF-8.
    pub fn prefers_utf8(&self) -> bool {
        self.encoding == TextEncoding::Cp932 && self.evidence.borrow().prefers_utf8()
    }
}

pub struct Reader<'a> {
    cursor: Cursor<&'a [u8]>,
    base_offset: usize,
    finite_numbers_only: bool,
    decode_diagnostics: Vec<DecodeDiagnostic>,
    text: Option<&'a TextContext>,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self::with_base_offset(data, 0)
    }

    pub fn with_base_offset(data: &'a [u8], base_offset: usize) -> Self {
        Self {
            cursor: Cursor::new(data),
            base_offset,
            finite_numbers_only: false,
            decode_diagnostics: Vec::new(),
            text: None,
        }
    }

    /// A reader whose ANSI strings follow `text` (encoding and evidence shared
    /// with the other readers of the same parse).
    pub fn with_text(data: &'a [u8], base_offset: usize, text: &'a TextContext) -> Self {
        Self {
            text: Some(text),
            ..Self::with_base_offset(data, base_offset)
        }
    }

    pub fn text_encoding(&self) -> TextEncoding {
        self.text.map(TextContext::encoding).unwrap_or_default()
    }

    pub fn bytes_read(&self) -> usize {
        self.cursor.position() as usize
    }

    pub(crate) fn remaining(&self) -> usize {
        self.cursor
            .get_ref()
            .len()
            .saturating_sub(self.bytes_read())
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

    pub(crate) fn require_finite_numbers(&mut self) {
        self.finite_numbers_only = true;
    }

    pub fn read_f64(&mut self) -> Result<f64, JwwError> {
        let value = f64::from_le_bytes(self.read_exact::<8>()?);
        if self.finite_numbers_only && !value.is_finite() {
            return Err(JwwError::InvalidArchive("non-finite entity value"));
        }
        Ok(value)
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, JwwError> {
        if len > self.remaining() {
            return Err(JwwError::UnexpectedEof("bytes"));
        }
        let mut buf = vec![0_u8; len];
        self.read_exact_into(&mut buf)?;
        Ok(buf)
    }

    pub fn read_cstring(&mut self) -> Result<String, JwwError> {
        self.read_cstring_with_context("cstring")
    }

    /// A string the user typed or named (text content, layer/group/block names,
    /// memo, image names): follows the file's text encoding and feeds the
    /// evidence that decides it.
    pub fn read_cstring_with_context(&mut self, field: &str) -> Result<String, JwwError> {
        self.read_cstring_impl(field, false)
    }

    /// A string Jw_cad copies from its own string table (font names, SXF line
    /// type and extended color names). Under Wine those stay CP932 while the
    /// user's strings are UTF-8, so they neither feed the evidence nor follow
    /// the file's encoding blindly: in a UTF-8 file they are read as UTF-8 only
    /// when their bytes are well-formed UTF-8, otherwise as CP932.
    pub fn read_system_cstring_with_context(&mut self, field: &str) -> Result<String, JwwError> {
        self.read_cstring_impl(field, true)
    }

    fn read_cstring_impl(&mut self, field: &str, system: bool) -> Result<String, JwwError> {
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
        if self.text_encoding() == TextEncoding::Utf8
            && (!system || utf8_shape(&bytes) == Some(bytes.len()))
        {
            return Ok(self.decode_utf8(&bytes, field, byte_offset));
        }
        let (decoded, _, had_errors) = SHIFT_JIS.decode(&bytes);
        if let Some(text) = self.text.filter(|_| !system) {
            text.evidence
                .borrow_mut()
                .observe(&bytes, &decoded, had_errors);
        }
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

    /// UTF-8 pass. A trailing incomplete sequence (a name cut by a fixed-size
    /// buffer when it was written) is dropped; anything else undecodable is
    /// replaced with U+FFFD. Both are reported like a CP932 replacement, with
    /// `details.encoding` = `utf-8`.
    fn decode_utf8(&mut self, bytes: &[u8], field: &str, byte_offset: usize) -> String {
        let (decoded, replaced) = match std::str::from_utf8(bytes) {
            Ok(text) => (text.to_string(), 0),
            Err(error) if error.error_len().is_none() => {
                let valid = std::str::from_utf8(&bytes[..error.valid_up_to()])
                    .map(str::to_string)
                    .unwrap_or_default();
                (valid, 1)
            }
            Err(_) => {
                let lossy = String::from_utf8_lossy(bytes).into_owned();
                let replaced = lossy.matches(char::REPLACEMENT_CHARACTER).count();
                (lossy, replaced)
            }
        };
        if replaced > 0 {
            self.decode_diagnostics
                .push(DecodeDiagnostic::decode_replaced(
                    "utf-8",
                    field,
                    byte_offset,
                    bytes.len(),
                    replaced,
                ));
        }
        decoded.trim_end_matches('\0').to_string()
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
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
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
    use super::{EncodingEvidence, Reader, TextContext, TextEncoding};

    fn ansi(text: &[u8]) -> Vec<u8> {
        let mut data = vec![text.len() as u8];
        data.extend_from_slice(text);
        data
    }

    fn evidence_of(strings: &[&[u8]]) -> EncodingEvidence {
        let context = TextContext::new(TextEncoding::Cp932);
        for bytes in strings {
            let data = ansi(bytes);
            let mut reader = Reader::with_text(&data, 0, &context);
            reader.read_cstring().unwrap();
        }
        context.evidence()
    }

    #[test]
    fn utf8_evidence_recognises_a_wine_file() {
        // 女子便所 / 縮　　尺 as UTF-8 bytes: every string is well-formed UTF-8
        // and the CP932 reading ends in U+0080 (an anomaly).
        let evidence = evidence_of(&["女子便所".as_bytes(), "縮\u{3000}\u{3000}尺".as_bytes()]);
        assert_eq!(evidence.non_ascii_strings, 2);
        assert_eq!(evidence.utf8_valid_strings, 2);
        assert_eq!(evidence.utf8_sequences, 8);
        assert!(evidence.cp932_anomalies >= 1);
        assert!(evidence.prefers_utf8());
    }

    #[test]
    fn utf8_evidence_accepts_a_name_cut_inside_a_sequence() {
        let mut cut = "Aダ0_積".as_bytes().to_vec();
        cut.truncate(cut.len() - 1); // the last kanji lost its final byte
        let evidence = evidence_of(&[&cut, "女子".as_bytes()]);
        assert_eq!(evidence.utf8_valid_strings, 2);
        assert!(evidence.prefers_utf8());
    }

    #[test]
    fn cp932_evidence_stays_cp932() {
        let (kanji, _, _) = encoding_rs::SHIFT_JIS.encode("図面ゲート 営業ｾﾝﾀｰ");
        let (katakana, _, _) = encoding_rs::SHIFT_JIS.encode("ﾒｰﾄﾙ");
        let evidence = evidence_of(&[&kanji, &katakana, b"ASCII only"]);
        assert_eq!(evidence.non_ascii_strings, 2);
        assert_eq!(evidence.utf8_valid_strings, 0);
        assert!(!evidence.prefers_utf8());
    }

    #[test]
    fn a_single_short_coincidence_is_not_enough_for_utf8() {
        // `ﾒｰ` in CP932 is D2 B0, which happens to be well-formed UTF-8 (one sequence).
        let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode("ﾒｰ");
        assert!(std::str::from_utf8(&bytes).is_ok());
        let evidence = evidence_of(&[&bytes]);
        assert_eq!(evidence.utf8_valid_strings, 1);
        assert_eq!(evidence.utf8_sequences, 1);
        assert_eq!(evidence.cp932_anomalies, 0);
        assert!(!evidence.prefers_utf8());
    }

    #[test]
    fn ascii_only_files_have_no_evidence() {
        let evidence = evidence_of(&[b"layer 1", b""]);
        assert_eq!(evidence, EncodingEvidence::default());
        assert!(!evidence.prefers_utf8());
    }

    #[test]
    fn system_strings_do_not_feed_the_evidence_and_fall_back_to_cp932() {
        // Jw_cad under Wine: the font name comes from its CP932 string table
        // while the typed text is UTF-8.
        let (font, _, _) = encoding_rs::SHIFT_JIS.encode("ＭＳ ゴシック");
        let context = TextContext::new(TextEncoding::Cp932);
        let data = ansi(&font);
        let mut reader = Reader::with_text(&data, 0, &context);
        assert_eq!(
            reader
                .read_system_cstring_with_context("entity.text.font_name")
                .unwrap(),
            "ＭＳ ゴシック"
        );
        let data = ansi("女子便所".as_bytes());
        let mut reader = Reader::with_text(&data, 0, &context);
        reader.read_cstring().unwrap();
        let evidence = context.evidence();
        assert_eq!(evidence.non_ascii_strings, 1);
        assert!(context.prefers_utf8());

        let utf8 = TextContext::new(TextEncoding::Utf8);
        let data = ansi(&font);
        let mut reader = Reader::with_text(&data, 0, &utf8);
        assert_eq!(
            reader
                .read_system_cstring_with_context("entity.text.font_name")
                .unwrap(),
            "ＭＳ ゴシック"
        );
        assert!(reader.into_decode_diagnostics().is_empty());
        let data = ansi("ＭＳ ゴシック".as_bytes());
        let mut reader = Reader::with_text(&data, 0, &utf8);
        assert_eq!(
            reader
                .read_system_cstring_with_context("entity.text.font_name")
                .unwrap(),
            "ＭＳ ゴシック"
        );
    }

    #[test]
    fn utf8_pass_decodes_and_drops_a_cut_tail() {
        let context = TextContext::new(TextEncoding::Utf8);
        let data = ansi("女子便所".as_bytes());
        let mut reader = Reader::with_text(&data, 0, &context);
        assert_eq!(reader.read_cstring().unwrap(), "女子便所");
        assert!(reader.into_decode_diagnostics().is_empty());

        let mut cut = "Aダ0_積".as_bytes().to_vec();
        cut.truncate(cut.len() - 1);
        let data = ansi(&cut);
        let mut reader = Reader::with_text(&data, 5, &context);
        assert_eq!(
            reader
                .read_cstring_with_context("header.layer[0].name")
                .unwrap(),
            "Aダ0_"
        );
        let diagnostics = reader.into_decode_diagnostics();
        assert_eq!(diagnostics.len(), 1);
        let details = diagnostics[0].decode_details().expect("decode details");
        assert_eq!(details.encoding, "utf-8");
        assert_eq!(details.field, "header.layer[0].name");
        assert_eq!(details.byte_offset, 6);
        assert_eq!(details.replacement_characters, 1);

        // Bytes that are not UTF-8 at all are replaced, not dropped.
        let data = ansi(&[b'a', 0x81, 0x40, b'b']);
        let mut reader = Reader::with_text(&data, 0, &context);
        assert_eq!(reader.read_cstring().unwrap(), "a\u{fffd}@b");
        assert_eq!(reader.into_decode_diagnostics().len(), 1);
        // Evidence is only collected by the CP932 pass.
        assert_eq!(context.evidence(), EncodingEvidence::default());
    }

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
