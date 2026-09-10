import fs from "node:fs";
import path from "node:path";
import { describe, expect, expectTypeOf, it } from "vitest";
import * as rawWasm from "../wasm/ezjww_wasm";
import {
  detectFileFormat, isJwcFile, isJwwFile, readCadDocument, readDocument,
  readDxfDocument, readDxfString, readHeader, readJwcDocument, readJwcHeader,
  type JwcHeader,
} from "../src/index";

const fixtures = path.resolve(__dirname, "../../../jwc_samples/generated");
const sample = (name = "q032") => new Uint8Array(fs.readFileSync(path.join(fixtures, `${name}.jwc`)));

describe("JWC public API", () => {
  it("keeps common JWW reads working with nonempty block name maps", () => {
    const input = fs.readFileSync(path.resolve(fixtures, "../../jww_samples/block_regressions/2blocks.jww"));
    const doc = readDocument(input);
    expect(Object.keys(doc.block_def_names)).toHaveLength(2);
    expect(readCadDocument(input)).toEqual({ format: "jww", document: doc });
    expect(readDxfDocument(input, { explodeInserts: true }).entities).toHaveLength(4);
  });

  it.each(["q070", "r001", "r013"])("never returns a partial truncated %s document", name => {
    const data = sample(name);
    for (const end of [0, 12, 200, 2420, 2421, data.length - 1]) {
      for (const read of [readJwcDocument, readCadDocument, readDxfDocument]) {
        expect(() => read(data.subarray(0, end))).toThrow();
      }
    }
  });

  it("keeps source types and version distinct from JWW", () => {
    const input = sample();
    expect(isJwcFile(input)).toBe(true);
    expect(isJwwFile(input)).toBe(false);
    expect(detectFileFormat(input)).toBe("jwc");
    const document = readJwcDocument(input);
    expect(document.header).toEqual(readJwcHeader(input));
    expect(document.header.source_version).toBeNull();
    expect(document.header).not.toHaveProperty("version");
    expect(document.entities[0]).toMatchObject({ type: "text", content: "日本語" });
    expect(document.entity_counts).toEqual({ TEXT: 1 });
    expect(readCadDocument(input)).toEqual({ format: "jwc", document });
    const cad = readCadDocument(input);
    if (cad.format === "jwc") expectTypeOf(cad.document.header).toEqualTypeOf<JwcHeader>();
    const jww = fs.readFileSync(path.join(fixtures, "q032rt.jww"));
    expectTypeOf(readDocument(jww).header.version).toBeNumber();
    expectTypeOf(readHeader(jww).version).toBeNumber();
    expect(readCadDocument(jww)).toEqual({ format: "jww", document: readDocument(jww) });
  });

  it("honors ArrayBufferView offsets and keeps JWW readers dedicated", () => {
    const input = sample();
    const storage = new Uint8Array(input.length + 16);
    storage.set(input, 7);
    const view = new DataView(storage.buffer, 7, input.length);
    expect(readJwcDocument(view)).toEqual(readJwcDocument(input.buffer));
    expect(readCadDocument(view).format).toBe("jwc");
    expect(() => readDocument(view)).toThrow();
    expect(() => readHeader(view)).toThrow();
    expect(detectFileFormat(new Uint8Array([1, 2, 3]))).toBeNull();
    expect(() => readCadDocument(new Uint8Array())).toThrow();
  });

  it("retains unverified line flags and reports them through source and conversion APIs", () => {
    const input = sample("r011");
    const document = readJwcDocument(input);
    expect(document.entities).toHaveLength(1);
    expect(document.entities[0]).toMatchObject({ type: "line", attributes: { flags_raw: 2 } });
    expect(document.diagnostics).toHaveLength(1);
    expect(document.diagnostics[0]).toMatchObject({
      code: "JWC_ATTRIBUTE_UNVERIFIED",
      severity: "warning",
      action: "retained",
      details: { field: "line.flags", byte_offset: 2441, count: 1, values: ["0x0002"] },
    });
    expect(readCadDocument(input)).toEqual({ format: "jwc", document });
    const dxf = readDxfDocument(input);
    expect(dxf.entities).toHaveLength(1);
    expect(dxf.entities[0].type).toBe("LINE");
    expect(dxf.jwc_conversion_report?.diagnostics).toEqual(document.diagnostics);
    expect(readDxfString(input)).toContain("\nLINE\n");
  });

  it.each([["r080", 2483]] as const)("rejects %s with its source offset", (name, offset) => {
    const input = sample(name);
    expect(isJwcFile(input)).toBe(true);
    for (const read of [readJwcDocument, readCadDocument, readDxfDocument, readDxfString]) {
      expect(() => read(input)).toThrow(`byte ${offset}`);
    }
  });

  it("carries scales, width factors, defaults and target versions to DXF", () => {
    const paper = readDxfDocument(sample("q054"));
    const model = readDxfDocument(sample("q054"), { jwcCoordinates: "model_millimeters" });
    expect(model.entities[0].x1).toBeCloseTo(paper.entities[0].x1! * 50, 7);
    expect(model.jwc_conversion_report?.coordinate_space).toBe("model_millimeters");
    const text = readDxfDocument(sample("r014"));
    expect(text.text_width_factors![0]).toBeCloseTo(3 / 4.2, 10);
    expect(text.jwc_conversion_report?.notices.some(n => n.kind === "dxf_limitation")).toBe(true);
    expect(readDxfString(sample(), { targetVersion: "AC1024" })).toContain("AC1024");
    expect(readDxfString(sample())).toContain("$INSUNITS");
    expect(() => readDxfDocument(sample(), { maxBlockNesting: 0 })).toThrow();
    expect(readDxfDocument(sample("r013"), { explodeInserts: true }).entities[0].type).toBe("ELLIPSE");
  });

  it("preserves CP932 diagnostics in both source and conversion reports", () => {
    const input = sample("q030");
    const text = readJwcDocument(input).entities[0];
    if (text.type !== "text") throw new Error("expected text fixture");
    input[text.string_source.byte_offset + text.string_source.byte_length - 2] = 0x81;
    const diagnostics = readJwcDocument(input).diagnostics;
    expect(diagnostics[0].code).toBe("CP932_DECODE_REPLACED");
    expect(readDxfDocument(input).jwc_conversion_report?.diagnostics).toEqual(diagnostics);
  });

  it("preserves JWC option positions and text widths with an optional em scale", () => {
    const input = sample("r014");
    const options = { jwcCoordinates: "model_millimeters", targetVersion: "AC1024" } as const;
    const plain = readDxfDocument(input, options);
    expect(rawWasm.readDxfDocument(input, false, 32, options.jwcCoordinates)).toEqual(plain);
    expect(rawWasm.readDxfString(input, false, 32, options.jwcCoordinates, options.targetVersion))
      .toBe(readDxfString(input, options));
    const scaled = readDxfDocument(input, { ...options, textEmScale: 1.364 });
    expect(scaled.entities[0].height).toBeCloseTo(plain.entities[0].height! / 1.364, 10);
    expect(scaled.entities[0].width_factor).toBeCloseTo(3 / 4.2, 10);
    expect(scaled.text_width_factors).toEqual(plain.text_width_factors);
    expect(rawWasm.readDxfString(input, false, 32, options.jwcCoordinates, options.targetVersion, 1.364))
      .toBe(readDxfString(input, { ...options, textEmScale: 1.364 }));
  });
});
