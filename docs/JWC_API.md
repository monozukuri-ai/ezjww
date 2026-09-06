# JWW and JWC API reference

Use the common readers to detect the input format automatically. JWC reads are
limited to the [supported binary profile](JWC_FORMAT.md).

## Choose a reader

Python's low-level readers take a file path string. TypeScript readers take
`Uint8Array | ArrayBuffer | ArrayBufferView` (`CadInput`, also named `JwwInput`);
buffer views retain their byte offset and length.

| Purpose | Python | TypeScript |
| --- | --- | --- |
| Detect the format | `detect_file_format` | `detectFileFormat` |
| Check the JWW signature | `is_jww_file` | `isJwwFile` |
| Read a JWW header | `read_header` | `readHeader` |
| Read a JWW source document | `read_document` | `readDocument` |
| Check the JWC signature | `is_jwc_file` | `isJwcFile` |
| Read a JWC header | `read_jwc_header` | `readJwcHeader` |
| Read a JWC source document | `read_jwc_document` | `readJwcDocument` |
| Read either source document | `read_cad_document` | `readCadDocument` |
| Convert either format to DXF data | `read_dxf_document` | `readDxfDocument` |
| Convert either format to DXF text | `read_dxf_string` | `readDxfString` |

Detection returns `"jww"`, `"jwc"`, or `None`/`null` based on file contents.
Signature checks are case-sensitive and do not validate the remaining structure.
A JWC header read requires the complete file because it checks record boundaries
and metadata after the entity records. A successful header read does not validate
all entity geometry; use a document reader for that.

The common source document is a discriminated result:

```python
import ezjww

cad = ezjww.read_cad_document("drawing.jwc")
if cad["format"] == "jwc":
    print(cad["document"]["header"]["source_version"])  # currently None
else:
    print(cad["document"]["header"]["version"])         # JWW integer version
```

Its `document` is the same payload returned by the corresponding format-specific
reader. JWW-only APIs continue to accept JWW only.

JWW documents expose printer/view settings separately as `metadata_settings`,
with `key` and `value` fields, while retaining the original source entities.
Use these settings when inspecting document metadata rather than treating them
as ordinary drawing text.

## Source data and converted geometry

A JWC document contains `profile_id`, `header`, `entities`, `entity_counts`, and
`diagnostics`. Entities retain original coordinates and attributes, source byte
spans, and raw record bytes. Text also retains its CP932 content bytes separately
from the fixed record. These coordinates have not been converted to millimeters.

`header.source_version` is currently `None`/`null`; the field is typed as a
nullable string. The profile identifier describes ezjww's accepted layout, not
the creating application or a JWW version. The header does not invent a source
font or RGB palette.

Source entity counts follow stored record categories. Full circles and full
ellipses both count as `CIRCLE`; inspect `flatness` to distinguish them. Converted
DXF geometry represents full ellipses as `ELLIPSE`.

## Python Drawing API

```python
import ezjww

drawing = ezjww.readfile("drawing.jwc")
source = drawing.source_document
print(drawing.source_format, drawing.header)
lines = drawing.modelspace().query("LINE")

drawing.saveas("drawing.dxf", target_version="AC1024")
conversion = drawing.report()["jwc_conversion_report"]
for notice in conversion["notices"]:
    print(notice)

model = ezjww.Drawing.from_file(
    "drawing.jwc", jwc_coordinates="model_millimeters"
)
print(model.bbox(), model.stats())
model.plot(save_path="drawing.pdf")  # requires ezjww[plot]
```

`readfile` and `Drawing.from_file` also accept `pathlib.Path`. Choose JWC
coordinates when creating the Drawing: `paper_millimeters` is the default;
`model_millimeters` applies each entity's own layer-group scale once. This option
does not change JWW conversion.

`source_document` and `header` expose the original format's data. The existing
`jww_document` property returns that source document for JWW and `None` for JWC.
An empty drawing from `ezjww.new()` has no source format, source document, or header.
When constructing a Drawing manually, supply `source_document` and `source_format`
together; they cannot be combined with `jww_document`.

The module functions `audit`, `bbox`, `stats`, `report`, `to_dxf_string`, and
`plot_jww` also accept supported JWC input and `jwc_coordinates=`. The existing
name `plot_jww` is retained for compatibility. Low-level DXF readers and writers
accept the same coordinate option as a keyword-only argument.

Statistics and bounding boxes include hidden entities. TEXT bounds use insertion
points, not rendered glyph outlines. JWC previews hide invisible layers and use
substitute fonts; exact source glyphs and character spacing are not reproduced.

## DXF output and reports

```python
import ezjww

result = ezjww.write_dxf_with_report(
    "drawing.jwc",
    "drawing.dxf",
    target_version="AC1024",
    jwc_coordinates="model_millimeters",
)
print(result["source_format"], result["source_profile"])
print(result["jwc_conversion_report"]["notices"])
```

`read_dxf_document` / `readDxfDocument` return `layers`, `entities`, `blocks`, and
`unsupported_entities`. JWC conversion also adds:

| Field | Meaning |
| --- | --- |
| `jwc_conversion_report` | Coordinate space, rendering policy, source mappings, conversion notices, and parser diagnostics |
| `text_width_factors` | Width/height ratios in the order of output TEXT entities |

These extra fields are absent from JWW results. JWC write reports include
`source_format="jwc"`, `source_profile`, `source_version=None`, and the conversion
report. The existing JWW report fields retain their meanings.

DXF text/file output supports AC1015 (default) and AC1024. The JWC writers preserve
TEXT width factors (group 41) and millimeter units (`$INSUNITS=4`) in both coordinate
modes. `write_dxf`, `write_dxf_with_report`, and `Drawing.saveas` use this behavior.

`explode_inserts` / `explodeInserts` expands JWW block references. The supported
JWC profile contains no block definitions, so expansion leaves its geometry
unchanged, including full ellipses. `max_block_nesting` / `maxBlockNesting`
defaults to 32 and must be at least 1.

## TypeScript

```typescript
import { readFileSync, writeFileSync } from "node:fs";
import { readCadDocument, readDxfDocument, readDxfString } from "ezjww";

const bytes = readFileSync("drawing.jwc");
const cad = readCadDocument(bytes);
if (cad.format === "jwc") {
  console.log(cad.document.header.paper, cad.document.header.source_version);
} else {
  console.log(cad.document.header.version);
}
const options = { jwcCoordinates: "model_millimeters" as const };
const dxf = readDxfDocument(bytes, options);
console.log(dxf.jwc_conversion_report?.notices);
writeFileSync("drawing.dxf", readDxfString(bytes, {
  ...options,
  targetVersion: "AC1024",
}));
```

`DxfOptions` uses camelCase option names: `jwcCoordinates`, `targetVersion`,
`explodeInserts`, and `maxBlockNesting`. `targetVersion` selects the version for
string output. `toDxfString` is an alias of `readDxfString`.

The npm entry point targets Node.js. For web-target WASM initialization and
local file handling, see the [browser example](../packages/ezjww/examples/browser/README.md).

## Errors and diagnostics

Malformed or unsupported JWC raises Python `ValueError` with a source byte
offset; file I/O raises `OSError` subclasses. Unknown formats raise `ValueError`
through the common reader. TypeScript parsing functions throw on invalid or
unsupported input. JWC document readers do not return partial documents on
structural failure.

Invalid CP932 sequences can be decoded with replacement characters while retaining
the original bytes. These produce structured `CP932_DECODE_REPLACED` diagnostics
in the source document and conversion report, and in Python `audit`/`report`.
See the [diagnostic catalog](DIAGNOSTICS.md).

Conversion notices are separate from audit issue codes:

| Notice kind | Meaning |
| --- | --- |
| `default` | A rendering value supplied by the converter |
| `derived` | A value calculated from source attributes |
| `dxf_limitation` | Source information that DXF cannot reproduce directly |

Font substitution and similar notices alone do not set `has_issues` or trigger
the CLI's `--fail-on-issues`. Those checks use parser and audit warnings/errors.

## Command line

```bash
ezjww info drawing.jwc --json
ezjww report drawing.jwc --json
ezjww bbox drawing.jwc --jwc-coordinates model_millimeters --json
ezjww to-dxf drawing.jwc -o drawing.dxf
ezjww to-dxf-dir drawings -o dxf --recursive --report json
ezjww plot drawing.jwc -o drawing.pdf
```

`info --json` returns the JWC source document with `source_format="jwc"`;
JWW retains its existing JSON shape. Directory conversion finds `.jww` and `.jwc`
case-insensitively and preserves subdirectories. Output-name collisions, including
case-only collisions, fail with exit code 2 before any output is created.

## Rust conversion

The core exposes `parse_cad_document` and `read_cad_document_from_file`, returning
`CadDocument`, as well as format-specific readers. JWC conversion can be used
directly:

```rust
use ezjww_core::jwc::{read_jwc_dxf_from_file, JwcConvertOptions, JwcCoordinateSpace};
use ezjww_core::DxfTargetVersion;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = read_jwc_dxf_from_file("drawing.jwc", JwcConvertOptions {
        coordinates: JwcCoordinateSpace::ModelMillimeters,
        ..Default::default()
    })?;
    result.write_to_file("drawing.dxf", DxfTargetVersion::Ac1024)?;
    Ok(())
}
```

Use `JwcDxfConversion::to_dxf_string` or `write_to_file` for JWC output. The
conversion result carries text width factors alongside its DXF document; passing
only the document to the generic DXF writer loses those factors.
