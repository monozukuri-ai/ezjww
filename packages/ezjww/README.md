# ezjww for TypeScript

Read JWW and supported JWC drawings, create new JWW drawings, and export DXF
with a Rust parser compiled to WebAssembly.

The basic writer API is available in **0.4.0**. This checkout adds unreleased
SXF color/line tables, solids, ellipses, dimensions, blocks and convenience APIs;
build from source to use these extensions.

## Installation

Node.js 18 or later:

```bash
npm install ezjww
```

## Read and convert a drawing

```typescript
import { readFileSync, writeFileSync } from "node:fs";
import { detectFileFormat, readCadDocument, readDxfDocument, readDxfString } from "ezjww";

const data = readFileSync("drawing.jwc"); // also accepts JWW
console.log(detectFileFormat(data));     // "jww", "jwc", or null

const cad = readCadDocument(data);
if (cad.format === "jwc") {
  console.log(cad.document.header.paper, cad.document.header.source_version);
} else {
  console.log(cad.document.header.version);
}

const options = { jwcCoordinates: "model_millimeters" as const };
const dxf = readDxfDocument(data, options);
console.log(dxf.entities.length, dxf.jwc_conversion_report?.notices);
writeFileSync("drawing.dxf", readDxfString(data, {
  ...options,
  targetVersion: "AC1024",
}));
```

All readers accept `Uint8Array`, `ArrayBuffer`, and `ArrayBufferView`, respecting
view offsets and lengths. Detection uses file contents, not the filename.
A matching signature does not validate the complete file.

| Purpose | API |
| --- | --- |
| Detect either format | `detectFileFormat` |
| Read either source document | `readCadDocument` |
| Read JWW only | `isJwwFile`, `readHeader`, `readDocument` |
| Read JWC only | `isJwcFile`, `readJwcHeader`, `readJwcDocument` |
| Convert either format | `readDxfDocument`, `readDxfString` (`toDxfString` alias) |
| Create a new JWW | `newJwwDocument`, `toJwwBytes` |

`DxfOptions` accepts `targetVersion` (`"AC1015"` by default or `"AC1024"`) for
string output, `explodeInserts` (default `false`), `maxBlockNesting` (default `32`,
minimum `1`), and `jwcCoordinates` (`"paper_millimeters"` by default or
`"model_millimeters"`). The JWC coordinate option does not change JWW conversion.

## Create a JWW drawing

Build this revision from source with `pnpm install --frozen-lockfile` and
`pnpm run build` in `packages/ezjww` to use the writer API:

```typescript
import { writeFileSync } from "node:fs";
import { newJwwDocument, toJwwBytes } from "ezjww";

const drawing = newJwwDocument();
drawing.options.memo = "平面図\r\n";
drawing.entities.push({
  type: "LINE",
  base: {
    group: 0, pen_style: 1, pen_color: 1, pen_width: 0,
    layer: 0, layer_group: 0, flag: 0,
  },
  start_x: 0, start_y: 0, end_x: 100, end_y: 0,
});
writeFileSync("created.jww", toJwwBytes(drawing));
```

The editable `JwwWriteDocument` has `options`, `entities` and `block_defs`; parsed
`JwwDocument` objects cannot be passed to the writer. Output is version 700,
supporting lines, circles/ellipses, arcs, points, text, solids, dimensions and
blocks, with editable palette/line/preset tables. `toWriteDocument()` explicitly
converts a parsed document and reports unsupported values. Coordinates are paper
mm. Native arc fields use radians;
text angles use degrees. Each write validates current values and returns an
independent `Uint8Array` without filesystem I/O. Unknown/missing fields, invalid
numbers and unsupported features throw errors, including through raw WASM.

See the [writer API and limits](https://github.com/monozukuri-ai/ezjww/blob/main/docs/JWW_WRITE.md)
for all required fields, settings and the browser API. These APIs share the
Python/Rust writer input contract and deterministic output.

## JWC support

The supported binary document profiles are `fixed2421_basic_v1` and
`fixed2389_basic_v1` (two header layouts, one record layout). They cover lines,
circles, circular and elliptical arcs, rotated ellipses, CP932 text, points, and
auxiliary points. Some curve sequences are represented as line segments. Settings
and attribute bits outside the reference corpus are retained and reported as
`JWC_*` diagnostics. Other JWC layouts are not covered by a general compatibility
guarantee.

The source document retains original coordinates, names, attributes, and byte
spans. `header.source_version` is currently `null`. DXF defaults to paper
millimeters centered on the sheet with +Y up; `model_millimeters` applies each
entity's own layer-group scale once.

DXF output preserves millimeter units and TEXT width factors. Rendering defaults
for the palette, dashes, and font, along with text-spacing limits, are reported in
`jwc_conversion_report.notices`. Unknown layouts or attributes, partial ellipses,
and inconsistent string pools throw errors with byte offsets. Malformed JWC does
not return a partial document; CP932 replacement decoding produces diagnostics.

See the [API reference](https://github.com/monozukuri-ai/ezjww/blob/main/docs/JWC_API.md),
[JWC support and limits](https://github.com/monozukuri-ai/ezjww/blob/main/docs/JWC_FORMAT.md),
and [diagnostic codes](https://github.com/monozukuri-ai/ezjww/blob/main/docs/DIAGNOSTICS.md).
Repository documentation describes the source version; use documentation at your
installed package's release tag when working with an older release.

## Browser example

The npm entry point targets Node.js. The repository includes a browser example
that builds the Rust WASM module for the web and parses selected files locally.
For the browser build, use Node.js 22.12+ (or 20.19+ on the 20.x line), Rust, and
pnpm. From a checkout:

```bash
cd packages/ezjww
pnpm install
pnpm run example:browser:dev
```

The app displays layers, source JSON, diagnostics, and previews, and can download
converted DXF. “JWW作成例” creates a drawing with all five supported entity types;
“生成JWW保存” downloads that new drawing. For JWC, it offers paper/model coordinates
and conversion notices.
Invisible JWC layers are hidden; fonts are substituted.
See the [browser guide](https://github.com/monozukuri-ai/ezjww/blob/main/packages/ezjww/examples/browser/README.md).

## License

[MIT](https://github.com/monozukuri-ai/ezjww/blob/main/LICENSE)
