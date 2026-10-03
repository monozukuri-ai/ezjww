# Migrating to ezjww 0.4

Version 0.4 adds JWW creation and changes Python's `new()` and `saveas()` APIs.
The previous behavior is available through explicit DXF names. These are breaking
changes; there is no deprecated `saveas("file.dxf")` forwarding alias.

| In 0.3.x | In 0.4 |
| --- | --- |
| `ezjww.new()` or `Drawing.new()` for an empty DXF view | `ezjww.new_dxf()` or `Drawing.new_dxf()` |
| `drawing.saveas("out.dxf", ...)` | `drawing.save_dxf("out.dxf", ...)` |
| No native creation API | `ezjww.new()` → `modelspace().add_*()` → `saveas("out.jww")` |

## Existing conversion code

```python
import ezjww

drawing = ezjww.readfile("input.jww")  # also accepts supported JWC
drawing.save_dxf("output.dxf", target_version="AC1024")
```

Keep the existing DXF options when renaming the method. The `to-dxf` and
`to-dxf-dir` CLI commands still export DXF. Reader, query, plotting and analysis
entry points keep their meanings. `new_dxf()` creates the old empty view; its
DXF export still requires a source-backed drawing.

## New JWW drawings

```python
import ezjww

drawing = ezjww.new(version=700, paper_size=3)  # A3; keyword-only settings
msp = drawing.modelspace()
msp.add_line((0, 0), (100, 0))
msp.add_circle((20, 20), 5)
msp.add_arc((40, 20), 5, start_angle=350, sweep_angle=30)
msp.add_point((60, 20))
msp.add_text("日本語", (0, -10), (9, -10))
drawing.saveas("created.jww")
drawing.save_dxf("created.dxf")
```

`new()` returns `JwwDrawing`, a `Drawing` subclass. Its modelspace contains
editable native entity dictionaries. Modify those dictionaries or `drawing.options`
to change subsequent saves, conversions and analysis results.

- `saveas()` requires `.jww`; `save_dxf()` requires `.dxf`, case-insensitively.
  The parent directory must exist. Validation happens before replacing a file.
- `readfile(...).saveas("copy.jww")` raises an error. Existing JWW editing and
  lossless rewriting are not supported.
- Coordinates are paper millimeters, centered on the sheet with +Y up. Group
  scale denominators do not rescale the supplied coordinates.
- Python/Rust arc builders take degrees; the low-level/native arc schema uses
  radians. Text `angle` uses degrees. Supply text baseline endpoints explicitly
  and align them with the text angle when native DXF export matters.
- Supported output is version 700: lines, circles, circular arcs, ordinary
  permanent points, plain text and the documented paper/layer/pen settings.
  Unsupported data is rejected rather than silently omitted.

## TypeScript / WASM

Reader APIs are unchanged. Use `newJwwDocument()` to get mutable `options` and
`entities`, then `toJwwBytes()` to create an independent `Uint8Array`. Choose
filesystem/download behavior in the calling application. A `readDocument()`
result is not a writer input. The npm entry point targets Node.js; web-target
WASM is built by the repository's browser example.

See [the writer reference](JWW_WRITE.md) for both language examples, defaults and
validation. The [native compatibility report](JWW_COMPATIBILITY.md) records
Jw_cad 10.02.1 / Wine 9.0 results, memo truncation and text-layout changes.
Windows desktop behavior, exact fonts and printing remain unqualified.
