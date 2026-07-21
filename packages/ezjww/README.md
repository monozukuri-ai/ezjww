# ezjww

TypeScript bindings for the Rust `ezjww` JWW parser and DXF converter.

```ts
import { readFileSync } from "node:fs";
import { readDocument, toDxfString } from "ezjww";

const data = readFileSync("sample.jww");
const document = readDocument(data);
for (const diagnostic of document.diagnostics) {
  console.log(diagnostic.code, diagnostic.details);
}
const dxf = toDxfString(data, { explodeInserts: true });
```

`isJwwFile(data)` checks the exact eight-byte `JwwData.` signature at offset 0;
it does not use a filename extension. `readDocument(data).diagnostics` reports
structured CP932 replacement events, including absolute byte offsets.

The repository documents the stable audit issue-code catalog in
`docs/DIAGNOSTICS.md` and the signature contract in `docs/JWW_SIGNATURE.md`.

## Browser example

```bash
cd packages/ezjww
pnpm install
pnpm run example:browser:dev
```

The example app parses a dropped `.jww` file in the browser and shows summary
counts, layers, a simple drawing preview, and JSON output.
