import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { parseArgs } from "node:util";
import { checkJwcParity } from "./jwc/check-parity.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(import.meta.url);
const { values } = parseArgs({ options: {
  python: { type: "string" }, package: { type: "string" }, report: { type: "string" },
} });
const packagePath = values.package ? resolve(values.package) : resolve(root, "packages/ezjww/dist/index.js");
const ezjww = require(packagePath);
const python = values.python ?? [".venv/bin/python", ".venv/Scripts/python.exe"]
  .map(p => resolve(root, p)).find(existsSync) ?? "python";

const sampleDir = resolve(root, "jww_samples");
function collect(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = resolve(directory, entry.name);
    return entry.isDirectory() ? collect(path) : /\.jww$/i.test(entry.name) ? [path] : [];
  });
}
const samplePaths = collect(sampleDir).sort();
const sampleId = path => relative(root, path).split("\\").join("/");

if (samplePaths.length === 0) {
  throw new Error(`no .jww samples found in ${sampleDir}`);
}

const expected = pythonSummary(samplePaths);
const actual = Object.fromEntries(
  samplePaths.map((path) => [sampleId(path), typescriptSummary(path)]),
);

assert.deepEqual(actual, expected);
const jwc = checkJwcParity({ root, api: ezjww, python });
if (values.report) writeFileSync(values.report, JSON.stringify({
  python, package: packagePath,
  jww: { inputs: samplePaths.length, cases: actual }, jwc,
}, null, 2) + "\n");
console.log(`python/ts parity ok: ${samplePaths.length} JWW, ${jwc.accepted}/${jwc.inputs} JWC accepted, ${jwc.dxf_hashes} P4 DXF hashes`);

function pythonSummary(paths) {
  const code = String.raw`
import hashlib
import json
import sys
from pathlib import Path

from ezjww import read_document, read_dxf_document, read_dxf_string

def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()

def sorted_map(value):
    return {str(k): value[k] for k in sorted(value)}

def validation_summary(value):
    return {
        "has_unresolved": bool(value["has_unresolved"]),
        "resolved_references": int(value["resolved_references"]),
        "total_references": int(value["total_references"]),
        "unresolved_def_numbers": list(value["unresolved_def_numbers"]),
    }

def dxf_summary(value):
    by_type = {}
    for entity in value["entities"]:
        by_type[entity["type"]] = by_type.get(entity["type"], 0) + 1
    return {
        "blocks": len(value["blocks"]),
        "entities": len(value["entities"]),
        "entity_counts": sorted_map(by_type),
        "layers": len(value["layers"]),
        "unsupported_entities": list(value["unsupported_entities"]),
    }

def summarize(path: Path):
    source = str(path)
    doc = read_document(source)
    dxf = read_dxf_document(source)
    exploded = read_dxf_document(source, True, 32)
    return {
        "block_defs": len(doc["block_defs"]),
        "block_def_names": sorted_map(doc["block_def_names"]),
        "dxf": dxf_summary(dxf),
        "dxf_exploded": dxf_summary(exploded),
        "dxf_exploded_sha256": sha256(read_dxf_string(source, True, 32)),
        "dxf_sha256": sha256(read_dxf_string(source)),
        "entities": len(doc["entities"]),
        "entity_counts": sorted_map(doc["entity_counts"]),
        "diagnostics": doc["diagnostics"],
        "header_version": int(doc["header"]["version"]),
        "validation": validation_summary(doc["validation"]),
    }

out = {sample["id"]: summarize(Path(sample["path"])) for sample in json.loads(sys.stdin.readline())}
print(json.dumps(out, ensure_ascii=False, sort_keys=True))
`;

  const output = execFileSync(python, ["-I", "-X", "utf8", "-c", code], {
    cwd: root,
    encoding: "utf8",
    input: JSON.stringify(paths.map(path => ({ id: sampleId(path), path }))) + "\n",
    maxBuffer: 32 * 1024 * 1024,
    timeout: 60_000,
    stdio: ["pipe", "pipe", "inherit"],
  });
  return JSON.parse(output);
}

function typescriptSummary(path) {
  const data = readFileSync(path);
  const doc = ezjww.readDocument(data);
  const dxf = ezjww.readDxfDocument(data);
  const exploded = ezjww.readDxfDocument(data, {
    explodeInserts: true,
    maxBlockNesting: 32,
  });

  return {
    block_defs: doc.block_defs.length,
    block_def_names: sortedObject(doc.block_def_names),
    dxf: dxfSummary(dxf),
    dxf_exploded: dxfSummary(exploded),
    dxf_exploded_sha256: sha256(ezjww.readDxfString(data, {
      explodeInserts: true,
      maxBlockNesting: 32,
    })),
    dxf_sha256: sha256(ezjww.readDxfString(data)),
    entities: doc.entities.length,
    entity_counts: sortedObject(doc.entity_counts),
    diagnostics: doc.diagnostics,
    header_version: doc.header.version,
    validation: {
      has_unresolved: doc.validation.has_unresolved,
      resolved_references: doc.validation.resolved_references,
      total_references: doc.validation.total_references,
      unresolved_def_numbers: doc.validation.unresolved_def_numbers,
    },
  };
}

function dxfSummary(value) {
  const byType = {};
  for (const entity of value.entities) {
    byType[entity.type] = (byType[entity.type] ?? 0) + 1;
  }
  return {
    blocks: value.blocks.length,
    entities: value.entities.length,
    entity_counts: sortedObject(byType),
    layers: value.layers.length,
    unsupported_entities: value.unsupported_entities,
  };
}

function sortedObject(value) {
  return Object.fromEntries(
    Object.entries(value).sort(([a], [b]) => a.localeCompare(b)),
  );
}

function sha256(text) {
  return createHash("sha256").update(text, "utf8").digest("hex");
}
