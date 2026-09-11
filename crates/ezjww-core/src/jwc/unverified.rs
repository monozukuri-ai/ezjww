//! Aggregation of values that lie outside the verified reference corpus.
//!
//! Real DOS-era files carry settings, style bits and spare bytes that the
//! generated corpus never produced. They are retained and reported once per
//! field instead of rejecting the document; structural inconsistencies still fail.

use std::collections::BTreeMap;

use crate::diagnostics::{
    Diagnostic, UnverifiedDiagnosticDetails, JWC_ATTRIBUTE_UNVERIFIED,
    JWC_CURVE_MARKERS_UNVERIFIED, JWC_GROUP_SCALE_DEFAULTED, JWC_HEADER_SETTINGS_UNVERIFIED,
    JWC_WRITE_SCALE_MISMATCH,
};

const MAX_VALUES: usize = 8;

/// Internal aggregation key for fixed-width name slots that carry no NUL terminator.
/// It is not a public issue code: like every key outside the dedicated list below it is emitted as `JWC_ATTRIBUTE_UNVERIFIED`.
pub(super) const UNTERMINATED_NAME_SLOT: &str = "jwc.name_slot.unterminated";

struct Entry {
    first_offset: usize,
    count: usize,
    values: Vec<String>,
}

#[derive(Default)]
pub(super) struct UnverifiedCollector {
    entries: BTreeMap<(&'static str, String), Entry>,
    order: Vec<(&'static str, String)>,
}

impl UnverifiedCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn note(
        &mut self,
        code: &'static str,
        field: &str,
        offset: usize,
        value: impl Into<String>,
    ) {
        let key = (code, field.to_string());
        let value = value.into();
        match self.entries.get_mut(&key) {
            Some(entry) => {
                entry.count += 1;
                if entry.values.len() < MAX_VALUES && !entry.values.contains(&value) {
                    entry.values.push(value);
                }
            }
            None => {
                self.entries.insert(
                    key.clone(),
                    Entry {
                        first_offset: offset,
                        count: 1,
                        values: vec![value],
                    },
                );
                self.order.push(key);
            }
        }
    }

    pub fn into_diagnostics(mut self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::with_capacity(self.order.len());
        for key in self.order {
            let entry = self.entries.remove(&key).expect("entry recorded with key");
            let (code, field) = key;
            let examples = entry.values.join(", ");
            let (severity, action, message) = match code {
                JWC_HEADER_SETTINGS_UNVERIFIED => (
                    "info",
                    "retained",
                    format!(
                        "{} {field} setting(s) differ from the reference corpus and were retained as raw text (first at byte {}; e.g. {examples}).",
                        entry.count, entry.first_offset
                    ),
                ),
                JWC_CURVE_MARKERS_UNVERIFIED => (
                    "warning",
                    "retained",
                    format!(
                        "{} line record(s) carried curve-marker bits outside the verified start/member/end sequence and were kept as ungrouped lines (first at byte {}; markers {examples}).",
                        entry.count, entry.first_offset
                    ),
                ),
                JWC_GROUP_SCALE_DEFAULTED => (
                    "warning",
                    "normalized",
                    format!(
                        "{} layer group(s) store scale 0; scale 1 was substituted (first at byte {}; groups {examples}).",
                        entry.count, entry.first_offset
                    ),
                ),
                JWC_WRITE_SCALE_MISMATCH => (
                    "warning",
                    "retained",
                    format!(
                        "Header write scale differs from the selected layer group's scale at byte {} ({examples}); per-group scales were used.",
                        entry.first_offset
                    ),
                ),
                UNTERMINATED_NAME_SLOT => (
                    "warning",
                    "retained",
                    format!(
                        "{} {field} slot(s) carry no NUL terminator; the whole fixed-width slot was accepted as the name (first at byte {}; e.g. {examples}).",
                        entry.count, entry.first_offset
                    ),
                ),
                _ => (
                    "warning",
                    "retained",
                    format!(
                        "{} {field} value(s) outside the verified reference corpus were retained (first at byte {}; e.g. {examples}).",
                        entry.count, entry.first_offset
                    ),
                ),
            };
            diagnostics.push(Diagnostic::jwc_unverified(
                if matches!(
                    code,
                    JWC_HEADER_SETTINGS_UNVERIFIED
                        | JWC_CURVE_MARKERS_UNVERIFIED
                        | JWC_GROUP_SCALE_DEFAULTED
                        | JWC_WRITE_SCALE_MISMATCH
                ) {
                    code
                } else {
                    JWC_ATTRIBUTE_UNVERIFIED
                },
                severity,
                action,
                message,
                UnverifiedDiagnosticDetails {
                    field,
                    byte_offset: entry.first_offset,
                    count: entry.count,
                    values: entry.values,
                },
            ));
        }
        diagnostics
    }
}
