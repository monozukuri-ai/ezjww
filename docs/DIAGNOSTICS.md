# Audit issue code catalog

Audit issue codes are stable machine-readable identifiers. They are part of
the public semantic-versioning contract:

- adding a code is a minor-version change;
- removing a code or changing its meaning is a major-version change;
- message wording and structured details may be clarified without changing a
  code's meaning;
- consumers should preserve unknown codes so newer producers remain usable.

The severity and action below are defaults. One code may occur in multiple
diagnostic instances with different byte offsets, fields, or entity types.
`audit()["issue_codes"]` contains each emitted code once, while
`audit()["diagnostics"]` preserves every instance.

| Code | Default severity | Action | Area | Emitted when |
|---|---|---|---|---|
| `CP932_DECODE_REPLACED` | warning | normalized | JWW parser | One or more undecodable byte sequences were replaced with U+FFFD while parsing a JWW string. `details.encoding` is `cp932` for ANSI strings and `utf-16le` for the Unicode strings (MFC `FF FE FF` marker) written by Jw_cad 8's Unicode builds. |
| `ENTITY_LIST_TRUNCATED` | error | skipped | JWW parser | The main entity list could not be read to its end (truncated upload, unknown record layout, corrupt tag). Entities parsed before the error are kept, block definitions behind the list are not read, and `details` carries `byte_offset`, `expected_entities`, `parsed_entities` and `error`. A list whose first entity already fails still raises. |
| `UNRESOLVED_BLOCK_REFERENCES` | warning | - | JWW validation | One or more block references could not be resolved. |
| `UNSUPPORTED_DXF_ENTITIES` | warning | skipped | DXF conversion | One or more parsed JWW entity kinds are unsupported by DXF conversion. |

## Structured diagnostics

`read_document(path)["diagnostics"]` contains parser diagnostics. `audit(path)`
and `report(path)["audit"]` include those parser diagnostics together with
validation and conversion diagnostics:

```json
{
  "code": "CP932_DECODE_REPLACED",
  "severity": "warning",
  "message": "CP932 decoding replaced 1 undecodable character sequence(s) in entity.text.content.",
  "action": "normalized",
  "details": {
    "encoding": "cp932",
    "field": "entity.text.content",
    "byte_offset": 1234,
    "byte_length": 8,
    "replacement_characters": 1,
    "had_errors": true
  }
}
```

`byte_offset` is a zero-based absolute offset from the start of the JWW file to
the first byte of the CString payload, after its length prefix. `byte_length`
is the encoded payload length. The parser continues with replacement decoding,
so the affected parsed string contains U+FFFD.

The audit result also includes aggregate fields:

- `decode_error_count`: number of affected CString fields;
- `decode_replacement_characters`: total U+FFFD characters inserted by the
  decoder;
- `decode_affected_fields`: unique parser field paths in encounter order.

## Python access

Use `ezjww.ALL_ISSUE_CODES` for exhaustive CI checks,
`ezjww.ISSUE_CODES` for catalog metadata, and
`ezjww.issue_code_details(code)` for JSON-friendly metadata.

