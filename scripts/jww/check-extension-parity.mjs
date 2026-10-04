// Shared public writer inputs, exact bytes, reader values and conversion diagnostics.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";

export function checkWriterExtensions({ root, api, python }) {
  const code = String.raw`
import hashlib, json, runpy, sys
import ezjww
cases = runpy.run_path(sys.argv[1] + '/scripts/jww/make_extension_cases.py')['cases']()
result = {}
for name, drawing in cases.items():
    doc = drawing.modelspace()._input()
    parsed = drawing.source_document
    # The Python reader additionally exposes derived standard/double dash runs.
    for section in ('standard', 'double_length'):
        for pattern in parsed['header']['line_types'][section]:
            pattern.pop('runs', None)
            pattern.pop('segments_mm', None)
    # Python also supplies a convenience name on each block reference; the
    # shared schema stores that mapping in block_def_names.
    for entities in [parsed['entities'], *(b['entities'] for b in parsed['block_defs'])]:
        for entity in entities:
            entity.pop('block_name', None)
    result[name] = dict(input=doc, sha256=hashlib.sha256(ezjww.to_jww_bytes(doc)).hexdigest(),
                       parsed=parsed, converted=ezjww.to_write_document(parsed))
print(json.dumps(result, ensure_ascii=True))
`;
  const cases = JSON.parse(execFileSync(python, ["-I", "-X", "utf8", "-c", code, root], {
    encoding: "utf8", maxBuffer: 8 * 1024 * 1024, timeout: 60_000,
    stdio: ["ignore", "pipe", "inherit"],
  }));
  const hashes = {};
  for (const [name, expected] of Object.entries(cases)) {
    const input = structuredClone(expected.input);
    const bytes = api.toJwwBytes(input);
    assert.deepEqual(input, expected.input, `${name}: input mutated`);
    const hash = createHash("sha256").update(bytes).digest("hex");
    assert.equal(hash, expected.sha256, `${name}: bytes`);
    const parsed = api.readDocument(bytes);
    assert.deepEqual(parsed, expected.parsed, `${name}: reader`);
    assert.deepEqual(api.toWriteDocument(parsed), expected.converted, `${name}: conversion`);
    hashes[name] = hash;
  }
  return { inputs: Object.keys(cases).length, sha256: hashes };
}
