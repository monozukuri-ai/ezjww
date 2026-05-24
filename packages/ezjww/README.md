# ezjww

TypeScript bindings for the Rust `ezjww` JWW parser and DXF converter.

```ts
import { readFileSync } from "node:fs";
import { readDocument, toDxfString } from "ezjww";

const data = readFileSync("sample.jww");
const document = readDocument(data);
const dxf = toDxfString(data, { explodeInserts: true });
```
