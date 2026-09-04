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
    expect(isJwwFile(new TextEncoder().encode("JwwData."))).toBe(true);
    expect(isJwwFile(new TextEncoder().encode("jwwData."))).toBe(false);
    expect(isJwwFile(new TextEncoder().encode("Jww"))).toBe(false);

    const header = readHeader(sample);

    expect(header.version).toBe(600);
    expect(header.layer_groups).toHaveLength(16);
    expect(header.palette?.pen_colors).toHaveLength(10);
    expect(header.palette?.extended_colors).toHaveLength(257);
    expect(header.palette?.extended_colors?.[1]).toBe(0x000000);
    expect(header.palette?.extended_colors?.[2]).toBe(0x0000ff);
  });

  it("parses a JWW document with metadata fields", () => {
    const document = readDocument(sample);

    expect(document.entities.length).toBeGreaterThan(0);
    expect(document.entity_counts.LINE).toBeGreaterThan(0);
    expect(document.validation.has_unresolved).toBe(false);
    expect(document.block_def_names).toBeDefined();
    expect(Array.isArray(document.diagnostics)).toBe(true);
  });

  it("reports structured CP932 replacement diagnostics", () => {
    const damaged = Uint8Array.from(sample);
    // 8-byte signature + 4-byte version + 1-byte short CString length.
    const memoPayloadOffset = 8 + 4 + 1;
    damaged[memoPayloadOffset] = 0xff;

    const document = readDocument(damaged);

    expect(document.diagnostics).toHaveLength(1);
    expect(document.diagnostics[0]).toMatchObject({
      code: "CP932_DECODE_REPLACED",
      severity: "warning",
      action: "normalized",
      details: {
        encoding: "cp932",
        field: "header.memo",
        byte_offset: memoPayloadOffset,
        had_errors: true,
      },
    });
    expect(
      document.diagnostics[0].details.replacement_characters,
    ).toBeGreaterThanOrEqual(1);
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

  it("rejects a text scale that cannot divide the height", () => {
    for (const textEmScale of [0, -1, Number.NaN]) {
      expect(() => readDxfDocument(sample, { textEmScale })).toThrow(
        "textEmScale must be a positive finite number",
      );
    }
  });

  it("scales only the DXF text height for a substituting renderer", () => {
    // group 41 carries the JWW pitch; only group 40 may move.
    const inflating = 1.364;
    const texts = (document: ReturnType<typeof readDxfDocument>) =>
      document.entities.filter((entity) => entity.type === "TEXT");

    const spec = texts(readDxfDocument(sample));
    const scaled = texts(readDxfDocument(sample, { textEmScale: inflating }));

    expect(spec.length).toBeGreaterThan(0);
    expect(scaled.length).toBe(spec.length);
    spec.forEach((plain, index) => {
      const corrected = scaled[index];
      expect(corrected.width_factor).toBeCloseTo(plain.width_factor!, 12);
      expect(corrected.height).toBeCloseTo(plain.height! / inflating, 12);
    });
  });
});
