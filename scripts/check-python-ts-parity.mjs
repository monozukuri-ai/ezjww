import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { basename, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(import.meta.url);
const ezjww = require(resolve(root, "packages/ezjww/dist/index.js"));
const python = existsSync(resolve(root, ".venv/bin/python"))
  ? resolve(root, ".venv/bin/python")
  : "python";

const sampleDir = resolve(root, "jww_samples");
const samplePaths = readdirSync(sampleDir)
  .filter((name) => name.endsWith(".jww"))
  .sort((a, b) => a.localeCompare(b, "ja"))
  .map((name) => resolve(sampleDir, name));

if (samplePaths.length === 0) {
  throw new Error(`no .jww samples found in ${sampleDir}`);
}

const expected = pythonSummary(samplePaths);
const actual = Object.fromEntries(
  samplePaths.map((path) => [basename(path), typescriptSummary(path)]),
);

assert.deepEqual(actual, expected);
console.log(`python/ts parity ok: ${samplePaths.length} sample(s)`);

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
        "dxf": dxf_summary(dxf),
        "dxf_exploded": dxf_summary(exploded),
        "dxf_exploded_sha256": sha256(read_dxf_string(source, True, 32)),
        "dxf_sha256": sha256(read_dxf_string(source)),
        "entities": len(doc["entities"]),
        "entity_counts": sorted_map(doc["entity_counts"]),
        "header_version": int(doc["header"]["version"]),
        "validation": validation_summary(doc["validation"]),
    }

out = {Path(path).name: summarize(Path(path)) for path in sys.argv[1:]}
print(json.dumps(out, ensure_ascii=False, sort_keys=True))
`;

  const output = execFileSync(python, ["-c", code, ...paths], {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
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
    dxf: dxfSummary(dxf),
    dxf_exploded: dxfSummary(exploded),
    dxf_exploded_sha256: sha256(ezjww.readDxfString(data, {
      explodeInserts: true,
      maxBlockNesting: 32,
    })),
    dxf_sha256: sha256(ezjww.readDxfString(data)),
    entities: doc.entities.length,
    entity_counts: sortedObject(doc.entity_counts),
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
