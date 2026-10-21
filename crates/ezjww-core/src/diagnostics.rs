use serde::Serialize;

pub const CP932_DECODE_REPLACED: &str = "CP932_DECODE_REPLACED";
/// The ANSI strings of a JWW/JWS file were read as UTF-8 instead of CP932 (Jw_cad under Wine with a UTF-8 locale).
pub const TEXT_ENCODING_DETECTED: &str = "TEXT_ENCODING_DETECTED";
pub const ENTITY_LIST_TRUNCATED: &str = "ENTITY_LIST_TRUNCATED";
/// The version-700 image archive could not be read to its end; images read before the error are kept.
pub const IMAGE_LIST_TRUNCATED: &str = "IMAGE_LIST_TRUNCATED";
/// JWC header CSV settings differ from the reference corpus; retained as raw text.
pub const JWC_HEADER_SETTINGS_UNVERIFIED: &str = "JWC_HEADER_SETTINGS_UNVERIFIED";
/// JWC record attribute values (styles, flags, spare bytes) outside the reference corpus.
pub const JWC_ATTRIBUTE_UNVERIFIED: &str = "JWC_ATTRIBUTE_UNVERIFIED";
/// JWC line curve-marker bits did not form a verified start/member/end sequence.
pub const JWC_CURVE_MARKERS_UNVERIFIED: &str = "JWC_CURVE_MARKERS_UNVERIFIED";
/// A JWC layer group stored scale 0; scale 1 was substituted.
pub const JWC_GROUP_SCALE_DEFAULTED: &str = "JWC_GROUP_SCALE_DEFAULTED";
/// The JWC header write scale differs from the selected group's scale.
pub const JWC_WRITE_SCALE_MISMATCH: &str = "JWC_WRITE_SCALE_MISMATCH";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecodeDiagnosticDetails {
    pub encoding: String,
    pub field: String,
    pub byte_offset: usize,
    pub byte_length: usize,
    pub replacement_characters: usize,
    pub had_errors: bool,
}

/// Details of a main entity list that could not be read to its end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TruncationDiagnosticDetails {
    /// Absolute byte offset at which parsing stopped.
    pub byte_offset: usize,
    /// Entity count announced by the file (may itself be corrupt).
    pub expected_entities: usize,
    /// Entities that were parsed successfully before the error.
    pub parsed_entities: usize,
    /// The parser error that stopped the read.
    pub error: String,
}

/// Details of JWC values that lie outside the verified reference corpus but were retained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnverifiedDiagnosticDetails {
    /// Header or record field name, e.g. `line.flags` or `header.csv0`.
    pub field: String,
    /// Absolute byte offset of the first occurrence.
    pub byte_offset: usize,
    /// Number of occurrences aggregated into this diagnostic.
    pub count: usize,
    /// Up to eight distinct observed values, as text.
    pub values: Vec<String>,
}

/// Details of a version-700 image archive that could not be read to its end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImageListDiagnosticDetails {
    /// Absolute byte offset of the image record that failed.
    pub byte_offset: usize,
    /// Image count announced by the file (may itself be corrupt).
    pub expected_images: usize,
    /// Images that were read successfully before the error.
    pub parsed_images: usize,
    /// The parser error that stopped the read.
    pub error: String,
}

/// Details of a file whose ANSI strings were decoded as UTF-8 (see [`crate::reader::EncodingEvidence`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EncodingDiagnosticDetails {
    /// The encoding the strings were read with (`utf-8`).
    pub encoding: String,
    /// ANSI strings with at least one byte >= 0x80.
    pub non_ascii_strings: usize,
    /// Multi-byte UTF-8 sequences found in them.
    pub utf8_sequences: usize,
    /// Strings whose CP932 reading had replacement, C1 control or private-use characters.
    pub cp932_anomalies: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum DiagnosticDetails {
    Decode(DecodeDiagnosticDetails),
    Truncation(TruncationDiagnosticDetails),
    Unverified(UnverifiedDiagnosticDetails),
    ImageList(ImageListDiagnosticDetails),
    Encoding(EncodingDiagnosticDetails),
}

/// A structured parser diagnostic (CP932 replacement, truncated entity list, ...).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub action: String,
    pub details: DiagnosticDetails,
}

/// Historical name of [`Diagnostic`]; kept for source compatibility.
pub type DecodeDiagnostic = Diagnostic;

impl Diagnostic {
    pub(crate) fn cp932_replaced(
        field: impl Into<String>,
        byte_offset: usize,
        byte_length: usize,
        replacement_characters: usize,
    ) -> Self {
        Self::decode_replaced(
            "cp932",
            field,
            byte_offset,
            byte_length,
            replacement_characters,
        )
    }

    /// Undecodable bytes in a string were replaced with U+FFFD. The code stays
    /// `CP932_DECODE_REPLACED` for every encoding (stable contract); the actual
    /// encoding is in `details.encoding` (`cp932`, `utf-8` or `utf-16le`).
    pub(crate) fn decode_replaced(
        encoding: &str,
        field: impl Into<String>,
        byte_offset: usize,
        byte_length: usize,
        replacement_characters: usize,
    ) -> Self {
        let field = field.into();
        let label = match encoding {
            "cp932" => "CP932",
            "utf-8" => "UTF-8",
            _ => "UTF-16LE",
        };
        Self {
            code: CP932_DECODE_REPLACED.to_string(),
            severity: "warning".to_string(),
            message: format!(
                "{label} decoding replaced {replacement_characters} undecodable character sequence(s) in {field}."
            ),
            action: "normalized".to_string(),
            details: DiagnosticDetails::Decode(DecodeDiagnosticDetails {
                encoding: encoding.to_string(),
                field,
                byte_offset,
                byte_length,
                replacement_characters,
                had_errors: true,
            }),
        }
    }

    /// The ANSI strings of the file were read as UTF-8 (second pass) because
    /// the CP932 pass found them all to be well-formed UTF-8.
    pub(crate) fn text_encoding_detected(evidence: crate::reader::EncodingEvidence) -> Self {
        Self {
            code: TEXT_ENCODING_DETECTED.to_string(),
            severity: "info".to_string(),
            message: format!(
                "Strings were read as UTF-8 instead of CP932: all {} non-ASCII strings are \
                 well-formed UTF-8 ({} multi-byte sequences, {} CP932 anomalies). Jw_cad running \
                 under Wine with a UTF-8 locale writes files like this.",
                evidence.non_ascii_strings, evidence.utf8_sequences, evidence.cp932_anomalies
            ),
            action: "normalized".to_string(),
            details: DiagnosticDetails::Encoding(EncodingDiagnosticDetails {
                encoding: "utf-8".to_string(),
                non_ascii_strings: evidence.non_ascii_strings,
                utf8_sequences: evidence.utf8_sequences,
                cp932_anomalies: evidence.cp932_anomalies,
            }),
        }
    }

    pub(crate) fn entity_list_truncated(
        byte_offset: usize,
        expected_entities: usize,
        parsed_entities: usize,
        error: impl Into<String>,
    ) -> Self {
        let error = error.into();
        Self {
            code: ENTITY_LIST_TRUNCATED.to_string(),
            severity: "error".to_string(),
            message: format!(
                "JWW entity list could not be read to its end: kept {parsed_entities} of \
                 {expected_entities} announced entities, stopped at byte {byte_offset} ({error}). \
                 Block definitions after the truncated list were not read."
            ),
            action: "skipped".to_string(),
            details: DiagnosticDetails::Truncation(TruncationDiagnosticDetails {
                byte_offset,
                expected_entities,
                parsed_entities,
                error,
            }),
        }
    }

    pub(crate) fn image_list_truncated(
        byte_offset: usize,
        expected_images: usize,
        parsed_images: usize,
        error: impl Into<String>,
    ) -> Self {
        let error = error.into();
        Self {
            code: IMAGE_LIST_TRUNCATED.to_string(),
            severity: "warning".to_string(),
            message: format!(
                "JWW image archive could not be read to its end: kept {parsed_images} of \
                 {expected_images} announced images, stopped at byte {byte_offset} ({error}). \
                 Drawing entities and block definitions were read completely."
            ),
            action: "skipped".to_string(),
            details: DiagnosticDetails::ImageList(ImageListDiagnosticDetails {
                byte_offset,
                expected_images,
                parsed_images,
                error,
            }),
        }
    }

    pub(crate) fn jwc_unverified(
        code: &str,
        severity: &str,
        action: &str,
        message: String,
        details: UnverifiedDiagnosticDetails,
    ) -> Self {
        Self {
            code: code.to_string(),
            severity: severity.to_string(),
            message,
            action: action.to_string(),
            details: DiagnosticDetails::Unverified(details),
        }
    }

    /// Details when this diagnostic reports retained unverified JWC values.
    pub fn unverified_details(&self) -> Option<&UnverifiedDiagnosticDetails> {
        match &self.details {
            DiagnosticDetails::Unverified(details) => Some(details),
            _ => None,
        }
    }

    /// Decode details when this diagnostic is a CP932 replacement report.
    pub fn decode_details(&self) -> Option<&DecodeDiagnosticDetails> {
        match &self.details {
            DiagnosticDetails::Decode(details) => Some(details),
            _ => None,
        }
    }

    /// Truncation details when this diagnostic reports a truncated entity list.
    /// Details when this diagnostic reports a partially read image archive.
    pub fn image_list_details(&self) -> Option<&ImageListDiagnosticDetails> {
        match &self.details {
            DiagnosticDetails::ImageList(details) => Some(details),
            _ => None,
        }
    }

    pub fn truncation_details(&self) -> Option<&TruncationDiagnosticDetails> {
        match &self.details {
            DiagnosticDetails::Truncation(details) => Some(details),
            _ => None,
        }
    }
}
