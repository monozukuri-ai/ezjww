# ezjww for TypeScript

Read JWW and supported JWC drawings, inspect source records, and export DXF
with a Rust parser compiled to WebAssembly.

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

`DxfOptions` accepts `targetVersion` (`"AC1015"` by default or `"AC1024"`) for
string output, `explodeInserts` (default `false`), `maxBlockNesting` (default `32`,
minimum `1`), and `jwcCoordinates` (`"paper_millimeters"` by default or
`"model_millimeters"`). The JWC coordinate option does not change JWW conversion.

## JWC support

The supported binary document profile is `fixed2421_basic_v1`. It covers lines,
circles, circular arcs, full rotated ellipses, CP932 text, points, and auxiliary
points. Some curve sequences are represented as line segments. Old DOS generations
and other JWC layouts are not covered by a general compatibility guarantee.

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
converted DXF. For JWC, it offers paper/model coordinates and conversion notices.
Invisible JWC layers are hidden; fonts are substituted.
See the [browser guide](https://github.com/monozukuri-ai/ezjww/blob/main/packages/ezjww/examples/browser/README.md).

## License

[MIT](https://github.com/monozukuri-ai/ezjww/blob/main/LICENSE)
