use serde::Serialize;

pub const CP932_DECODE_REPLACED: &str = "CP932_DECODE_REPLACED";
pub const ENTITY_LIST_TRUNCATED: &str = "ENTITY_LIST_TRUNCATED";
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum DiagnosticDetails {
    Decode(DecodeDiagnosticDetails),
    Truncation(TruncationDiagnosticDetails),
    Unverified(UnverifiedDiagnosticDetails),
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
    /// encoding is in `details.encoding` (`cp932` or `utf-16le`).
    pub(crate) fn decode_replaced(
        encoding: &str,
        field: impl Into<String>,
        byte_offset: usize,
        byte_length: usize,
        replacement_characters: usize,
    ) -> Self {
        let field = field.into();
        let label = if encoding == "cp932" {
            "CP932"
        } else {
            "UTF-16LE"
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
    pub fn truncation_details(&self) -> Option<&TruncationDiagnosticDetails> {
        match &self.details {
            DiagnosticDetails::Truncation(details) => Some(details),
            _ => None,
        }
    }
}
