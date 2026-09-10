# JWC support and conversion limits

ezjww reads two binary JWC header profiles that share one record layout. These are
ezjww profile identifiers, not Jw_cad version numbers. This page describes the
accepted input and conversion behavior; it is not a complete specification of
every JWC generation.

| Header profile | Document profile | Signature byte 22 | Fixed header | Layer-group scales |
| --- | --- | --- | --- | --- |
| `fixed2421_csv32_v1` | `fixed2421_basic_v1` | `f` | 2,421 bytes | `f32 × 16` at offset 1797 |
| `fixed2389_u16scale_v1` | `fixed2389_basic_v1` | `.` | 2,389 bytes | `u16 × 16` at offset 1797; every later header offset is 32 bytes earlier |

The source version is unknown and returned as `source_version=None` in Python
or `null` in TypeScript. JWC writing, JWK/JWS files, and `JWC_TEMP` text files
are not supported.

## Detection and validation

The JWC binary signature is the case-sensitive 13-byte ASCII prefix
`jw_cad(c)data`. A matching prefix identifies a format candidate. It does not
guarantee that the layout or geometry can be parsed. The 40-byte signature must
otherwise match `jw_cad(c)data.......a.?.m...............`, where byte 22
selects the header profile above.

Both profiles have a fixed header with three bounded CSV blocks (offsets 200,
400, 600), temporary-point arrays, text preset tables, layer-group scales,
edit/visible flags and write-layer bytes, followed by fixed-size entity records,
a text string pool, normal points and the 2,304-byte name area. Readers validate
this framing and fail on structural inconsistencies:

- fixed-position LFs and CSV framing (NUL terminator, NUL/space padding, ASCII);
- entity counts and their record spans, which must end exactly at the name area;
- string-pool addresses and text references (contiguous, NUL-terminated, in range);
- temporary-point slot usage, write-layer packing, paper code, finite
  coordinates, positive radii, angles in `[0, 360)`;
- pen colors outside 1–9, line-style numbers outside 1–9, text presets outside
  1–10, empty text, zero-length text baselines.

Values that merely differ from ezjww's reference corpus no longer reject a file.
They are retained in the source document and reported once per field as
`JWC_HEADER_SETTINGS_UNVERIFIED`, `JWC_ATTRIBUTE_UNVERIFIED`,
`JWC_CURVE_MARKERS_UNVERIFIED`, `JWC_GROUP_SCALE_DEFAULTED`, or
`JWC_WRITE_SCALE_MISMATCH` (see the [diagnostic catalog](DIAGNOSTICS.md)).

| Constraint | Accepted input |
| --- | --- |
| Paper | A0 through A4 |
| Group scales | Positive, finite values (u16 profile: 0 is substituted with 1 and reported) |
| Coordinates and radii | Finite coordinates; positive radii |
| Pen colors | 1–9 for records with a color (1–5 have a native color reference) |
| Line styles | Low nibble 1–9 (JWW numbering); high-nibble attribute bits are retained |
| Text presets | Indices 1–10; zero-size presets are drawn at 2.0 mm and reported |
| Text encoding | CP932, with structured replacement diagnostics for invalid sequences |
| Text content | 1–256 bytes before the NUL terminator, with a nonzero-length baseline |
| File size | At most 64 MiB |
| Record count | At most 100,000, including auxiliary points |
| Auxiliary points | At most 100 |
| Text pool | At most 65,535 bytes |

The string pool is addressed by DOS far pointers: the storage CSV at offset 600
starts with `SSSS:OOOO` (segment:offset, hexadecimal) for the pool start and end,
and each text record's 32-bit string reference carries the same form. The
pool-relative offset is `reference − pool start`. Segments `0000`, `4000` and
`6647` have been observed; the reference corpus always used `4000`.

A document read returns either a complete document or an error; structural
failures do not produce partial JWC documents.

## Geometry

| Source geometry | DXF output | Notes |
| --- | --- | --- |
| Line | `LINE` | Curve-marker sequences remain line segments; unknown flag bits are retained |
| Full circle | `CIRCLE` | Equal start and end angles denote a closed curve |
| Circular arc | `ARC` | The tilt angle rotates the start/end angles (the angles are measured in the tilted frame) |
| Full ellipse | `ELLIPSE` | Axis ratio and rotation are retained |
| Elliptical arc | `ELLIPSE` with start/end parameters | Angles are parameters in the tilted frame |
| Text | `TEXT` | Content, insertion, height, width factor, and rotation are retained |
| Ordinary point | `POINT` | Source position, layer, and color are retained; flag bits are retained |
| Auxiliary point | `POINT` | Position and layer are retained; color uses a conversion default |

Curve sequences are not fitted to splines. The profiles have no block definitions;
enabling INSERT expansion leaves the geometry unchanged.

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

Arc angles use a positive counterclockwise sweep. Text rotation follows its
source baseline.

Both AC1015 and AC1024 JWC output set `$INSUNITS=4` (millimeters) and preserve
TEXT width/height ratios in DXF group 41.

## Rendering defaults

JWC pen and style indices are source attributes. The following output palette and
styling are converter defaults, not an RGB palette or font recovered from the
file. They are identified by the rendering policy `native_10021_reference_v1` in
the conversion report.

| Attribute | Output behavior |
| --- | --- |
| Pen colors 1, 2, 3, 4, 5 | DXF ACI colors 132, 18, 92, 52, 212 (native reference DXF) |
| Pen colors 6, 7, 8, 9 | DXF ACI colors 170, 96, 10, 8 (placeholders; observed only in real files) |
| Line style 1 | `CONTINUOUS` |
| Line style 2 | `JWC_DASHED1`: 1.25 mm dash, 1.25 mm gap (native reference) |
| Line style 3 | `JWC_DASHED2`: 2.5 mm dash, 2.5 mm gap (native reference) |
| Line style 4 | `JWC_DASHED3`: 0.6 mm dash, 0.6 mm gap (placeholder) |
| Line styles 5, 6 | `JWC_DASHDOT1` / `JWC_DASHDOT2`: dash-dot, 7.5/1.25 mm and 12.5/2.5 mm (placeholders) |
| Line styles 7, 8 | `JWC_DIVIDE1` / `JWC_DIVIDE2`: double dash-dot (placeholders) |
| Line style 9 | `CONTINUOUS` (auxiliary lines are not hidden) |
| Auxiliary-point color | ACI 18 |
| Font | DXF `STANDARD`/`txt`; the viewer supplies a substitute font |
| Missing line width | DXF entity width inherits layer behavior; no source width is inferred |
| Layer table color/style | Neutral ACI 7 and `CONTINUOUS`; entities carry explicit values |

Dash lengths are fixed in output millimeters in both coordinate modes; they are
not multiplied by the layer-group scale. Style numbers follow JWW numbering
(1 continuous, 2–4 dashed, 5–6 dash-dot, 7–8 double dash-dot, 9 auxiliary);
JWC line style 3 is a long dash in the native reference, not JWW's dash-dot.

Text spacing is retained during normalization, but DXF TEXT has no directly
equivalent spacing field. Previews and exported DXF preserve numeric size,
rotation, and width factor; exact glyph shapes and character spacing are not
reproduced.

## Layers and visibility

Source group/layer names and states remain in the source document. Name slots
are NUL-terminated; bytes after the terminator are ignored because real files
leave uninitialized memory there. Conversion sanitizes names that DXF cannot
represent and resolves duplicate output names. Use the converted layer table and
each entity's `layer` to inspect output names.

Edit bytes use bit 0 for editable and bit 1 for protected; visible bytes use
bit 0. Other bits are retained and reported. If either the group or layer is
invisible, its DXF layer is frozen. Protected or noneditable groups/layers become
locked. Geometry is retained even when hidden. JWC previews hide invisible
layers; statistics and bounding boxes still include their entities. TEXT bounds
cover its insertion point, not the glyph outline.

## Conversion metadata

`jwc_conversion_report` records the coordinate space, rendering policy, parser
diagnostics, notices, and mappings between output geometry and source entity
indices/byte spans. Mappings also record the applied scale.

Notices distinguish defaults, derived values, and DXF limitations. They are
separate from parser and audit issue codes. Notices added for real files are
`arc.tilt` (derived: circular arcs rotated by their tilt) and `text.size`
(default: zero-size preset replaced by 2.0 mm). See the [API reference](JWC_API.md)
for access from Python/TypeScript and the [diagnostic catalog](DIAGNOSTICS.md)
for error handling.
