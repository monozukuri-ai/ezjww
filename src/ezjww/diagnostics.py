"""Stable audit issue contracts and the public issue-code catalog."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal

IssueSeverity = Literal["info", "warning", "error"]

CP932_DECODE_REPLACED = "CP932_DECODE_REPLACED"
TEXT_ENCODING_DETECTED = "TEXT_ENCODING_DETECTED"
ENTITY_LIST_TRUNCATED = "ENTITY_LIST_TRUNCATED"
IMAGE_LIST_TRUNCATED = "IMAGE_LIST_TRUNCATED"
UNRESOLVED_BLOCK_REFERENCES = "UNRESOLVED_BLOCK_REFERENCES"
UNSUPPORTED_DXF_ENTITIES = "UNSUPPORTED_DXF_ENTITIES"
JWC_HEADER_SETTINGS_UNVERIFIED = "JWC_HEADER_SETTINGS_UNVERIFIED"
JWC_ATTRIBUTE_UNVERIFIED = "JWC_ATTRIBUTE_UNVERIFIED"
JWC_CURVE_MARKERS_UNVERIFIED = "JWC_CURVE_MARKERS_UNVERIFIED"
JWC_GROUP_SCALE_DEFAULTED = "JWC_GROUP_SCALE_DEFAULTED"
JWC_WRITE_SCALE_MISMATCH = "JWC_WRITE_SCALE_MISMATCH"
JWC_ARC_ANGLE_NORMALIZED = "JWC_ARC_ANGLE_NORMALIZED"
JWC_ELLIPSE_AXES_SWAPPED = "JWC_ELLIPSE_AXES_SWAPPED"
JWC_LINE_STYLE_DEFAULTED = "JWC_LINE_STYLE_DEFAULTED"
JWS_SCHEMA_VERSION_USED = "JWS_SCHEMA_VERSION_USED"


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
        "One or more undecodable byte sequences (CP932 or UTF-8 for ANSI strings, "
        "UTF-16LE for Unicode strings) were replaced.",
    ),
    TEXT_ENCODING_DETECTED: IssueCode(
        "info",
        "normalized",
        "JWW parser",
        "Every non-ASCII ANSI string of the file is well-formed UTF-8, so the "
        "strings were read as UTF-8 instead of CP932 (Jw_cad under Wine with a "
        "UTF-8 locale writes such files).",
    ),
    ENTITY_LIST_TRUNCATED: IssueCode(
        "error",
        "skipped",
        "JWW parser",
        "The main entity list could not be read to its end; entities parsed before "
        "the error were kept and block definitions were not read.",
    ),
    IMAGE_LIST_TRUNCATED: IssueCode(
        "warning",
        "skipped",
        "JWW parser",
        "The version-700 image archive after the block definitions could not be "
        "read to its end; images read before the error were kept and the drawing "
        "itself is complete.",
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
    JWC_ARC_ANGLE_NORMALIZED: IssueCode(
        "info",
        "normalized",
        "JWC parser",
        "Arc angles stored outside [0, 360) degrees (or an end one full turn from "
        "the start) were wrapped; a full turn is read as a closed curve.",
    ),
    JWC_ELLIPSE_AXES_SWAPPED: IssueCode(
        "warning",
        "normalized",
        "JWC parser",
        "An arc stores a flatness above one, so its radius is the minor axis; the "
        "conversion swaps the axes (radius x flatness, 1 / flatness, tilt + 90 "
        "degrees) so the same ellipse has a flatness of at most one.",
    ),
    JWC_LINE_STYLE_DEFAULTED: IssueCode(
        "warning",
        "normalized",
        "JWC parser",
        "A record stores line style 0, which is not a Jw_cad style number; the "
        "conversion draws it continuous (style 1).",
    ),
    JWS_SCHEMA_VERSION_USED: IssueCode(
        "info",
        "normalized",
        "JWS parser",
        "The header version is older than the MFC class schema of the records "
        "(Jw_cad 6 writes header 420 with schema 600); the records were read with "
        "the schema's layout.",
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
