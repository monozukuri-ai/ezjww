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
| `CP932_DECODE_REPLACED` | warning | normalized | JWW/JWC parser | One or more undecodable byte sequences were replaced with U+FFFD. `details.encoding` is `cp932` for JWC strings and JWW ANSI strings, or `utf-16le` for JWW Unicode strings marked with MFC `FF FE FF`. |
| `ENTITY_LIST_TRUNCATED` | error | skipped | JWW parser | The main entity list could not be read to its end (truncated upload, unknown record layout, corrupt tag). Entities parsed before the error are kept, block definitions behind the list are not read, and `details` carries `byte_offset`, `expected_entities`, `parsed_entities` and `error`. A list whose first entity already fails still raises. |
| `UNRESOLVED_BLOCK_REFERENCES` | warning | - | JWW validation | One or more block references could not be resolved. |
| `UNSUPPORTED_DXF_ENTITIES` | warning | skipped | DXF conversion | One or more parsed JWW entity kinds are unsupported by DXF conversion. |
| `JWC_HEADER_SETTINGS_UNVERIFIED` | info | retained | JWC parser | Header CSV settings (`header.csv0`, `header.csv1`, `header.csv2`) differ from the reference corpus. They are retained as raw text and do not affect geometry. `details` carries `field`, the first `byte_offset`, `count`, and up to eight `values` written as `[index]=text`. |
| `JWC_ATTRIBUTE_UNVERIFIED` | warning | retained | JWC parser | Record attribute values outside the reference corpus were retained: line-style high-nibble bits, line/arc/text/point flag bits, spare bytes, layer state bits, equal nonzero arc angles, and text preset slot 0 or zero-size presets. Aggregated per `details.field` with `count` and sample `values`. |
| `JWC_CURVE_MARKERS_UNVERIFIED` | warning | retained | JWC parser | Line curve-marker bits (`0x40` start, `0x80` member, `0xC0` end) did not form a verified sequence; the affected lines are kept ungrouped. `values` lists the offending markers or `unterminated`. |
| `JWC_GROUP_SCALE_DEFAULTED` | warning | normalized | JWC parser | A layer group in the `fixed2389_u16scale_v1` profile stores scale 0; scale 1 was substituted. `values` lists the group indices. |
| `JWC_WRITE_SCALE_MISMATCH` | warning | retained | JWC parser | The header write scale (`csv0[9]`) differs from the selected layer group's scale; per-group scales were used for conversion. |

## Structured diagnostics

`read_document(path)["diagnostics"]` and
`read_jwc_document(path)["diagnostics"]` contain parser diagnostics for JWW and
JWC respectively. The common reader exposes them at
`read_cad_document(path)["document"]["diagnostics"]`. Python `audit(path)` and
`report(path)["audit"]` include parser diagnostics together with validation and
conversion diagnostics. A diagnostic has the following structure (the offset and
field below are illustrative):

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

`byte_offset` is a zero-based absolute file offset to the first byte of the
affected encoded string. For JWW, this is the CString payload after its length
prefix; for JWC it is the name or text content in its source span. `byte_length`
is the encoded content length. The parser continues with replacement decoding,
so the affected parsed string contains U+FFFD. JWC also retains the raw bytes.

The audit result also includes aggregate fields:

- `decode_error_count`: number of affected string fields;
- `decode_replacement_characters`: total U+FFFD characters inserted by the
  decoder;
- `decode_affected_fields`: unique parser field paths in encounter order.

## Structural errors and conversion notices

`ENTITY_LIST_TRUNCATED` applies to JWW's partial-read behavior. A malformed or
unsupported JWC document raises an error with a byte offset instead of returning
a partial document. Python uses `ValueError` for these parse failures and
`OSError` subclasses for file I/O failures.

JWC conversion defaults, derived values, and DXF representation limits are
reported separately in `jwc_conversion_report.notices`. Notices such as font
substitution are not audit issue codes and alone do not set `has_issues` or
trigger the CLI's `--fail-on-issues`. See the [API reference](JWC_API.md) for the
conversion report fields.

## Python access

Use `ezjww.ALL_ISSUE_CODES` for exhaustive CI checks,
`ezjww.ISSUE_CODES` for catalog metadata, and
`ezjww.issue_code_details(code)` for JSON-friendly metadata.
