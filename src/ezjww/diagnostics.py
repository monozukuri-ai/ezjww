"""Stable audit issue contracts and the public issue-code catalog."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal

IssueSeverity = Literal["info", "warning", "error"]

CP932_DECODE_REPLACED = "CP932_DECODE_REPLACED"
ENTITY_LIST_TRUNCATED = "ENTITY_LIST_TRUNCATED"
UNRESOLVED_BLOCK_REFERENCES = "UNRESOLVED_BLOCK_REFERENCES"
UNSUPPORTED_DXF_ENTITIES = "UNSUPPORTED_DXF_ENTITIES"


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
        "One or more undecodable CP932 byte sequences were replaced.",
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
