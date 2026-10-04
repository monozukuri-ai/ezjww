# Changelog

## 0.5.0 — unreleased

### Added

- SXF colors (100–356), per-entity COLORREF allocation, editable palette,
  standard/double/SXF line patterns and text presets.
- Polygon/circle solids, tilted ellipses/arcs, native dimensions and nested blocks.
- Python layer selection, bulk insertion, estimated text endpoints, per-font
  factors, multiline text and public dictionary-to-bytes entrypoints.
- Explicit reader-to-writer conversion with per-entity diagnostics in Python
  and TypeScript/WASM. Dimension reader payloads now retain child base attributes.
- Linux Python/Wine native validation harness and documented enlarged paper sizes.

See [the extension guide](docs/WRITER_EXTENSIONS.md) for supported values and
native qualification limits. Output remains version 700; there is no lossless
existing-file rewrite or version-600 writer.

## 0.4.0

### Added

- Create new version-700 JWW drawings through Python, Rust and TypeScript/WASM:
  lines, circles, circular arcs, ordinary points, plain text, paper settings,
  layer/group settings and basic pen attributes.
- Strict validation, deterministic JWW bytes, Python file saving and in-memory
  DXF export, analysis and preview. The native header template is embedded.
- A 21-case native compatibility corpus for Jw_cad 10.02.1 on Wine 9.0,
  including large counts, extended class references and long strings.
- Verification of installed wheels and npm archives against a native-qualified
  mixed drawing; an external sdist build followed by clean wheel installation.

### Fixed

- Preserve the complete Cargo workspace in the sdist so builds can use its
  bundled `Cargo.lock` with `--locked`.

### Breaking Python changes

- `new()` / `Drawing.new()` create writable JWW drawings. Use `new_dxf()` /
  `Drawing.new_dxf()` for the previous empty DXF view.
- `saveas()` saves new JWW drawings. Replace DXF calls with `save_dxf()`.
  Existing-file JWW rewriting is unsupported and raises an error.

See the [0.4 migration guide](docs/MIGRATING_0_4.md).

### Compatibility limits

Native saving may truncate/reflow memo text, remove empty TEXT and recalculate
text endpoints. Native DXF rotation follows the text baseline. Wine Direct2D
display has known failures; Windows desktop and printing have not been qualified.
See [the recorded evidence](docs/JWW_COMPATIBILITY.md). Existing JWW/JWC reading
and DXF conversion remain covered by the regression and package checks.
