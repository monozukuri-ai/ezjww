# ezjww

TypeScript bindings for the Rust `ezjww` JWW parser and DXF converter.

```ts
import { readFileSync } from "node:fs";
import { readDocument, toDxfString } from "ezjww";

const data = readFileSync("sample.jww");
const document = readDocument(data);
const dxf = toDxfString(data, { explodeInserts: true });
```

## Browser example

```bash
cd packages/ezjww
pnpm install
pnpm run example:browser:dev
```

The example app parses a dropped `.jww` file in the browser and shows summary
counts, layers, a simple drawing preview, and JSON output.
