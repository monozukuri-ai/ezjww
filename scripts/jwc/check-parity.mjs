// Required corpus parity, including full source values and pinned P4 DXF outputs.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const sha = value => createHash("sha256").update(value).digest("hex");
const errors = {
  r011: "unsupported JWC layout at byte 2441 (line.flags): unverified flags or curve-marker sequence",
  r080: "unsupported JWC layout at byte 2483 (text.string_reference): unexplained gap in string pool",
};

export function checkJwcParity({ root, api, python }) {
  const manifest = JSON.parse(readFileSync(resolve(root, "jwc_samples/manifest.json"), "utf8"));
  assert.equal(manifest.samples.length, 80, "the complete JWC corpus is required");
  const cases = manifest.samples.map(sample => ({
    name: sample.id,
    id: `jwc_samples/${sample.files.jwc.path}`,
    path: resolve(root, "jwc_samples", sample.files.jwc.path),
    sha256: sample.files.jwc.sha256,
  }));
  assert.equal(new Set(cases.map(c => c.id)).size, 80, "duplicate relative sample ID");
  for (const sample of cases) assert.equal(sha(readFileSync(sample.path)), sample.sha256, sample.id);
  const p4 = JSON.parse(readFileSync(resolve(root, "jwc_samples/expected_dxf.json"), "utf8"));
  const code = String.raw`
import hashlib, json, sys
import ezjww

def sha(text): return hashlib.sha256(text.encode("utf-8")).hexdigest()
out = {}
for sample in json.loads(sys.stdin.readline()):
    path = sample["path"]
    row = {"format": ezjww.detect_file_format(path)}
    try: row["header"] = ezjww.read_jwc_header(path)
    except ValueError as error: row["header_error"] = str(error)
    try: row["cad"] = ezjww.read_cad_document(path)
    except ValueError as error:
        row["error"] = str(error)
        for read in [ezjww.read_jwc_document, ezjww.read_dxf_document, ezjww.read_dxf_string, ezjww.readfile]:
            try: read(path)
            except ValueError as other: assert str(other) == row["error"]
            else: raise AssertionError("unexpectedly accepted " + sample["id"])
    else:
        assert row["cad"] == {"format": "jwc", "document": ezjww.read_jwc_document(path)}
        row["conversions"] = []
        for space in ["paper_millimeters", "model_millimeters"]:
            dxf = ezjww.read_dxf_document(path, jwc_coordinates=space)
            assert dxf == ezjww.read_dxf_document(path, True, jwc_coordinates=space)
            hashes = {}
            for target in ["AC1015", "AC1024"]:
                text = ezjww.read_dxf_string(path, target_version=target, jwc_coordinates=space)
                assert text == ezjww.read_dxf_string(path, True, target_version=target, jwc_coordinates=space)
                hashes[target] = sha(text)
            row["conversions"].append({"space": space, "dxf": dxf, "hashes": hashes})
    out[sample["id"]] = row
print(json.dumps({"python_module": ezjww.__file__, "cases": out}, ensure_ascii=False))
`;
  const expected = JSON.parse(execFileSync(python, ["-I", "-X", "utf8", "-c", code], {
    input: JSON.stringify(cases) + "\n", cwd: root, encoding: "utf8", maxBuffer: 128 * 1024 * 1024,
    timeout: 60_000,
  }));
  const evidence = [];
  let dxfHashes = 0;
  for (const sample of cases) {
    const input = readFileSync(sample.path);
    const py = expected.cases[sample.id];
    assert.equal(api.detectFileFormat(input), "jwc", sample.id);
    assert.equal(py.format, "jwc");
    if (sample.name === "r080") {
      assert.equal(py.header_error, errors.r080);
      assert.throws(() => api.readJwcHeader(input), e => String(e) === errors.r080);
    } else {
      assert.ok(py.header, sample.id);
      assert.deepEqual(api.readJwcHeader(input), py.header, sample.id);
    }
    if (errors[sample.name]) {
      assert.equal(py.error, errors[sample.name], sample.id);
      for (const read of [api.readJwcDocument, api.readCadDocument, api.readDxfDocument, api.readDxfString]) {
        assert.throws(() => read(input), e => String(e) === py.error, sample.id);
      }
      evidence.push({ id: sample.id, input_sha256: sample.sha256, expected_error: py.error });
      continue;
    }
    assert.equal(py.error, undefined, sample.id);
    assert.deepEqual(api.readCadDocument(input), py.cad, sample.id);
    assert.deepEqual(api.readJwcDocument(input), py.cad.document, sample.id);
    const conversions = [];
    for (const { space, dxf, hashes } of py.conversions) {
      const opts = { jwcCoordinates: space };
      assert.deepEqual(api.readDxfDocument(input, opts), dxf, `${sample.id} ${space}`);
      assert.deepEqual(api.readDxfDocument(input, { ...opts, explodeInserts: true }), dxf);
      const reference = p4.cases.find(c => c.id === sample.name && `${c.space}_millimeters` === space);
      // P4 calls paper coordinates "paper"; model coordinates "model".
      assert.ok(reference, `${sample.id} ${space}: missing P4 reference`);
      for (const targetVersion of ["AC1015", "AC1024"]) {
        const text = api.readDxfString(input, { ...opts, targetVersion });
        assert.equal(text, api.readDxfString(input, { ...opts, targetVersion, explodeInserts: true }));
        assert.equal(sha(text), hashes[targetVersion], sample.id);
        assert.equal(sha(text), targetVersion === "AC1015" ? reference.dxf_sha256 : reference.dxf_2010_sha256);
        dxfHashes++;
      }
      conversions.push({ space, hashes });
    }
    evidence.push({ id: sample.id, input_sha256: sample.sha256, entities: py.cad.document.entities.length, conversions });
  }
  assert.equal(dxfHashes, 312);
  return { inputs: 80, accepted: 78, rejected: 2, dxf_hashes: dxfHashes, python_module: expected.python_module, cases: evidence };
}
