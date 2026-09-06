# JWC support and conversion limits

ezjww reads the binary JWC document profile `fixed2421_basic_v1`, using the
`fixed2421_csv32_v1` header layout. These are ezjww profile identifiers, not Jw_cad
version numbers. This page describes the accepted input and conversion behavior;
it is not a complete specification of every JWC generation.

Compatibility with other layouts and old DOS-era files is not established.
The source version is unknown and returned as `source_version=None` in Python
or `null` in TypeScript. JWC writing, JWK/JWS files, and `JWC_TEMP` text files
are not supported.

## Detection and validation

The JWC binary signature is the case-sensitive 13-byte ASCII prefix
`jw_cad(c)data`. A matching prefix identifies a format candidate. It does not
guarantee that the layout or geometry can be parsed.

The supported layout has a 2,421-byte fixed header, fixed-size entity records,
a text string pool, and layer/group metadata. Readers validate boundaries,
counts, supported settings and flags, and string references. Header reads require
the complete file and do not validate every entity's geometry.

| Constraint | Accepted input |
| --- | --- |
| Paper | A0 through A4 |
| Group scales | Positive, finite values consistent with the header |
| Coordinates and radii | Finite coordinates; positive radii |
| Pen colors | JWC indices 1–5 for records with a color |
| Line styles | JWC indices 1–3 for lines and arcs |
| Text presets | Indices 1–10 with positive width and height |
| Text encoding | CP932, with structured replacement diagnostics for invalid sequences |
| Text content | 1–256 bytes before the NUL terminator, with a nonzero-length baseline |
| File size | At most 64 MiB |
| Record count | At most 100,000, including auxiliary points |
| Auxiliary points | At most 100 |
| Text pool | At most 65,535 bytes in the supported layout |

These are implementation limits for this profile. Unknown layouts, settings,
attributes, or flags are rejected. Inconsistent counts, truncated records,
out-of-range string references, gaps in the string pool, and unexpected trailing
data also fail. A document read returns either a complete supported document or
an error; structural failures do not produce partial JWC documents.

## Geometry

| Source geometry | DXF output | Limits |
| --- | --- | --- |
| Line | `LINE` | Supported curve-marker sequences remain line segments |
| Full circle | `CIRCLE` | Positive radius |
| Circular arc | `ARC` | Supported start/end angles and zero circle tilt |
| Full ellipse | `ELLIPSE` | Axis ratio and rotation are retained |
| Text | `TEXT` | Content, insertion, height, width factor, and rotation are retained |
| Ordinary point | `POINT` | Source position, layer, and color are retained |
| Auxiliary point | `POINT` | Position and layer are retained; color uses a conversion default |

Partial ellipses, empty text records, and unsupported curve markers are rejected.
Curve sequences are not fitted to splines. The profile has no block definitions;
enabling INSERT expansion leaves its geometry unchanged.

The source document preserves its record order: auxiliary points, lines, arcs,
text, then ordinary points. Source spans are absolute byte offsets and lengths.
Raw bytes and source coordinates remain available independently of DXF conversion.

## Coordinates and units

DXF conversion defaults to `paper_millimeters`. Select `model_millimeters` using
Python's `jwc_coordinates`, TypeScript's `jwcCoordinates`, or the CLI's
`--jwc-coordinates` option.

Let `W` and `H` be sheet width and height in millimeters, `x`, `y`, and `r` be
source coordinates and radius, and `s` be the entity's own layer-group scale.

| Value | Paper millimeters | Model millimeters |
| --- | --- | --- |
| X | `x * W / 518 - W / 2` | Paper X multiplied by `s` |
| Y | `y * W / 518 - H / 2` | Paper Y multiplied by `s` |
| Radius | `r * W / 518` | Paper radius multiplied by `s` |
| Text width, height, spacing | Preset value divided by 10 | Paper dimension multiplied by `s` |

Both modes use the sheet center as `(0, 0)`, +X to the right, and +Y up.
Model coordinates apply each entity's scale once; the currently selected writing
group does not determine the scale for the entire drawing. Conversion does not
recover precision lost in the source's single-precision coordinates.

Arc angles use a positive counterclockwise sweep. Full ellipses retain their axis
ratio and rotation. Text rotation follows its source baseline.

Both AC1015 and AC1024 JWC output set `$INSUNITS=4` (millimeters) and preserve
TEXT width/height ratios in DXF group 41.

## Rendering defaults

JWC pen indices are source attributes. The following output palette and styling
are converter defaults, not an RGB palette or font recovered from the file.
They are identified by the rendering policy `native_10021_reference_v1` in the
conversion report.

| Attribute | Output behavior |
| --- | --- |
| Pen colors 1, 2, 3, 4, 5 | DXF ACI colors 132, 18, 92, 52, 212 |
| Line style 1 | `CONTINUOUS` |
| Line style 2 | `JWC_DASHED1`: 1.25 mm dash, 1.25 mm gap |
| Line style 3 | `JWC_DASHED2`: 2.5 mm dash, 2.5 mm gap |
| Auxiliary-point color | ACI 18 |
| Font | DXF `STANDARD`/`txt`; the viewer supplies a substitute font |
| Missing line width | DXF entity width inherits layer behavior; no source width is inferred |
| Layer table color/style | Neutral ACI 7 and `CONTINUOUS`; entities carry explicit values |

Dash lengths are fixed in output millimeters in both coordinate modes; they are
not multiplied by the layer-group scale. JWC line style 3 is a long dash, not
JWW's dash-dot style with the same numeric index.

Text spacing is retained during normalization, but DXF TEXT has no directly
equivalent spacing field. Previews and exported DXF preserve numeric size,
rotation, and width factor; exact glyph shapes and character spacing are not
reproduced.

## Layers and visibility

Source group/layer names and states remain in the source document. Conversion
sanitizes names that DXF cannot represent and resolves duplicate output names.
Use the converted layer table and each entity's `layer` to inspect output names.

If either the group or layer is invisible, its DXF layer is frozen. Protected or
noneditable groups/layers become locked. Geometry is retained even when hidden.
JWC previews hide invisible layers; statistics and bounding boxes still include
their entities. TEXT bounds cover its insertion point, not the glyph outline.

## Conversion metadata

`jwc_conversion_report` records the coordinate space, rendering policy, parser
diagnostics, notices, and mappings between output geometry and source entity
indices/byte spans. Mappings also record the applied scale.

Notices distinguish defaults, derived values, and DXF limitations. They are
separate from parser and audit issue codes. See the [API reference](JWC_API.md)
for access from Python/TypeScript and the [diagnostic catalog](DIAGNOSTICS.md)
for error handling.
