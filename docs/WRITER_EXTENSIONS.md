# Writer extensions requested by zumen-gate

Version **0.5.0** addresses the R1–R8 requests dated 2026-10-04. It is currently
unreleased; use a source build. The public writer produces new version-700 documents.
Validation runs before bytes are returned or a destination file is replaced.

## Request coverage

| Request | API / behavior |
| --- | --- |
| R1 colors | `color=` on Python builders; raw entity `color`; `options.palette`; valid native pens 1–9 and 100–356 |
| R2 line types | 1–9, 16–19, 30–62; `options.line_types` |
| R3 fills | `add_solid`, `add_circle_solid`; raw `SOLID` / `CIRCLE_SOLID`; explicit zero-area warning/skip policy |
| R4 ellipses | `add_arc(..., flatness=, tilt_angle=)` and `add_ellipse` |
| R5 dimensions | `add_dimension`; inline LINE/TEXT/auxiliary bases; named dimension flags |
| R6 blocks | `add_block`, `add_block_ref`; raw `block_defs`; cycle/depth/reference validation |
| R7 text | Optional endpoint, per-font factors, ten editable presets, style offsets, multiline helper |
| R8 helpers | `select_layer`, `extend`, public `new_jww_document` / `to_jww_bytes`, explicit `to_write_document` / `toWriteDocument`, paper dimensions and Linux/Wine harness |

Version 600 remains unsupported, as permitted by the low-priority request.
Images, marker/temporary points, alternate text anchors and arbitrary curve flags
remain outside the bounded writer. The native application's editing, printing
and Windows desktop behavior need separate qualification.

## Colors and patterns

Values use **COLORREF `0x00BBGGRR`**, as returned by the reader. For example,
`0x0000FF` is red. To convert an RGB hex value, swap its red and blue bytes.
Ordinary elements have no inline RGB field in the file: the writer reuses a
matching pen or allocates an unused user slot 117–356. It scans modelspace,
block definitions and dimension members first, so referenced pens are never
overwritten. It reports exhaustion instead of approximating colors. Input
objects are unchanged. The resolved pen appears in the serialized readback.

Native pens are 1–9, 100 (reserved), 101–116 (SXF fixed) and 117–356 (user).
The request's proposed 10–256 range is not the native range. Pen 10 is only
valid for a SOLID/CIRCLE_SOLID with an inline `color`; 16 is not a line color.
Jw_cad normalizes reserved pen 100 to 1 on save. Use 101–356 for retained SXF pens.

`new_jww_document()` supplies editable default tables. `palette.pen_colors` has
10 entries (index 0 is background), and `extended_colors` has 257 entries indexed
by `pen - 100`. Changed colors patch both screen and printer colors; native
line-width settings retain template defaults. This is not full print-style editing.

`line_types` contains 8 standard, 5 random, 4 double-length and 33 SXF slots.
Slot numbers must remain in order. Standard/double patterns use a 32-bit pattern,
`unit_dots` 1–32 and positive screen/printer pitches. SXF slots add a name and
zero or up to 10 positive alternating dash/gap lengths in mm (an even count).
Reserved slot 30 may retain its template's zero unit length; Jw_cad normalizes
a style-30 entity to style 9 and rewrites that unused table slot on save. Reader-derived
`runs` and standard/double `segments_mm` are accepted but recomputed from the
stored fields; edit the pattern/pitches. SXF `segments_mm` is a stored field.

```python
import ezjww

drawing = ezjww.new()
drawing.select_layer(2, 5)
msp = drawing.modelspace()
msp.add_line((0, 0), (100, 0), color=0x1256AB, jwwattribs={"pen_style": 47})
slot = drawing.options["line_types"]["sxf"][17]  # style 47
slot.update(name="custom 6-2", pattern=0x77777777, unit_dots=4,
            pitch=1, printer_pitch=64, segments_mm=[6.0, 2.0])
drawing.saveas("sxf.jww")
```

## Solids and ellipses

```python
msp.add_solid((0, 0), (20, 0), (20, 10), (0, 10), color=0x1256AB)
msp.add_solid((30, 0), (50, 0), (40, 10))  # triangle
msp.add_circle_solid((20, 30), 10, start_angle=30, sweep_angle=240)
msp.add_circle_solid((50, 30), 15, inner_radius=8, flatness=0.8, ring_style=106)
msp.add_ellipse((80, 30), 20, 0.5, tilt_angle=30, start_angle=20, sweep_angle=240)
```

Python solid arguments follow the polygon boundary. Native field traversal is
`point1 → point4 → point2 → point3`; the helper maps to those reader field names.
A triangle duplicates its final boundary vertex. Nonfinite, zero-area and crossing
solids fail validation. `degenerate="warn_skip"` explicitly warns and returns
`None` without inserting a zero-area solid. The default `"reject"` leaves rejection
to serialization, like other builders.

Circle-solid style 101 represents a disk/sector/chord; ring styles 105/106 store
inner radius in `solid_mode`. Style 105 scales the inner ellipse proportionally;
106 keeps the band width constant. The helper validates `0 < inner_radius < radius`
and the minor-axis limit for 106. The raw schema also accepts documented styles
101 and 111/modes; it rejects unknown combinations.

All Python geometry angles are degrees. Raw arc/circle-solid/block fields are
radians. Ellipse `flatness` is the minor/major radius ratio `(0, 1]`; partial arc
angles are in the tilted ellipse-axis coordinate system. `CIRCLE` is also the
reader's type for a full ellipse. A full ellipse has start 0 and sweep 2π.

## Dimensions and blocks

```python
msp.add_dimension((0, 0), (40, 0), "40", (15, 4), sxf_mode=1,
    aux_lines=[((0, -10), (0, 5)), ((40, -10), (40, 5))],
    aux_points=[(0, 0), (40, 0), (0, -10), (40, -10)],
    text_options={"size_x": 3, "size_y": 3})
part = ezjww.new().modelspace()
part.add_line((0, 0), (1000, 0))
number = drawing.add_block("part", part.entities)
msp.add_block_ref(number, (50, 50), scale_x=0.01, scale_y=0.01, rotation=30)
```

Dimensions use `CDataSunpou`, containing a LINE, TEXT, two auxiliary LINE slots and
four POINT slots. Non-SXF dimensions may omit auxiliaries in the Python helper;
unused slots are zero-filled. SXF mode 1 requires all six slots. Raw input always
requires their full lengths. The helper sets LINE/outer flags `0x2000`, value TEXT
`0x4010` and auxiliary POINT `0x0040`. Use `jwwattribs={"dimension": "line"}`,
`"value_text"` or `"aux"` for individual elements; conflicting flags are errors.
Supported text dimension-setting bits are `0x3ffa`, with mutually exclusive
rounding/prefix-suffix choices validated. The caller supplies the displayed value;
this API does not implement native dimension editing or automatic measurement.

The reader now preserves each inline dimension member's `base`. The explicit
converter rejects older cached dimension dictionaries without these attributes;
read the source file again to avoid inventing lost colors/flags.

Block definitions own copied entity dictionaries and use local origin (0, 0).
Numbers/names must be unique, references must resolve, cycles fail, and nesting
is limited to 32 definitions. Nonuniform/negative nonzero scales are accepted.
Python computes `is_referenced` from the final graph. Raw inputs must set it
consistently. Block timestamps are deterministic zero. The native suffix `@@SfigorgFlag@@4` added on save is preserved verbatim;
other special block-name metadata is rejected by the bounded writer.

## Text

```python
msp.font_width_factors["ＭＳ ゴシック"] = 1.0
msp.add_text("日本語 ABC", (0, 0), spacing=0.5, angle=30)
drawing.options["text_presets"][2].update(size_x=4, size_y=5, spacing=0.5)
msp.add_text("Preset 3", (0, 20), text_type=3)
msp.add_multiline_text("first\nsecond", (0, 40), line_spacing=8)
```

Omitted endpoints use full-width=1, half-width=0.5 character widths, intercharacter
spacing and the text angle. A per-call `font_width_factor` overrides the modelspace
font map. This estimates layout; explicit endpoints remain available. Font
substitution can change native measurements. Presets 1–10 supply omitted width,
height, spacing and pen; explicit arguments take precedence.

`text_type` is preset 0–10 plus style offsets 10000/20000/30000. The official
[serialization code](https://www.jwcad.net/jwdatafmt.txt) labels 10000 italic and
20000 bold; the prose immediately above reverses those labels. This writer
preserves the numeric encoding. It does not claim font-style rendering parity
with the DXF converter. Text is single-line; the multiline helper inserts one
TEXT per line with explicit baseline spacing in paper mm.

## Bulk input and explicit reader conversion

```python
raw = ezjww.new_jww_document()
raw["entities"].extend([])  # native dictionaries, validated in one call
payload = ezjww.to_jww_bytes(raw)

result = ezjww.to_write_document(ezjww.read_document("source.jww"))
print(result["diagnostics"])
if result["document"] is not None:
    from pathlib import Path
    Path("normalized.jww").write_bytes(ezjww.to_jww_bytes(result["document"]))
```

TypeScript uses `toWriteDocument(parsed, skipUnsupported = false)` and
`toJwwBytes(result.document)`. The converter reports each unsupported entity
with a path and returns no document by default. Explicit `skip_unsupported=True`
(or the second TS argument `true`) permits reported omissions. Invalid block
references/header settings and reader error diagnostics still reject the document.
Reader warnings are included in the conversion diagnostics. Treat an empty or partial
result as such; a successful encode does not imply the entire drawing survived.

Every conversion warns that unexposed header settings, timestamps and embedded
files use new-document defaults. Current layer states may be normalized with a
diagnostic, and output version is 700. `readfile().saveas()` remains unsupported.
No private downstream drawing is copied into this repository.

## Linux/Wine native harness

Install Wine, Xvfb, xdotool and ImageMagick's `import`. Supply Jw_cad 10.02.1 and
its locally licensed runtime files; the harness verifies the executable hash.
It runs Linux Python and requires no Windows Python or `winreg`:

```sh
python scripts/jww/make_extension_cases.py /tmp/extension-inputs
python scripts/jww/native_roundtrip_wine.py \
  --install /path/to/jww --inputs /tmp/extension-inputs \
  --output /tmp/extension-native --display :194 \
  --font-dir /path/to/local/fonts
```

`--font-dir` is optional; supply fonts suitable for Japanese text measurements.
The output directory and X display must be new. The tool creates a fresh Wine
prefix, copies only the required application files and generated drawings, and
controls only its own prefix/display. It checks dialog titles, saved file names,
normal process exits and unchanged original bytes. Logs survive failures. The
New button coordinate is tied to this pinned version at Wine's default 96 DPI;
a changed dialog layout fails the title gate rather than proceeding silently.

The run folder includes both native JWW saves, DXF exports, direct screenshots,
and GDI copies when a single `View_Direct2d = 1` setting can be changed. These
copies are diagnostics; they do not add a writer setting or alter original inputs.
Do not distribute the application's binaries or fonts when archiving results.
See [native evidence and limitations](JWW_COMPATIBILITY.md).
