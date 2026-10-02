# ezjww

`ezjww` reads Jw_cad drawings and converts them to DXF. Its Rust core is
available through Python and TypeScript/WebAssembly.

- Read JWW files and a [limited binary JWC profile](https://github.com/monozukuri-ai/ezjww/blob/main/docs/JWC_FORMAT.md).
- Inspect source records, headers, layers, and parsing diagnostics.
- Query converted geometry, calculate bounds, and audit drawings.
- Export ASCII DXF (AC1015 or AC1024) and render PNG/PDF previews.
- Convert individual files or directories from the command line.

## Installation

Python 3.9 or later:

```bash
pip install ezjww
```

For previews, install the optional Matplotlib dependency:

```bash
pip install "ezjww[plot]"
```

For Node.js, see the [TypeScript package](https://github.com/monozukuri-ai/ezjww/blob/main/packages/ezjww/README.md):

```bash
npm install ezjww
```

To build the Python package from source, install a Rust toolchain, then run:

```bash
git clone https://github.com/monozukuri-ai/ezjww.git
cd ezjww
pip install .
```

## Python quick start

`readfile` detects JWW or JWC from the file contents. Replace `drawing.jww`
with your input path; the same API accepts supported `.jwc` files.

```python
import ezjww

drawing = ezjww.readfile("drawing.jww")
print(drawing.source_format, drawing.header)
print(drawing.stats())
print(drawing.bbox())

lines = drawing.modelspace().query("LINE")
health = drawing.audit()
for diagnostic in health["diagnostics"]:
    print(diagnostic["code"], diagnostic["details"])

drawing.saveas("drawing.dxf", target_version="AC1024")
```

Use `read_cad_document` when you need the original records. Its return value
identifies the format and contains the corresponding source document:

```python
import ezjww

cad = ezjww.read_cad_document("drawing.jwc")
source = cad["document"]
print(cad["format"], source["entity_counts"])

# JWC DXF geometry defaults to paper millimeters. Select model millimeters
# to apply each entity's layer-group scale.
model = ezjww.readfile("drawing.jwc", jwc_coordinates="model_millimeters")
model.saveas("drawing-model.dxf")
print(model.report()["jwc_conversion_report"]["notices"])
```

The JWW-specific `read_document` and `read_header` keep their original meanings.
Use `read_jwc_document` and `read_jwc_header` for JWC-specific reads.
See the [API reference](https://github.com/monozukuri-ai/ezjww/blob/main/docs/JWC_API.md) for format detection, coordinate options,
errors, and conversion metadata.

### Queries and previews

```python
import ezjww

drawing = ezjww.readfile("drawing.jww")
entities = drawing.modelspace().query('LINE POINT[layer=="#lv4", color==5]')
flat = drawing.to_dxf(explode_inserts=True, max_block_nesting=32)

drawing.plot(save_path="drawing.png")  # requires ezjww[plot]
drawing.plot(save_path="drawing.pdf")
```

DXF TEXT records include a width factor (group 41). For JWW, it is estimated
from the stored endpoint span and half/full-width character cells. Actual glyph
advances depend on the selected font; this is not an exact fit for every font
substitution. JWC retains its existing width/height ratio.

### Line type settings

A JWW file records how each of its line types is drawn. `read_header` and
`read_document` report them as `header["line_types"]` (`None` for files older
than version 3.00):

```python
import ezjww

line_types = ezjww.read_header("drawing.jww")["line_types"]
for item in line_types["standard"]:          # line types 2-9
    print(item["number"], item["runs"], item["segments_mm"])
for item in line_types["sxf"] or []:         # line types 30-62 (version 4.20+)
    print(item["number"], item["name"], item["segments_mm"])
```

- `standard` (2-9: dashed 1-3, chain 1-2, double-dot chain 1-2, construction
  line) and `double_length` (16-19) carry the bit `pattern`, its `unit_dots`,
  the screen `pitch` and the `printer_pitch`. `runs` is the pattern as dash and
  gap lengths in bits, starting with the longest dash, and `segments_mm` the
  lengths it prints at: one bit is `printer_pitch / 32` mm.
- `random` (11-15) carries the amplitude and pitch of the hand-drawn line types.
- `sxf` (30-62) carries the SXF-compatible line types with their `name` and the
  `segments_mm` stored in the file. Numbers 47-62 are user-defined.

Entities refer to a line type by `pen_style`.

### DXF line types

The DXF conversion names each Jw_cad line type and defines the ones a drawing
uses in the `LTYPE` table, with the dash pattern the file records (the Jw_cad
defaults for files older than version 3.00):

| Jw_cad line type | DXF linetype |
| --- | --- |
| 1, SXF 31 | `CONTINUOUS` |
| 2-4 (dashed 1-3) | `JWW_DASHED1`-`JWW_DASHED3` |
| 5-6 (chain 1-2) | `JWW_DASHDOT1`, `JWW_DASHDOT2` |
| 7-8 (double-dot chain 1-2) | `JWW_DIVIDE1`, `JWW_DIVIDE2` |
| 9 (construction line) | `JWW_CONSTRUCTION` |
| 16-19 (double length) | `JWW_DASHDOT_X2`, `JWW_DIVIDE_X2`, `JWW_DASHED_X2`, `JWW_DASHED_X4` |
| 32-45 (SXF predefined) | `SXF_DASHED`, `SXF_CHAIN`, ... |
| 47-62 (SXF user-defined) | `SXF_USER_17`-`SXF_USER_32` |
| 11-15 (random lines), undefined numbers | `BYLAYER` |

Pattern lengths are millimetres on paper, like the coordinates of a JWW
drawing, so the DXF needs no linetype scale. A line type whose pattern the user
turned solid has an empty pattern. Jw_cad does not print construction lines
(line type 9); they are converted like any other line, on the linetype
`JWW_CONSTRUCTION`, so that a reader can leave them out.

```python
import ezjww

document = ezjww.read_dxf_document("drawing.jww")
for line_type in document["line_types"]:
    print(line_type["name"], line_type["description"], line_type["pattern"])
```

`pattern` uses the DXF convention: positive is a dash, negative a gap. JWC
drawings keep the fixed linetypes of that format.

Use `text_em_scale` when a target renderer draws a larger em box per unit of DXF
text height. It must be positive and finite; the default is `1.0`. This divides
the exported text height (group 40). A value such as `1.364` is an example that
must be measured for the renderer and font, not a universal correction.

```python
scaled = drawing.to_dxf(text_em_scale=1.364)
ezjww.write_dxf("drawing.jww", "drawing.dxf", text_em_scale=1.364)
drawing.plot(text_em_scale=1.364, save_path="drawing.png")
```

Previews apply the width factor to glyphs before rotation and recover the source
em height from `text_em_scale`. They use the locally available font, so they may
differ from an external DXF viewer. Audit, bounding-box and statistics queries
retain their existing defaults.

Block expansion supports nested JWW INSERTs; `max_block_nesting` must be at least
1. Statistics and bounding boxes include hidden entities. TEXT bounds use the
insertion point, so they are not bounds of the rendered glyphs.

## Command line

The Python package installs the `ezjww` command. Each single-file command below
accepts JWW and supported JWC input.

```bash
ezjww info drawing.jww --json
ezjww audit drawing.jwc --json
ezjww bbox drawing.jwc --jwc-coordinates model_millimeters --json
ezjww stats drawing.jww --json
ezjww report drawing.jwc --json
ezjww to-dxf drawing.jwc -o drawing.dxf --report json
ezjww to-dxf drawing.jww -o drawing.dxf --text-em-scale 1.364
ezjww to-dxf-dir drawings -o dxf --recursive
ezjww plot drawing.jwc -o drawing.png
```

Use `ezjww --help` or `ezjww <command> --help` for options. Audit/report commands
support `--fail-on-issues` for automation. Rendering requires `ezjww[plot]`.

Directory conversion finds `.jww` and `.jwc` case-insensitively and preserves
subdirectories. If inputs would share an output name (for example, `a.jww` and
`a.jwc`), it exits with code 2 before writing any converted files.

## Compatibility

JWW parsing supports CP932 and Unicode strings, block definitions, and structured
diagnostics. Some damaged JWW entity lists can be read partially; inspect
[diagnostics](https://github.com/monozukuri-ai/ezjww/blob/main/docs/DIAGNOSTICS.md) before relying on the result.

JWC support covers the `fixed2421_basic_v1` and `fixed2389_basic_v1` document
profiles (two header layouts sharing one record layout). It includes lines,
circles, circular and elliptical arcs, rotated ellipses, CP932 text, points, and
auxiliary points. Settings and attribute bits outside ezjww's reference corpus are
retained and reported as `JWC_*` diagnostics; structurally inconsistent or
malformed JWC files raise an error instead of returning a partial document.
Compatibility with every JWC generation is not established.

JWC output uses millimeters, with the sheet center as the origin and +Y pointing
up. Palette, dash lengths, and fonts use documented conversion defaults; exact
source fonts and character spacing are not reproduced. The source format version
is unknown and returned as `None` in Python or `null` in TypeScript.

## Documentation

- [Documentation index](https://github.com/monozukuri-ai/ezjww/blob/main/docs/README.md)
- [Python and TypeScript APIs for JWW/JWC](https://github.com/monozukuri-ai/ezjww/blob/main/docs/JWC_API.md)
- [JWC support, coordinates, and conversion limits](https://github.com/monozukuri-ai/ezjww/blob/main/docs/JWC_FORMAT.md)
- [Audit codes and error handling](https://github.com/monozukuri-ai/ezjww/blob/main/docs/DIAGNOSTICS.md)
- [JWW signature](https://github.com/monozukuri-ai/ezjww/blob/main/docs/JWW_SIGNATURE.md)
- [Browser example](https://github.com/monozukuri-ai/ezjww/blob/main/packages/ezjww/examples/browser/README.md)

## License

[MIT](https://github.com/monozukuri-ai/ezjww/blob/main/LICENSE)
