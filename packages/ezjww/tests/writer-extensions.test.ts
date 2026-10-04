import { describe, it, expect } from "vitest";
import { newJwwDocument, toJwwBytes, readDocument, toWriteDocument,
  type EntityBase, type JwwWriteEntity } from "../src/index";

const base = (): EntityBase => ({ group: 0, pen_style: 1, pen_color: 1,
  pen_width: 0, layer: 0, layer_group: 0, flag: 0 });
const line = () => ({ type: "LINE" as const, base: base(), start_x: 0, start_y: 0, end_x: 10, end_y: 0 });

describe("extended writer contract", () => {
  it("allocates COLORREF without changing input or existing pens", () => {
    const d = newJwwDocument();
    d.options.palette!.extended_colors![17] = 0x123456;
    d.entities.push({ ...line(), base: { ...base(), pen_color: 117 } },
      { ...line(), color: 0x987654 }, { ...line(), color: 0x987654 });
    const before = structuredClone(d);
    const parsed = readDocument(toJwwBytes(d));
    expect(parsed.entities.map(e => e.base.pen_color)).toEqual([117, 118, 118]);
    expect(parsed.header.palette!.extended_colors!.slice(17, 19)).toEqual([0x123456, 0x987654]);
    expect(d).toEqual(before);
  });
  it("writes ellipse, solid, dimension and nested block definitions", () => {
    const d = newJwwDocument();
    const text = { base: { ...base(), flag: 0x4010 }, start_x: 2, start_y: 1, end_x: 5, end_y: 1,
      size_x: 3, size_y: 3, spacing: 0, text_type: 10000, angle: 0, font_name: "Arial", content: "10" };
    const point = { base: { ...base(), flag: 0x40 }, x: 0, y: 0, is_temporary: false, code: 0, angle: 0, scale: 0 };
    const { type: _type, ...payload } = line();
    d.entities.push(
      { type: "CIRCLE", base: base(), center_x: 0, center_y: 0, radius: 20, flatness: 0.5,
        tilt_angle: Math.PI / 6, start_angle: 0, arc_angle: 2 * Math.PI, is_full_circle: true },
      { type: "SOLID", base: { ...base(), pen_color: 10 }, color: 0xA53212,
        point1_x: 0, point1_y: 0, point4_x: 10, point4_y: 0,
        point2_x: 10, point2_y: 10, point3_x: 0, point3_y: 10 },
      { type: "DIMENSION", base: base(), line: payload, text, sxf_mode: 0,
        aux_lines: [payload, payload], aux_points: [point, point, point, point] });
    d.block_defs = [{ base: base(), number: 0, name: "part", is_referenced: true, entities: [line()] }];
    d.entities.push({ type: "BLOCK", base: base(), def_number: 0, ref_x: 0, ref_y: 0,
      scale_x: 0.01, scale_y: 0.01, rotation: 0.5 });
    const parsed = readDocument(toJwwBytes(d));
    expect(parsed.entities.map(e => e.type)).toEqual(["CIRCLE", "SOLID", "DIMENSION", "BLOCK"]);
    const converted = toWriteDocument(parsed);
    expect(converted.document).not.toBeNull();
    expect(readDocument(toJwwBytes(converted.document!)).entities).toEqual(parsed.entities);
  });
  it.each([10, 16, 99, 357])("rejects reserved/out-of-range pen %i", pen_color => {
    const d = newJwwDocument();
    d.entities = [{ ...line(), base: { ...base(), pen_color } }];
    expect(() => toJwwBytes(d)).toThrow(/pen_color/);
  });
  it("reports every unsupported conversion and requires explicit skip", () => {
    const d = newJwwDocument();
    d.entities = [line()];
    const parsed = readDocument(toJwwBytes(d));
    parsed.entities.push({ ...line(), base: { ...base(), flag: 1 } });
    expect(toWriteDocument(parsed).document).toBeNull();
    const result = toWriteDocument(parsed, true);
    expect(result.document!.entities).toHaveLength(1);
    expect(result.diagnostics.some(e => e.action === "omitted")).toBe(true);
  });
  it("rejects malformed tables and block references", () => {
    const d = newJwwDocument();
    d.options.line_types!.sxf![17].segments_mm = [2, -1];
    expect(() => toJwwBytes(d)).toThrow(/segments_mm/);
    d.options.line_types = null;
    const block: JwwWriteEntity = { type: "BLOCK", base: base(), def_number: 0,
      ref_x: 0, ref_y: 0, scale_x: 1, scale_y: 1, rotation: 0 };
    d.entities = [block];
    expect(() => toJwwBytes(d)).toThrow(/missing block/);
    d.block_defs = [{ base: base(), number: 0, name: "cycle", is_referenced: true, entities: [block] }];
    expect(() => toJwwBytes(d)).toThrow(/cyclic/);
  });
});
