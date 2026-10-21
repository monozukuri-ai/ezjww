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
    let cp932 = TextContext::new(TextEncoding::Cp932);
    let mut body = crate::parser::parse_symbol_body(data, HEADER_SIZE, header.version, &cp932)
        .map_err(|(byte_offset, source)| JwsError::Archive {
            byte_offset,
            source,
        })?;
    let mut text_encoding = TextEncoding::Cp932;
    if cp932.prefers_utf8() {
        // Same second pass as the JWW reader: the file's strings are UTF-8.
        let utf8 = TextContext::new(TextEncoding::Utf8);
        if let Ok(mut again) =
            crate::parser::parse_symbol_body(data, HEADER_SIZE, header.version, &utf8)
        {
            again.diagnostics.insert(
                0,
                DecodeDiagnostic::text_encoding_detected(cp932.evidence()),
            );
            body = again;
            text_encoding = TextEncoding::Utf8;
        }
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
pub fn read_jws_document_from_file(path: impl AsRef<Path>) -> Result<JwsDocument, JwsError> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_JWS_BYTES as u64 {
        return Err(JwsError::invalid(0, "file exceeds 256 MiB limit"));
    }
    let mut data = Vec::new();
    file.take(MAX_JWS_BYTES as u64 + 1).read_to_end(&mut data)?;
    parse_jws_document(&data)
}
