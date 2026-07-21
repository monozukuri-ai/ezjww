use serde::Serialize;

pub const CP932_DECODE_REPLACED: &str = "CP932_DECODE_REPLACED";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecodeDiagnosticDetails {
    pub encoding: String,
    pub field: String,
    pub byte_offset: usize,
    pub byte_length: usize,
    pub replacement_characters: usize,
    pub had_errors: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecodeDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub action: String,
    pub details: DecodeDiagnosticDetails,
}

impl DecodeDiagnostic {
    pub(crate) fn cp932_replaced(
        field: impl Into<String>,
        byte_offset: usize,
        byte_length: usize,
        replacement_characters: usize,
    ) -> Self {
        let field = field.into();
        Self {
            code: CP932_DECODE_REPLACED.to_string(),
            severity: "warning".to_string(),
            message: format!(
                "CP932 decoding replaced {replacement_characters} undecodable character sequence(s) in {field}."
            ),
            action: "normalized".to_string(),
            details: DecodeDiagnosticDetails {
                encoding: "cp932".to_string(),
                field,
                byte_offset,
                byte_length,
                replacement_characters,
                had_errors: true,
            },
        }
    }
}
