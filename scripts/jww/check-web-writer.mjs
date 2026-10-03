// Exercise the generated web-target glue in Node. Browser UI checks are separate.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = new URL("../../", import.meta.url);
const wasmDir = new URL("packages/ezjww/examples/browser/src/wasm/", root);
const source = readFileSync(new URL("ezjww_wasm.js", wasmDir));
// The generated ESM is self-contained; an explicit module avoids its URL-based
// fetch path and importing it as CommonJS from the Node-target npm package.
const web = await import(`data:text/javascript;base64,${source.toString("base64")}`);
web.initSync({ module: readFileSync(new URL("ezjww_wasm_bg.wasm", wasmDir)) });

const doc = web.newJwwDocument();
const fixture = name => readFileSync(new URL(`jww_samples/writer/${name}.jww`, root));
assert.deepEqual(Buffer.from(web.toJwwBytes(doc)), fixture("empty"));
const basic = fixture("basic/basic");
doc.entities = web.readDocument(basic).entities;
assert.deepEqual(Buffer.from(web.toJwwBytes(doc)), basic);
assert.equal(web.readDxfDocument(web.toJwwBytes(doc), false, 32).entities.length, 6);
doc.entities[0].base.pen_style = 257;
assert.throws(() => web.toJwwBytes(doc), e => String(e).includes("base.pen_style"));
assert.throws(() => web.toJwwBytes(web.readDocument(basic)), e => String(e).includes("document"));
console.log(`Web-target WASM writer smoke passed: ${fileURLToPath(wasmDir)}`);
