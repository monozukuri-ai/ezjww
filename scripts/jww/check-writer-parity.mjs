// Compare the exact new-document input contract and output bytes with Python.
// Tests separately construct the five acceptance cases in Rust/Python/TS.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

export function checkWriterParity({ root, api, python }) {
  const code = String.raw`
import hashlib, json, sys
from pathlib import Path
from ezjww import _core, read_document

root = Path(sys.argv[1])
cases = {}
for name in ('empty', 'line', 'basic', 'settings', 'unicode'):
    path = root / 'jww_samples' / 'writer'
    if name not in ('empty', 'line'):
        path /= 'basic'
    path /= name + '.jww'
    parsed = read_document(str(path))
    doc = _core.new_jww_document()
    doc['entities'] = parsed['entities']
    if name == 'settings':
        doc['options'] = {k: parsed['header'][k] for k in doc['options']}
    elif name == 'unicode':
        doc['options']['memo'] = '日本語𠮷\r\n'
        doc['options']['layer_groups'][0]['name'] = '図面𠮷'
        doc['options']['layer_groups'][0]['layers'][2]['name'] = '日本語の層' * 60
    output = _core.to_jww_bytes(doc)
    assert output == path.read_bytes(), name
    cases[name] = {'input': doc, 'sha256': hashlib.sha256(output).hexdigest()}
print(json.dumps({'defaults': _core.new_jww_document(), 'cases': cases}, ensure_ascii=True))
`;
  const expected = JSON.parse(execFileSync(python, ["-I", "-X", "utf8", "-c", code, root], {
    cwd: root, encoding: "utf8", maxBuffer: 4 * 1024 * 1024,
    timeout: 60_000, stdio: ["ignore", "pipe", "inherit"],
  }));
  assert.deepEqual(api.newJwwDocument(), expected.defaults);
  const hashes = {};
  for (const [name, value] of Object.entries(expected.cases)) {
    const bytes = api.toJwwBytes(value.input);
    assert.ok(bytes instanceof Uint8Array);
    const sha256 = createHash("sha256").update(bytes).digest("hex");
    assert.equal(sha256, value.sha256, name);
    const folder = ["empty", "line"].includes(name) ? "" : "basic";
    assert.deepEqual(Buffer.from(bytes), readFileSync(resolve(root, "jww_samples/writer", folder, `${name}.jww`)));
    hashes[name] = sha256;
  }
  return { inputs: Object.keys(hashes).length, sha256: hashes };
}
