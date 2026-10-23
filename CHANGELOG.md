# Changelog

## Unreleased

- DXF conversion gives every font the texts use a text style of its own that
  names the font *file* AutoCAD looks for (`ＭＳ ゴシック` -> `msgothic.ttc`,
  `HGｺﾞｼｯｸM` -> `HGRGM.TTC`) and keeps the TrueType family in the style's
  `ACAD` xdata, with an `APPID` table registering `ACAD`. Before, every TEXT
  used `STANDARD` with `txt` and no big font, so AutoCAD drew Japanese text as
  `?`. An unknown face keeps its name as the family and gets a file that draws
  its script (`msgothic.ttc` for Japanese); texts without a font stay on
  `STANDARD`. The table and the rules follow `cad2d_ir.fonts`, so the direct
  conversion and the IR path name the same files. (0.7.5.)
- DXF conversion writes an entity whose own pen width is 0 (most Jw_cad
  entities) with its pen color's printed width from `header.pens`
  (`JwwPens::printer_width_mm`), snapped to the nearest DXF lineweight
  (group 370). Before, such entities were written at the default lineweight
  (-3) and the widths Jw_cad prints with were lost. An entity's own pen width
  still wins; JWC and JWS conversions, which carry no pen table, are unchanged.
  (`ConversionView::pens`, 0.7.4.)
- Detect JWW/JWS files whose ANSI strings are UTF-8 (Jw_cad under Wine with a
  UTF-8 locale) and read them as UTF-8 instead of CP932: the CP932 pass collects
  `EncodingEvidence` (every non-ASCII string well-formed UTF-8, plus at least
  three multi-byte sequences or one CP932 anomaly), then the file is parsed
  again with `TextEncoding::Utf8`. The result carries `text_encoding`
  (`ParsedJwwDocument`, `JwsDocument`, the DTOs, the Python document dict and
  `Drawing.text_encoding`) and a leading `TEXT_ENCODING_DETECTED` diagnostic;
  a name cut inside a UTF-8 sequence by Jw_cad's fixed buffer is trimmed and
  reported as `CP932_DECODE_REPLACED` with `details.encoding = "utf-8"`. Font
  names and SXF line type / extended color names (`Reader::read_system_cstring_with_context`)
  come from Jw_cad's own CP932 string table even under Wine, so they do not
  count as evidence and fall back to CP932 when they are not well-formed UTF-8.
- `Drawing.add_block(..., copy_entities=False)` registers the given entity list
  without deep-copying it, for callers that build the list for that call. The
  default still copies. (253 blocks of a DWG export spent 5 s in `deepcopy`.)
- Read the pen widths of a JWW header (`header.pens` / `JwwPens`): the screen
  width in dots and the printer color, width and point radius of pen colors
  0..=9 and of the extended colors 100..=356, with the unit the widths are in
  (`max_draw_width` < 0 means "線幅を1/100mm単位とする", otherwise printer dots
  with the recorded `printer_dpi`). `JwwPens::printer_width_mm` converts a pen
  color's width to millimetres. Jw_cad prints an entity with `pen_width` 0 at
  its pen color's width, so this is the line width of most drawings.
- Write the pen widths: `options.pens` (`JwwWriteOptions::pens`, supplied by
  `new_jww_document()` from the template and kept by `to_write_document`)
  patches the screen widths, the printer pens, the unit flag and the dpi flag
  of the written header. Without it the template's Jw_cad defaults remain:
  dots at 300 dpi, which also makes an entity `pen_width` a count of dots.
- Read JWS registered symbols (versions 351, 420, 600, 700) through Rust,
  Python and TypeScript/WASM. Add dedicated header/document APIs and integrate
  signature detection, `readfile`, analysis, preview, DXF and CLI commands.
  Source coordinates, insertion origin and group scales are preserved;
  conversion reports make default drawing settings explicit.
- Add original native JWS fixtures, strict truncation/error checks and
  Python/WASM parity coverage. Image references are retained without claiming
  embedded JWS bitmap support. See [JWS support](docs/JWS_FORMAT.md).

## 0.6.0 — unreleased

### Added

- Embedded images (version 700). The reader returns the image archive as
  `images` (name, bytes, `compressed`), parses `^@BM` placement texts into
  `image` (`path`, `file_name`, `width`, `height`, `extra`) and reports a
  partially readable archive with `IMAGE_LIST_TRUNCATED`. The writer embeds
  files (`JwwDrawing.embed_image`, Rust `add_image`, low-level `images`),
  places them (`modelspace().add_image`, Rust `add_image_reference`) and
  `to_write_document` copies the archive. A `%temp%` placement must name an
  embedded image; external image links are accepted; other `^@` control texts
  are still rejected. Helpers: `image_reference`, `image_reference_content`,
  `compress_image`, `decompress_image` (Python); `imageReference`,
  `imageReferenceContent` (TypeScript).

## 0.5.0

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
