use super::{header::HEADER_SIZE, parse_jws_header, JwsDocument, JwsError};
use crate::diagnostics::DecodeDiagnostic;
use crate::reader::{TextContext, TextEncoding};
use std::{fs::File, io::Read, path::Path};

// Bound file allocation independently of counts stored by an untrusted input.
pub const MAX_JWS_BYTES: usize = 256 * 1024 * 1024;
pub fn parse_jws_document(data: &[u8]) -> Result<JwsDocument, JwsError> {
    if data.len() > MAX_JWS_BYTES {
        return Err(JwsError::invalid(0, "file exceeds 256 MiB limit"));
    }
    let header = parse_jws_header(data)?;
    // Jw_cad 6 writes a header version of 420 while its records carry the MFC
    // class schema 600 of the running program. The schema decides the record
    // layout (and the strict class-tag check), so it wins over the header.
    let mut version = header.version;
    let mut schema_note = None;
    if let Some(schema) = peek_entity_schema(data, HEADER_SIZE) {
        let layout = u32::from(schema);
        if matches!(layout, 351 | 420 | 600 | 700)
            && !crate::parser::symbol_schema_matches(header.version, schema)
        {
            schema_note = Some(DecodeDiagnostic::jws_schema_version_used(
                header.version,
                layout,
            ));
            version = layout;
        }
    }
    let cp932 = TextContext::new(TextEncoding::Cp932);
    let mut body = crate::parser::parse_symbol_body(data, HEADER_SIZE, version, &cp932).map_err(
        |(byte_offset, source)| JwsError::Archive {
            byte_offset,
            source,
        },
    )?;
    let mut text_encoding = TextEncoding::Cp932;
    if cp932.prefers_utf8() {
        // Same second pass as the JWW reader: the file's strings are UTF-8.
        let utf8 = TextContext::new(TextEncoding::Utf8);
        if let Ok(mut again) = crate::parser::parse_symbol_body(data, HEADER_SIZE, version, &utf8) {
            again.diagnostics.insert(
                0,
                DecodeDiagnostic::text_encoding_detected(cp932.evidence()),
            );
            body = again;
            text_encoding = TextEncoding::Utf8;
        }
    }
    if let Some(note) = schema_note {
        body.diagnostics.insert(0, note);
    }
    Ok(JwsDocument {
        header,
        entities: body.entities,
        block_defs: body.block_defs,
        images: body.images,
        diagnostics: body.diagnostics,
        text_encoding,
    })
}
/// The MFC class schema of the first record: the `CObList` count (a WORD, or
/// `0xFFFF` followed by a DWORD), then the new-class tag `0xFFFF`, the schema,
/// the class-name length and a `CData…` name. `None` when the body does not
/// start that way; the parser then reports the real problem.
fn peek_entity_schema(data: &[u8], offset: usize) -> Option<u16> {
    let word = |at: usize| Some(u16::from_le_bytes([*data.get(at)?, *data.get(at + 1)?]));
    let mut at = offset;
    if word(at)? == 0xFFFF {
        at += 4;
    }
    at += 2;
    if word(at)? != 0xFFFF {
        return None;
    }
    let schema = word(at + 2)?;
    let name_len = usize::from(word(at + 4)?);
    let name = data.get(at + 6..at.checked_add(6)?.checked_add(name_len)?)?;
    name.starts_with(b"CData").then_some(schema)
}

pub fn read_jws_document_from_file(path: impl AsRef<Path>) -> Result<JwsDocument, JwsError> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_JWS_BYTES as u64 {
        return Err(JwsError::invalid(0, "file exceeds 256 MiB limit"));
    }
    let mut data = Vec::new();
    file.take(MAX_JWS_BYTES as u64 + 1).read_to_end(&mut data)?;
    parse_jws_document(&data)
}
