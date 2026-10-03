# JWW writing in Rust

The Rust core creates new version-700 JWW drawings containing lines, circles,
circular arcs, ordinary permanent points and plain text. It also writes paper
size, memo, layer/group names, states, protection and scale denominators.
Coordinates are paper millimeters, with the origin at the paper center and +Y up.
Changing a group's scale does not multiply or divide entity coordinates.

Python `new()`, `new_dxf()` and native `saveas()` changes belong to the later
binding stage. This API is currently Rust only. Existing-file editing, ellipses,
solids, blocks, dimensions, images, marker/temporary points and older output
versions remain outside the writer's scope.

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
| Entity `base.pen_color` | Basic colors 1..9 |
| Line/arc `base.pen_style` | Basic styles 1..9 |
| Line/arc/point `base.pen_width` | Native width code 0..500; 0 uses pen defaults |
| Point/text `base.pen_style` | 1 (ordinary point / default text anchor) |
| Text `base.pen_width` | 0; this field stores dimension flags for text |
| Entity `base.group`, `base.flag` | 0 |

Only the current group may have state 3, and each group must have exactly one
state-3 layer matching its `write_layer`. Change both the index and states when
selecting another layer. Basic pen codes use the template's palette, line patterns
and print settings; width codes are not a new physical-width API. SXF/custom
colors, custom line patterns and arbitrary flags are rejected.

Options default to A3, empty memo, current group/layer 0, all scales 1, and the
native template's names (`0` and `Defpoints` for layers 0 and 1 in group 0;
others empty). Blank names are stored as blank; readers supply display names
such as `GroupF` / `F-F`. Other header settings come from the embedded native
[template](../crates/ezjww-core/src/writer/templates/README.md), without requiring
Jw_cad or a local template file at runtime. The header's variable-length name
sections are rebuilt before copying the remaining settings.

### Angles and text

- `add_arc` takes **degrees**: start in [0, 360), positive counterclockwise sweep
  in (0, 360). Crossing zero is allowed. Use `add_circle` for a full circle.
- Direct `Arc` values use **radians**, matching the parser: circular flatness 1,
  tilt 0, and full circles have start 0 and sweep 2*pi. Invalid values are rejected
  without wrapping angles or converting ellipses.
- Text angle is **degrees** in [0, 360). Width/height must be positive; spacing is
  nonnegative. `add_text` defaults to 3 mm width/height, zero spacing/rotation,
  free-size text type 0 and `ＭＳ ゴシック`. Plain types 0..10 are accepted; italic,
  bold, dimension and alternative anchor encodings are deferred.
- Supply text baseline endpoints explicitly. The writer does no font measurement.
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
source. Public Python and TypeScript bindings and stubs are unchanged.

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
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo fmt --all --check
```

Tests cover class reuse across mixed entities, new classes after PID 32,766,
65,534/65,535/65,536 entities, CString boundaries, invalid input, unchanged
palettes after long layer names, and readback through Python and raw/wrapped WASM.
Normal CI uses the saved native artifacts and does not launch Jw_cad.
