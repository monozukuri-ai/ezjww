# JWW writing: Rust foundation

This first stage creates version-700 JWW files in the Rust core. It supports empty
drawings and ordinary solid lines on layer 0 of group 0, using pen color 1,
pen width 0, and no curve group or attribute flags. Coordinates are paper
millimeters with +Y up. Other entities and attributes return an error.

Python `new()`, `new_dxf()` and native `saveas()` changes are planned for the later
binding stage; this change does not introduce those Python or TypeScript APIs.
Existing-file editing, blocks, dimensions, images and older output versions are
outside this writer's current scope.

## Rust API

```rust
use ezjww_core::{to_jww_bytes, Coord2D, JwwWriteDocument};

fn main() -> Result<(), ezjww_core::JwwWriteError> {
    let mut drawing = JwwWriteDocument::default();
    drawing.add_line(Coord2D::new(0.0, 0.0), Coord2D::new(100.0, 0.0));
    let data = to_jww_bytes(&drawing)?;
    assert!(data.starts_with(b"JwwData."));
    Ok(())
}
```

`JwwWriteOptions` defaults to version 700, A3 (`paper_size = 3`), and an empty
memo. Paper sizes use the official numeric codes (0..4 and 8..14). The memo uses
MFC UTF-16LE CString encoding. Embedded NULs, non-finite coordinates, unsupported
versions and unsupported entity attributes are rejected before serialization.
Errors name the field, such as `entities[0].end_y`.

The output contains a complete default header, the main entity list, an empty
block-definition list and a zero image count. The header is embedded at build
time, so generation needs neither a local template file nor Jw_cad. Its
[provenance and extraction](../crates/ezjww-core/src/writer/templates/README.md)
are recorded separately. Input deterministically defines output bytes.

`JwwWriteDocument` is intentionally a new-document input type. A parsed
`JwwDocument` does not retain all original settings or images and cannot be
passed to this API as if it were a lossless editable source.

Version-700 reading now walks the complete documented header to locate its
entity list, including when that list is empty. Truncated version-700 headers
are rejected. Earlier versions retain their existing class-search behavior.

## Generate the acceptance cases

Use an existing output directory and unused filenames. The example refuses to
overwrite an existing file:

```sh
cargo run -p ezjww-core --example jww_write -- empty /tmp/empty.jww
cargo run -p ezjww-core --example jww_write -- line /tmp/line.jww
```

## Native Jw_cad validation

The fixtures under `jww_samples/writer/` record the application version, hashes,
environment and exact source/output files for the empty drawing and 100 mm line.
These are original drawings under the repository license. Application binaries
and fonts are not distributed. Wine verification does not establish Windows
hardware validation.

To repeat the check, use a disposable Windows desktop or a separate Wine prefix
and Xvfb display. Copy Jw_cad 10.02.1 into a new runtime directory, including its
DXF export data. The script checks the executable hash before launching it.
Generate `empty.jww` and `line.jww` into that runtime directory, then run with a
Windows Python interpreter:

```text
python scripts\jww\native_roundtrip.py --runtime C:\lab --log C:\lab\native.json
```

For each input the script opens it, saves `<name>_saved.jww`, closes the process,
reopens the saved JWW, saves `<name>_reopened.jww` and `<name>_reopened.dxf`, and
checks normal process exit. It records hashes and confirms the input was not
modified. Existing outputs are rejected before launching any CAD process.

The automation establishes successful open/save/reopen, not geometry equality
by itself. Rust regression tests compare the native-saved geometry with the
writer inputs, excluding Jw_cad's recognized internal setting TEXT records.
The native DXF line endpoints are independently checked in the fixture manifest.

## Checks

```sh
python scripts/jww/check_fixtures.py
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo fmt --all --check
```

Coverage includes empty drawings, line/class reuse, 65,534/65,535/65,536 entities,
long UTF-16 strings and supplementary characters, late class references above
PID 32,766, format limits, false class tags in header settings, truncated headers
and invalid writer input. Native tests use saved artifacts; they do not launch
Jw_cad in normal CI.
