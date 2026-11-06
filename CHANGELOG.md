# Changelog

## Unreleased

- JWW writing from Python: after `JwwDrawing.stream_entities()` the modelspace
  entities are checked and encoded as they are added instead of being kept as
  dicts, for drawings too large to hold: a line takes about 360 bytes as dicts
  and 49 in the file. `to_jww_bytes()` (or `saveas()`) then writes the file
  once, with the options, block definitions and images as they are at that
  time. Converting DWGs of 1.93 million and 840,000 entities to JWW now peaks
  at 757 MB and 354 MB instead of 1,323 MB and 621 MB. The bytes are those of
  the same drawing without streaming: the pens of RGB colors are still chosen
  among the pens of the whole document and in the order of `to_jww_bytes`
  (block definitions first, also when they are added last), by writing a
  placeholder and the pen over it at the end. An entity the writer refuses is
  left out without raising and listed in `refused_entities`, with the message
  `to_jww_bytes()` would have raised for it. A streamed entity cannot be
  changed after it is added, and `to_dxf`, `source_document` and the analyses
  are not available. The shared decoder gains `writer::input::EntityStream`
  and `StreamFinisher`. (0.7.11.)
- JWW writing from Python: `add_dimension` gives the auxiliary points pen
  style 1, the only one points support; a dimension drawn with another line
  style (`jwwattribs={"pen_style": 2}`) was refused. The value text still takes
  `text_options["jwwattribs"]` over `jwwattribs`. (0.7.11.)
- JWW writing from Python: `new(share_bases=True)` (and `JwwModelspace(...,
  share_bases=True)`) lets the entities added with the same attributes share
  one `base` dict, for large drawings whose entities are not changed after they
  are added. A line was about 600 bytes, 270 of them its own `base`; a drawing
  of 840,000 entities had 76 different ones, and the peak of converting a DWG
  to it fell from 842 MB to 623 MB. The written file is the same. Dimension
  members keep their own `base`. Off by default, so an entity's `base` can
  still be changed without reaching other entities. (0.7.10.)
- JWW writing from Python: `to_jww_bytes` also encodes the input one entity at
  a time, so the decoded entities are not held either; the header with the
  final palette and the entity count go in front of the encoded entities at the
  end. For drawings of 380,000 and 840,000 entities the peak now rises by
  50 MB and 85 MB (0.7.8: 120 MB and 250 MB; 0.7.7: 750 MB and 1.6 GB), and the
  conversion takes 1.5 s and 3.4 s (0.7.7: 1.9 s and 4.1 s). The output and
  every error message are unchanged. `writer::input::DocumentEncoder`, whose
  `finish` returns the JWW bytes, replaces 0.7.8's `DocumentBuilder`. (0.7.9.)
- JWW writing from Python: `to_jww_bytes` (and with it a new drawing's
  `to_jww_bytes`, `saveas`, `source_document` and `to_dxf`) decodes the input
  one list element at a time instead of first turning the whole document into
  one JSON tree. The
  tree took 750 MB and 1.6 GB on top of the caller's dicts for drawings of
  380,000 and 840,000 entities; the peak now rises by 120 MB and 250 MB, and
  the conversion is about 15 % faster. Pens for RGB colors are allocated in the same
  order (block definitions, entities, images), so the output is unchanged; on
  any error the whole document is decoded as before, so every error message
  is the same too. The shared decoder gains `writer::input::DocumentBuilder`
  and `scan_pens` for adapters that convert their input element by element.
  (0.7.8.)
- JWW writing: the path of each input value (`document.entities[i].base.layer`)
  is rendered only for an error message. `to_jww_bytes`, `to_write_document` and
  the writer's color allocation built that string for every value, which was
  most of the time spent turning the input into the write document: for
  drawings of 380,000 and 840,000 entities (converted from 5 MB and 3.6 MB
  DWGs) it took 3.8 s and 7.9 s and now takes 2.1 s and 4.7 s. The output and
  the error messages are unchanged. (0.7.7.)
- JWC reading accepts the framing and value variants of real DOS-era files
  (23 of 23 production files that the reference-corpus rules rejected now read):
  a missing LF at bytes 199/399/599/799, a CSV block padded with spaces and no
  NUL, pool addresses with fewer than four hex digits (`0:10ea`), a write-layer
  table that is not packed `group << 4 | layer`, stale inactive temporary-point
  slots and text references stored as pool-relative offsets are retained and
  reported as `JWC_ATTRIBUTE_UNVERIFIED`; paper codes 8–14 (2A, 3A, 4A, 5A,
  10 m, 50 m, 100 m — `JwcPaper` gained the variants) and a coordinate extent
  other than 518 (`csv0[30]`, the paper width in source units) scale the
  drawing by paper width / extent; arc angles outside [0, 360) are wrapped and
  an end one full turn from the start is a closed curve
  (`JWC_ARC_ANGLE_NORMALIZED`); a flatness above one keeps the stored values
  and the conversion swaps the axes (`JWC_ELLIPSE_AXES_SWAPPED`); line style 0
  is drawn continuous (`JWC_LINE_STYLE_DEFAULTED`). `JWC_MAX_ENTITIES` is
  4,000,000 and the text pool may be 16 MiB (`JWC_MAX_TEXT_POOL_BYTES`); both
  stay bounded by the 64 MiB file limit. (0.7.6.)
- JWS reading follows the MFC class schema of the records when the header
  version is older (Jw_cad 6 writes header 420 with schema 600), reported as
  `JWS_SCHEMA_VERSION_USED`. (0.7.6.)
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
