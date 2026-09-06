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

JWC support is restricted to the `fixed2421_basic_v1` document profile. It includes
lines, circles, circular arcs, full rotated ellipses, CP932 text, points, and
auxiliary points. Partial ellipses and unknown layouts or attributes are rejected.
Malformed JWC files raise an error instead of returning a partial document.
Compatibility with all JWC generations, including old DOS files, is not established.

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
