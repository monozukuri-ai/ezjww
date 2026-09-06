# ezjww browser example

Open a JWW or supported JWC drawing, inspect its source data, preview it, and
save a converted DXF. Files are parsed locally in WebAssembly.

## Run the example

Use Node.js 22.12+ (or 20.19+ on the 20.x line), Rust, and pnpm. From a repository
checkout:

```bash
cd packages/ezjww
pnpm install
pnpm run example:browser:dev
```

Open the URL printed by Vite. Load the bundled JWW sample, or choose or drop your
own `.jww` or `.jwc` file. For JWC, choose paper or model millimeters and review
the conversion notices before downloading the DXF.

## Build and preview

From `packages/ezjww`:

```bash
pnpm run example:browser:build
pnpm run example:browser:preview
```

## Display limits

JWC support is limited to the [supported binary profile](../../../../docs/JWC_FORMAT.md).
Invisible JWC layers are hidden. Text uses substitute fonts; exact source glyphs
and character spacing are not reproduced. The source JWC version is unknown.

See the [API reference](../../../../docs/JWC_API.md) for coordinate options,
source records, and diagnostics.
