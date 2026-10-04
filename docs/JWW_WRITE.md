# JWW writing in Python and Rust

The Rust core creates version-700 JWW drawings with lines, circles/ellipses,
arcs, points, text, polygon/circle solids, dimensions and block definitions/references.
It writes paper/layer settings, palette colors, line patterns and text presets.
Coordinates are paper millimeters, with the origin at the paper center and +Y up.
Changing a group's scale does not rescale entity coordinates.

The basic Python/Rust/TypeScript writer shipped in 0.4.0. The extended entities,
tables and helpers documented here are part of **0.5.0**, currently unreleased;
build this revision to use them. Existing-file lossless editing, embedded images, marker/temporary points,
curve-group semantics and older output versions remain outside the writer's scope.
See [the extension guide](WRITER_EXTENSIONS.md) for the R1–R8 request mapping,
examples, native evidence and remaining qualification limits.

## Python API

```python
import ezjww

drawing = ezjww.new(version=700, paper_size=3, memo="図面の説明\r\n")
msp = drawing.modelspace()
line = msp.add_line((0, 0), (100, 0), jwwattribs={"layer": 2, "pen_color": 3})
line["end_y"] = 10  # the returned dict is the editable native entity
msp.add_circle((20, 20), 5)
msp.add_arc((40, 20), 5, start_angle=350, sweep_angle=30)
msp.add_point((60, 20))
msp.add_text("日本語 ABC", (0, -10), (24, -10), spacing=0.5)
drawing.options["layer_groups"][0]["name"] = "平面図"
drawing.options["layer_groups"][0]["scale"] = 50.0

data = drawing.to_jww_bytes()
drawing.saveas("created.jww")
drawing.save_dxf("created.dxf", target_version="AC1024")
print(drawing.stats(), drawing.bbox())
# drawing.plot(save_path="created.png")  # requires ezjww[plot]
```

`new()` and `Drawing.new()` return a `JwwDrawing`, which is a subclass of `Drawing`.
Constructor arguments are keyword-only: `version=700`, `paper_size=3`, `memo=""`.
A new drawing has `source_format == "jww"` and `source_path is None`.

`modelspace()` returns a persistent `JwwModelspace`. Its `entities` list and the
dictionaries returned by the `add_*` methods are mutable. Entity geometry
uses the native field names in the reader's `JwwEntity` schema. Each `jwwattribs`
mapping sets fields in the entity's `base`: `pen_style`, `pen_color`, `pen_width`,
`layer`, `layer_group`, `group`, `flag`. The support limits in the table below
apply. Unrecognized attribute names are rejected.

Python arc/ellipse/block builders take degrees and store native radians.
`add_arc(..., flatness=1.0, tilt_angle=0.0)` and `add_ellipse(...)` support
minor/major radius ratios in `(0, 1]`. `add_text(..., end=None)` can estimate the
baseline endpoint from full/half-width characters, spacing and rotation. Use
`msp.font_width_factors[font_name]` or `font_width_factor=` to calibrate the
estimate. No font engine measurement is performed. Text presets 1–10 supply
omitted sizes, spacing and color from `options["text_presets"]`.

Select a group/layer and add raw dictionaries in bulk:

```python
drawing.select_layer(2, 5)
msp.extend([])  # list of native writer dictionaries
```

Builders use the selected group/layer; `jwwattribs` overrides them.
`header`, `source_document`, `jww_document`, `to_dxf()` and `to_dxf_string()` are
fresh derived snapshots. Change the editable `options` / entity dictionaries
to update the drawing. Conversion, analysis and plotting reflect current edits.
Conversion options belong to `to_dxf()`; the editable modelspace holds native
JWW values. Its `query()` filters native pen numbers and hexadecimal group-layer
IDs such as `msp.query('LINE[layer=="0-2", color==3]')`. Existing `readfile()`
modelspaces continue to expose converted DXF entities and DXF layer/color values.

Rust validates all input on serialization/conversion/analysis, including manual
list/dict edits. Unknown fields, missing fields and unsupported types fail with
field-specific `ValueError`s. Public `new_jww_document()` / `to_jww_bytes()`
accept a `JwwWriteDocument` dictionary containing `options`, `entities` and optional
`block_defs`. Parsed documents require explicit `to_write_document()` conversion
with diagnostics; they are rejected as direct writer inputs.

`saveas()` requires a `.jww` extension; `save_dxf()` requires `.dxf` (case
insensitive). Neither method infers a format from an arbitrary extension. Both
serialize fully before creating a temporary file in the destination directory,
then use atomic replacement. Existing targets survive validation and replacement
failures; temporary files are cleaned up. I/O failures propagate as `OSError`.
The parent directory must already exist. Existing targets are replaced on success.

### Migration from the previous Python API

| Previous call | Current call |
| --- | --- |
| `new()` / `Drawing.new()` for an empty DXF view | `new_dxf()` / `Drawing.new_dxf()` |
| `readfile(...).saveas("out.dxf", ...)` | `readfile(...).save_dxf("out.dxf", ...)` |
| New native JWW creation | `new()` → `modelspace().add_*()` → `saveas("out.jww")` |

See the [0.4 migration guide](MIGRATING_0_4.md) for complete examples.
This is an intentional API change. Update existing DXF `saveas()` call sites.
The legacy empty DXF view retains query/analysis behavior; exporting it still
requires a source-backed drawing. Native `saveas()` on `readfile()` results is
rejected because the parser does not retain all data needed to rewrite that file.
The CLI's `to-dxf` / batch conversion commands continue to produce DXF and use
`save_dxf()` internally.

## TypeScript / WASM API

Build this revision from source (`pnpm install --frozen-lockfile` then
`pnpm run build` in `packages/ezjww`) to use the unreleased 0.5.0 extensions.

```typescript
import { writeFileSync } from "node:fs";
import { newJwwDocument, toJwwBytes, readDxfString } from "ezjww";

const drawing = newJwwDocument();
drawing.options.memo = "図面の説明\r\n";
drawing.options.layer_groups[0].name = "平面図";
drawing.entities.push({
  type: "LINE",
  base: {
    group: 0, pen_style: 1, pen_color: 3, pen_width: 0,
    layer: 2, layer_group: 0, flag: 0,
  },
  start_x: 0, start_y: 0, end_x: 100, end_y: 10,
});
const bytes = toJwwBytes(drawing); // Uint8Array
writeFileSync("created.jww", bytes);
writeFileSync("created.dxf", readDxfString(bytes, { targetVersion: "AC1024" }));
```

`newJwwDocument()` takes no arguments and returns a fresh, mutable
`JwwWriteDocument` containing `options`, `entities` and `block_defs`, with the same native
defaults as Python and Rust. Edit those objects directly; `toJwwBytes()`
validates the current values on every call, does not mutate the input, and
returns an independent byte buffer. Identical inputs produce identical bytes.
There is no filesystem I/O in either function. Node callers choose their own
write/overwrite policy; browser callers can download a `Blob` made from the bytes.

`JwwWriteEntity` is a discriminated union of `JwwWriteLine`, `JwwWriteCircle`,
`JwwWriteArc`, `JwwWritePoint`, `JwwWriteText`, `JwwWriteSolid`,
`JwwWriteCircleSolid`, `JwwWriteDimension` and `JwwWriteBlock`. Geometry fields are required;
the schema matches the Python low-level writer input. `base` uses `EntityBase`.
Options contain exactly 16 groups with 16 layers each. The supported values in
the tables below also apply to JavaScript, including calls directly to WASM.

Arc fields use **radians**, including `start_angle`, `arc_angle` and `tilt_angle`.
`flatness` is in `(0, 1]` and `tilt_angle` rotates the ellipse axes; a `CIRCLE` must have `is_full_circle: true`,
start 0 and sweep `2 * Math.PI`. An `ARC` must have `is_full_circle: false` and
a positive sweep below `2 * Math.PI`. Text `angle` is **degrees**. Plain points
require `is_temporary: false`, `code: 0`, `angle: 0`, `scale: 0`.

Unknown or missing object fields, incorrect types, fractional/out-of-range
integers, non-finite numbers and unsupported features throw field-specific
errors. There is no coercion of numeric strings or booleans. Unpaired JavaScript
UTF-16 surrogates are rejected; valid supplementary characters are preserved.
Errors from the raw WASM interface use the existing string-error convention
(`String(error)` works for both reader and writer failures).

`readDocument()` results have a different shape and are rejected by the writer.
Use `toWriteDocument(parsed, skipUnsupported = false)` for explicit bounded
conversion. Its result contains `document` (or `null`) and `diagnostics`; unsupported
entities require explicit omission. Unexposed settings use template defaults.
There is no general DXF-to-JWW converter. For preview/DXF export, read the new bytes.
Reader dimensions now include each inline member's `base` attributes, and the
header exposes `text_presets`. These are additive schema fields.

The npm entry point targets Node.js. A web-target WASM build exports the same
writer functions after initialization:

```typescript
import initWasm, { newJwwDocument, toJwwBytes } from "./wasm/ezjww_wasm.js";
import type { JwwWriteDocument } from "ezjww";

await initWasm();
const drawing = newJwwDocument() as JwwWriteDocument;
const bytes = toJwwBytes(drawing);
const blob = new Blob([Uint8Array.from(bytes)], { type: "application/octet-stream" });
```

See the [browser example](../packages/ezjww/examples/browser/README.md) for
generation, preview and a JWW download. The raw generated WASM declarations use
`any` for JS objects; the npm TypeScript wrapper supplies the strict writer types.

## Rust API

```rust
use ezjww_core::{to_jww_bytes, Coord2D, JwwWriteDocument};

fn main() -> Result<(), ezjww_core::JwwWriteError> {
    let mut drawing = JwwWriteDocument::default();
    drawing.options.memo = "図面の説明\r\n".into();
    drawing.options.layer_groups[0].name = "平面図".into();
    drawing.options.layer_groups[0].scale = 50.0; // 1:50; coordinates stay in paper mm
    let line = drawing.add_line(Coord2D::new(0.0, 0.0), Coord2D::new(100.0, 0.0));
    line.base.layer = 2;
    line.base.pen_color = 3;
    drawing.add_circle(Coord2D::new(20.0, 20.0), 5.0);
    drawing.add_arc(Coord2D::new(40.0, 20.0), 5.0, 350.0, 30.0);
    drawing.add_point(Coord2D::new(60.0, 20.0));
    let text = drawing.add_text(
        Coord2D::new(0.0, -10.0), Coord2D::new(24.0, -10.0), "日本語 ABC",
    );
    text.size_x = 3.0;
    text.size_y = 3.0;
    text.spacing = 0.5;
    let data = to_jww_bytes(&drawing)?;
    assert!(data.starts_with(b"JwwData."));
    Ok(())
}
```

Builders return the inserted entity for attribute changes. Input is validated
when calling `to_jww_bytes`; the function never writes to the filesystem.
Identical inputs produce identical bytes. The existing `add_line` builder now
also returns a mutable entity reference.

### Settings and attributes

| Field | Supported values |
| --- | --- |
| `version` | 700 |
| `paper_size` | 0..4 (A0..A4), 8..14 (official enlarged paper codes) |
| `write_layer_group`, each group's `write_layer` | 0..15 |
| Layer/group `state` | 0 hidden, 1 display only, 2 editable, 3 current |
| Layer/group `protect` | 0 none, 1 allow display changes, 2 fixed display |
| Group `scale` | Finite positive scale denominator; default 1 |
| Entity `base.layer`, `base.layer_group` | 0..15 |
| Entity `base.pen_color` | 1..9 or 100..356; SOLID/CIRCLE_SOLID use 10 for inline COLORREF |
| Line/arc `base.pen_style` | 1..9, 16..19, 30..62 |
| Line/arc/point `base.pen_width` | Native width code 0..500; 0 uses pen defaults |
| Point/text `base.pen_style` | 1 (ordinary point / default text anchor) |
| Text `base.pen_width` | 0 or the validated dimension flag combinations |
| Entity `base.group` | 0; curve groups are unsupported |
| Entity `base.flag` | 0 or supported dimension flags; see extension guide |
| `palette`, `line_types`, `text_presets` | Optional reader-shaped tables; defaults supplied by `new_jww_document()` |

Only the current group may have state 3, and each group must have exactly one
state-3 layer matching its `write_layer`. Change both the index and states when
selecting another layer; Python's `select_layer()` handles this. Width codes
remain native codes. Color values use Windows COLORREF (`0x00BBGGRR`), matching
the reader. Tables and allocation rules are in the extension guide.

Options default to A3, empty memo, current group/layer 0, all scales 1, and the
native template's names (`0` and `Defpoints` for layers 0 and 1 in group 0;
others empty). Blank names are stored as blank; readers supply display names
such as `GroupF` / `F-F`. Other header settings come from the embedded native
[template](../crates/ezjww-core/src/writer/templates/README.md), without requiring
Jw_cad or a local template file at runtime. The header's variable-length name
sections are rebuilt before copying the remaining settings.

### Enlarged paper dimensions

| Code | Label | Width × height (mm) |
| --- | --- | --- |
| 8 | 2A | 1682 × 1189 |
| 9 | 3A | 2378 × 1682 |
| 10 | 4A | 3364 × 2378 |
| 11 | 5A | 4756 × 3364 |
| 12 | 10m | 10000 × 7073 |
| 13 | 50m | 50000 × 35366 |
| 14 | 100m | 100000 × 70732 |

These are Jw_cad 10.02.1's stored dimensions, including its integer rounding for
10m–100m sheets. Its help describes these labels as screen widths.
[`inspect_paper_sizes.py`](../scripts/jww/inspect_paper_sizes.py) reproduces the
table from a locally installed, hash-checked executable without redistributing it.

### Angles and text

- `add_arc` takes **degrees**: start in [0, 360), positive counterclockwise sweep
  in (0, 360). Crossing zero is allowed. Use `add_circle` for a full circle.
- Direct `Arc` values use **radians**, matching the parser. Full circles/ellipses
  have start 0 and sweep 2*pi. Partial ellipse angles are measured in the rotated
  major/minor-axis coordinate system. Invalid values are rejected without wrapping.
- Text angle is **degrees** in [0, 360). Width/height must be positive; spacing is
  nonnegative. `add_text` defaults to 3 mm width/height, zero spacing/rotation,
  free-size text type 0 and `ＭＳ ゴシック`. Types 0..10 accept style offsets
  10000/20000/30000. The serialization reference's code calls 10000 italic and
  20000 bold (its prose reverses these labels); the raw value is retained.
  Alternative anchors remain unsupported.
- Supply baseline endpoints explicitly or use the Python estimate. There is no font measurement.
  Jw_cad recalculates the endpoint and may choose a matching text preset on save.
- Strings use MFC UTF-16LE, including supplementary characters. The Rust codec
  preserves empty and long strings (including length boundaries around 255 and
  65,534 UTF-16 units). Native application retention is tested separately below.
- Text is single-line: controls including NUL, tabs and newlines are rejected.
  Empty fonts, `^@` image/control prefixes and recognized native internal-setting
  text are rejected. NUL is also rejected in memo and layer/group names.

Errors identify fields, for example `entities[0].radius` or
`options.layer_groups[2].scale`. Non-finite numbers and unsupported entity types
are errors. Ordinary point marker fields must be zero, because those fields are
not stored for ordinary points.

`JwwWriteDocument` is a new-document input type. A parsed `JwwDocument` does not
retain every original setting or image and cannot serve as a lossless editable
source. Python bindings, `_core.pyi` and the TypeScript writer types expose the
new-document contract. The converter explicitly reports unsupported input.

## Reproduce acceptance cases

The example refuses to overwrite existing files:

```sh
cargo run -p ezjww-core --example jww_write -- basic /tmp/basic.jww
cargo run -p ezjww-core --example jww_write -- settings /tmp/settings.jww
cargo run -p ezjww-core --example jww_write -- unicode /tmp/unicode.jww
```

`empty` and `line` reproduce the original PR1 acceptance files byte for byte.
The new input definitions are in `examples/support/writer_cases.rs` under the
core crate. The writer follows the official
[JWW serialization reference](https://www.jwcad.net/jwdatafmt.txt).

## Native Jw_cad validation

The expanded [native compatibility report](JWW_COMPATIBILITY.md) records 21 cases
on Jw_cad 10.02.1 / Wine 9.0, including extended counts/classes, CString boundaries,
pen attributes and rotated text. It documents memo loss on native save, the
baseline rule for native DXF rotation and a Direct2D display problem under Wine.
Windows desktop validation remains unrun.

Fixtures under `jww_samples/writer/` record application/environment information,
hashes and source/output files. `basic/` adds mixed geometry, nondefault layer
settings and Unicode cases. These drawings are original work under the repository
license; application binaries and fonts are not distributed.

Use a disposable Windows desktop or isolated Wine prefix/Xvfb display. Copy
Jw_cad 10.02.1 and its DXF export data into a new runtime directory and generate
the input cases there. Run with Windows Python:

```text
python scripts\jww\native_roundtrip.py --runtime C:\lab --log C:\lab\native.json --cases basic settings unicode --jww-only unicode
```

The script checks the executable hash, refuses existing output files, and requires
other Jw_cad windows to be closed. It temporarily sets the separate JWW/DXF folder
preferences to the runtime directory and restores them on exit. It only controls
its own CAD processes. Each case is opened, saved as `<name>_saved.jww`, closed,
reopened, saved as `<name>_reopened.jww`, exported to `<name>_reopened.dxf`, and
closed normally. Input hashes must stay unchanged. Without `--cases`, it runs
`empty` and `line`.

The `basic` and `settings` cases passed native JWW save/reopen and DXF export.
The `unicode` case passed JWW save/reopen, preserving supplementary characters,
255- and 1,024-character text and long layer names. Empty TEXT is preserved by the
Rust codec but deleted on native save. The native application's DXF export of the
1,024-character case exited before completing its file; use `--jww-only unicode`
for this case. Native DXF export of long text is not qualified by these checks.

Regression tests compare native-saved entities and settings, excluding the
recognized internal setting TEXT records added by Jw_cad. They account for
angle wrapping and native text layout explicitly. Native DXF tags are checked
independently of ezjww's DXF converter. Wine checks do not establish Windows
hardware behavior or font rendering on another system.

## Checks

```sh
python scripts/jww/check_fixtures.py
python scripts/jww/check_compatibility.py
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo fmt --all --check
```

Tests cover class reuse across mixed entities, new classes after PID 32,766,
65,534/65,535/65,536 entities, CString boundaries, invalid input, unchanged
palettes after long layer names, and readback through Python and raw/wrapped WASM.
Normal CI uses the saved native artifacts and does not launch Jw_cad.

The Python writer tests reproduce the five native-qualified Rust input fixtures
byte for byte, including UTF-16 text and all exposed settings. They also exercise
mutation visibility, DXF export, plotting, strict binding validation and atomic
save failures. This reuses the recorded native evidence; it is not a new Windows
or Wine application run. Clean installed-wheel checks create and reopen a JWW
without accessing a source-tree template.

TypeScript tests independently construct the same five fixtures and compare
exact output bytes through both the npm wrapper and raw WASM. Cross-language
checks compare Python/TypeScript writer defaults and serialized inputs. Packed
npm checks compile a consumer of the installed writer declarations and generate,
save and reopen a JWW using the bundled WASM. These checks reuse the recorded
native evidence; they do not run a new Jw_cad application session.

After building the browser example, `node scripts/jww/check-web-writer.mjs`
checks the web-target WASM glue in Node. CI runs this separately from the
Node-target package tests; browser interaction and Jw_cad validation are separate
checks.

## Linux Python native harness

The [Linux/Wine procedure](WRITER_EXTENSIONS.md#linuxwine-native-harness) uses
`wine`, `xdotool` and Xvfb; it does not require Windows Python. Run it in a new
output directory. It creates and controls its own prefix/display and preserves
input hashes, application hashes, screenshots, outputs and partial failure logs.
