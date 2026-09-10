"""Stable audit issue contracts and the public issue-code catalog."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal

IssueSeverity = Literal["info", "warning", "error"]

CP932_DECODE_REPLACED = "CP932_DECODE_REPLACED"
ENTITY_LIST_TRUNCATED = "ENTITY_LIST_TRUNCATED"
UNRESOLVED_BLOCK_REFERENCES = "UNRESOLVED_BLOCK_REFERENCES"
UNSUPPORTED_DXF_ENTITIES = "UNSUPPORTED_DXF_ENTITIES"
JWC_HEADER_SETTINGS_UNVERIFIED = "JWC_HEADER_SETTINGS_UNVERIFIED"
JWC_ATTRIBUTE_UNVERIFIED = "JWC_ATTRIBUTE_UNVERIFIED"
JWC_CURVE_MARKERS_UNVERIFIED = "JWC_CURVE_MARKERS_UNVERIFIED"
JWC_GROUP_SCALE_DEFAULTED = "JWC_GROUP_SCALE_DEFAULTED"
JWC_WRITE_SCALE_MISMATCH = "JWC_WRITE_SCALE_MISMATCH"


@dataclass(frozen=True)
class IssueCode:
    """Catalog metadata for one stable ``audit()`` issue identifier."""

    severity: IssueSeverity
    action: str | None
    area: str
    condition: str


ISSUE_CODES: dict[str, IssueCode] = {
    CP932_DECODE_REPLACED: IssueCode(
        "warning",
        "normalized",
        "JWW parser",
        "One or more undecodable byte sequences (CP932, or UTF-16LE for Unicode "
        "strings) were replaced.",
    ),
    ENTITY_LIST_TRUNCATED: IssueCode(
        "error",
        "skipped",
        "JWW parser",
        "The main entity list could not be read to its end; entities parsed before "
        "the error were kept and block definitions were not read.",
    ),
    UNRESOLVED_BLOCK_REFERENCES: IssueCode(
        "warning",
        None,
        "JWW validation",
        "One or more block references could not be resolved.",
    ),
    UNSUPPORTED_DXF_ENTITIES: IssueCode(
        "warning",
        "skipped",
        "DXF conversion",
        "One or more parsed JWW entity kinds are unsupported by DXF conversion.",
    ),
    JWC_HEADER_SETTINGS_UNVERIFIED: IssueCode(
        "info",
        "retained",
        "JWC parser",
        "Header CSV settings differ from the reference corpus; they were retained "
        "as raw text and do not affect geometry.",
    ),
    JWC_ATTRIBUTE_UNVERIFIED: IssueCode(
        "warning",
        "retained",
        "JWC parser",
        "Record attribute values (style bits, flag bits, spare bytes, layer state "
        "bits, text presets, unterminated layer/group name slots) lie outside the "
        "reference corpus and were retained.",
    ),
    JWC_CURVE_MARKERS_UNVERIFIED: IssueCode(
        "warning",
        "retained",
        "JWC parser",
        "Line curve-marker bits did not form a verified start/member/end sequence; "
        "the lines were kept ungrouped.",
    ),
    JWC_GROUP_SCALE_DEFAULTED: IssueCode(
        "warning",
        "normalized",
        "JWC parser",
        "A layer group stores scale 0 in the u16-scale header profile; scale 1 was "
        "substituted.",
    ),
    JWC_WRITE_SCALE_MISMATCH: IssueCode(
        "warning",
        "retained",
        "JWC parser",
        "The header write scale differs from the selected layer group's scale; "
        "per-group scales were used.",
    ),
}

ALL_ISSUE_CODES: tuple[str, ...] = tuple(sorted(ISSUE_CODES))


def issue_code_details(code: str) -> dict[str, Any]:
    """Return JSON-friendly catalog metadata for *code*."""

    definition = ISSUE_CODES[code]
    return {
        "code": code,
        "severity": definition.severity,
        "action": definition.action,
        "area": definition.area,
        "condition": definition.condition,
    }
