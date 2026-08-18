use serde::Serialize;

pub const CP932_DECODE_REPLACED: &str = "CP932_DECODE_REPLACED";
pub const ENTITY_LIST_TRUNCATED: &str = "ENTITY_LIST_TRUNCATED";

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum DiagnosticDetails {
    Decode(DecodeDiagnosticDetails),
    Truncation(TruncationDiagnosticDetails),
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

    /// Decode details when this diagnostic is a CP932 replacement report.
    pub fn decode_details(&self) -> Option<&DecodeDiagnosticDetails> {
        match &self.details {
            DiagnosticDetails::Decode(details) => Some(details),
            DiagnosticDetails::Truncation(_) => None,
        }
    }

    /// Truncation details when this diagnostic reports a truncated entity list.
    pub fn truncation_details(&self) -> Option<&TruncationDiagnosticDetails> {
        match &self.details {
            DiagnosticDetails::Truncation(details) => Some(details),
            DiagnosticDetails::Decode(_) => None,
        }
    }
}
