import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import {
  isJwwFile,
  readDocument,
  readDxfDocument,
  readDxfString,
  readHeader,
  toDxfString,
} from "../src/index";

const sample = readFileSync(resolve(__dirname, "../../../jww_samples/Test1.jww"));

describe("ezjww wasm wrapper", () => {
  it("detects and reads a JWW header", () => {
    expect(isJwwFile(sample)).toBe(true);

    const header = readHeader(sample);

    expect(header.version).toBe(600);
    expect(header.layer_groups).toHaveLength(16);
  });

  it("parses a JWW document with metadata fields", () => {
    const document = readDocument(sample);

    expect(document.entities.length).toBeGreaterThan(0);
    expect(document.entity_counts.LINE).toBeGreaterThan(0);
    expect(document.validation.has_unresolved).toBe(false);
    expect(document.block_def_names).toBeDefined();
  });

  it("converts a document to DXF entities and text", () => {
    const dxf = readDxfDocument(sample);
    const text = readDxfString(sample);

    expect(dxf.entities.length).toBeGreaterThan(0);
    expect(dxf.unsupported_entities).toEqual([]);
    expect(text).toContain("SECTION");
    expect(text.endsWith("  0\nEOF\n")).toBe(true);
    expect(toDxfString(sample)).toBe(text);
  });

  it("rejects invalid block nesting before calling wasm", () => {
    expect(() => readDxfDocument(sample, { maxBlockNesting: 0 })).toThrow(
      "maxBlockNesting must be an integer >= 1",
    );
  });
});
