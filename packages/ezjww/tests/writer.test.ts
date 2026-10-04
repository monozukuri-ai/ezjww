import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import * as raw from "../wasm/ezjww_wasm";
import {
  newJwwDocument, toJwwBytes, readDocument, readDxfDocument, readDxfString,
  type EntityBase, type JwwWriteDocument, type JwwWriteLine,
  type JwwWriteCircle, type JwwWriteArc, type JwwWriteText,
} from "../src/index";

const base = (): EntityBase => ({
  group: 0, pen_style: 1, pen_color: 1, pen_width: 0, layer: 0, layer_group: 0, flag: 0,
});
const line = (x1 = 0, y1 = 0, x2 = 100, y2 = 0): JwwWriteLine => ({
  type: "LINE", base: base(), start_x: x1, start_y: y1, end_x: x2, end_y: y2,
});
const circle = (x: number, y: number, radius: number): JwwWriteCircle => ({
  type: "CIRCLE", base: base(), center_x: x, center_y: y, radius,
  start_angle: 0, arc_angle: 2 * Math.PI, tilt_angle: 0, flatness: 1, is_full_circle: true,
});
const text = (content: string, x1: number, y1: number, x2: number, y2: number): JwwWriteText => ({
  type: "TEXT", base: base(), start_x: x1, start_y: y1, end_x: x2, end_y: y2,
  text_type: 0, size_x: 3, size_y: 3, spacing: 0, angle: 0, font_name: "ＭＳ ゴシック", content,
});

// Independent port of the Rust/Python acceptance inputs, without reading a
// fixture to construct the document or using its parser to choose defaults.
function acceptance(name: string): JwwWriteDocument {
  const doc = newJwwDocument();
  switch (name) {
    case "empty": break;
    case "line": doc.entities.push(line()); break;
    case "basic": {
      const l = line(-50, -20, 50, -20);
      Object.assign(l.base, { pen_style: 3, pen_color: 3, pen_width: 25, layer: 2 });
      const c = circle(-30, 20, 10);
      c.base.pen_color = 2;
      const arc: JwwWriteArc = {
        ...circle(0, 20, 10), type: "ARC", is_full_circle: false,
        start_angle: 350 * (Math.PI / 180), arc_angle: 30 * (Math.PI / 180),
      };
      const t1 = text("日本語 ABC", -50, -40, -20, -40);
      t1.spacing = 0.5;
      t1.base.pen_color = 5;
      const t2 = text("縦方向", 30, -40, 30, -20);
      t2.angle = 90;
      t2.size_y = 5;
      doc.entities.push(l, c, arc, {
        type: "POINT", base: { ...base(), pen_color: 4 },
        x: 30, y: 20, is_temporary: false, code: 0, angle: 0, scale: 0,
      }, t1, t2);
      break;
    }
    case "settings": {
      Object.assign(doc.options, { paper_size: 4, memo: "図面設定の検証\r\n", write_layer_group: 2 });
      doc.options.layer_groups.forEach((group, g) => {
        Object.assign(group, {
          name: `グループ${g.toString(16).toUpperCase()}`, state: g === 2 ? 3 : g % 3,
          write_layer: 5, scale: g === 2 ? 50 : 0.5, protect: g === 4 ? 1 : g === 5 ? 2 : 0,
        });
        group.layers.forEach((layer, l) => Object.assign(layer, {
          name: `層${g.toString(16).toUpperCase()}-${l.toString(16).toUpperCase()}`,
          state: l === 5 ? 3 : l % 3, protect: l === 8 ? 1 : l === 9 ? 2 : 0,
        }));
      });
      const l = line(-10, -5, 10, 5);
      Object.assign(l.base, { layer_group: 2, layer: 5 });
      const c = circle(0, 0, 2);
      Object.assign(c.base, { layer_group: 15, layer: 15 });
      doc.entities.push(l, c);
      break;
    }
    case "unicode":
      doc.options.memo = "日本語𠮷\r\n";
      doc.options.layer_groups[0].name = "図面𠮷";
      doc.options.layer_groups[0].layers[2].name = "日本語の層".repeat(60);
      ["", "日本語𠮷ｶﾅ ABC", "長".repeat(255), "文".repeat(1024)].forEach((s, i) => {
        doc.entities.push(text(s, 0, i * 10, 20, i * 10));
      });
      break;
    default: throw new Error(`unknown case ${name}`);
  }
  return doc;
}

describe("new JWW writer", () => {
  it.each(["empty", "line", "basic", "settings", "unicode"])(
    "generates exact native-qualified Rust bytes: %s", (name) => {
      const doc = acceptance(name);
      const before = structuredClone(doc);
      const path = ["empty", "line"].includes(name) ? `${name}.jww` : `basic/${name}.jww`;
      const fixture = readFileSync(resolve(__dirname, `../../../jww_samples/writer/${path}`));
      const bytes = toJwwBytes(doc);
      expect(bytes).toBeInstanceOf(Uint8Array);
      expect(Buffer.from(bytes)).toEqual(fixture);
      expect(raw.toJwwBytes(doc)).toEqual(bytes);
      expect(toJwwBytes(doc)).toEqual(bytes);
      expect(doc).toEqual(before);
      expect(readDocument(bytes).entities).toEqual(doc.entities);
      expect(readDocument(bytes).diagnostics).toEqual([]);
      expect(readDxfDocument(bytes).entities).toHaveLength(doc.entities.length);
      for (const targetVersion of ["AC1015", "AC1024"] as const) {
        expect(readDxfString(bytes, { targetVersion })).toBe(readDxfString(fixture, { targetVersion }));
      }
    },
  );

  it("returns fresh defaults and independent output buffers; edits appear on the next write", () => {
    const doc = newJwwDocument();
    expect(doc).toEqual(raw.newJwwDocument());
    expect(doc.options.layer_groups[15].layers[15].name).toBe("");
    doc.entities.push(line());
    const bytes = toJwwBytes(doc);
    doc.options.memo = "更新";
    doc.options.layer_groups[0].scale = 50;
    doc.entities[0] = line(0, 0, 20, 10);
    const latest = toJwwBytes(doc);
    expect(readDocument(bytes).entities[0].end_x).toBe(100);
    expect(readDocument(latest).entities[0].end_x).toBe(20);
    expect(readDocument(latest).header.memo).toBe("更新");
    expect(readDxfDocument(latest).entities[0].x2).toBe(20);
    bytes.fill(0);
    expect(toJwwBytes(doc)).toEqual(latest);
    expect(newJwwDocument().options.memo).toBe("");
    expect(newJwwDocument().options.layer_groups[0].scale).toBe(1);
  });

  // Exercise the raw WASM boundary as well: callers may bypass TypeScript or
  // edit nested objects after constructing them.
  const invalidCases: [string, (doc: any) => void, string][] = [
    ["old output version", d => { d.options.version = 600; }, "version"],
    ["paper code", d => { d.options.paper_size = 7; }, "paper_size"],
    ["boolean number", d => { d.entities[0].start_x = true; }, "entities[0].start_x"],
    ["numeric string", d => { d.entities[0].start_x = "2"; }, "entities[0].start_x"],
    ["NaN", d => { d.entities[0].start_x = NaN; }, "entities[0].start_x"],
    ["infinity", d => { d.entities[0].end_y = Infinity; }, "entities[0].end_y"],
    ["fractional integer", d => { d.entities[0].base.layer = 1.5; }, "base.layer"],
    ["negative integer", d => { d.entities[0].base.layer = -1; }, "base.layer"],
    ["u8 overflow", d => { d.entities[0].base.pen_style = 257; }, "pen_style"],
    ["u16 overflow", d => { d.entities[0].base.pen_color = 65537; }, "pen_color"],
    ["u32 overflow", d => { d.options.paper_size = 2 ** 32 + 3; }, "paper_size"],
    ["BigInt", d => { d.options.paper_size = 3n; }, "paper_size"],
    ["unsupported pen", d => { d.entities[0].base.pen_color = 99; }, "pen_color"],
    ["unsupported group", d => { d.entities[0].base.group = 1; }, "entities[0].base"],
    ["unsupported entity", d => { d.entities[0].type = "IMAGE"; }, "entities[0].type"],
    ["unknown root field", d => { d.header = {}; }, "header"],
    ["unknown option", d => { d.options.palette = {}; }, "options.palette"],
    ["unknown group field", d => { d.options.layer_groups[0].extra = 0; }, "layer_groups[0].extra"],
    ["unknown layer field", d => { d.options.layer_groups[0].layers[0].extra = 0; }, "layers[0].extra"],
    ["unknown base field", d => { d.entities[0].base.color = 3; }, "base.color"],
    ["unknown entity field", d => { d.entities[0].z = 1; }, "entities[0].z"],
    ["missing option", d => { delete d.options.memo; }, "options.memo"],
    ["missing field", d => { delete d.entities[0].end_y; }, "entities[0].end_y"],
    ["missing base", d => { delete d.entities[0].base; }, "entities[0].base"],
    ["missing entity type", d => { delete d.entities[0].type; }, "entities[0].type"],
    ["array option", d => { d.options = []; }, "options"],
    ["null base", d => { d.entities[0].base = null; }, "entities[0].base"],
    ["map options", d => { d.options = new Map(Object.entries(d.options)); }, "options.layer_groups"],
    ["non-array entities", d => { d.entities = new Set(d.entities); }, "entities"],
    ["sparse entities", d => { d.entities.length = 2; }, "entities[1]"],
    ["group count", d => { d.options.layer_groups.pop(); }, "layer_groups"],
    ["layer count", d => { d.options.layer_groups[0].layers.pop(); }, "layers"],
    ["invalid scale", d => { d.options.layer_groups[0].scale = 0; }, "scale"],
    ["inconsistent current layer", d => { d.options.layer_groups[0].write_layer = 2; }, "layer_groups[0]"],
    ["NUL in memo", d => { d.options.memo = "a\0b"; }, "memo"],
    ["unpaired surrogate", d => { d.options.memo = "\ud800"; }, "memo"],
    ["symbol key", d => { d[Symbol("extra")] = true; }, "string keys"],
    ["inherited field", d => { d.entities[0] = Object.create(d.entities[0]); }, "entities[0].type"],
  ];
  it.each(invalidCases)("rejects %s in raw and wrapped WASM", (_label, change, field) => {
    const doc = acceptance("line");
    change(doc);
    for (const write of [toJwwBytes, raw.toJwwBytes]) {
      expect(() => write(doc)).toThrow(field);
    }
  });

  it("rejects a parsed document and invalid root values", () => {
    for (const input of [null, undefined, [], 1, "", readDocument(toJwwBytes(newJwwDocument()))]) {
      expect(() => raw.toJwwBytes(input)).toThrow("document");
    }
  });

  it("validates circle, point and text semantics without normalization", () => {
    const changes: [(doc: any) => void, string][] = [
      [d => { d.entities[1].is_full_circle = 1; }, "is_full_circle"],
      [d => { d.entities[1].is_full_circle = false; }, "is_full_circle"],
      [d => { d.entities[1].radius = -1; }, "radius"],
      [d => { d.entities[1].flatness = 0; }, "flatness"],
      [d => { d.entities[2].arc_angle = 2 * Math.PI; }, "arc_angle"],
      [d => { d.entities[3].is_temporary = true; }, "is_temporary"],
      [d => { d.entities[3].is_temporary = 0; }, "is_temporary"],
      [d => { d.entities[3].code = 1; }, "code"],
      [d => { d.entities[4].content = "a\nb"; }, "content"],
      [d => { d.entities[4].content = "\udc00"; }, "content"],
      [d => { d.entities[4].font_name = ""; }, "font_name"],
      [d => { d.entities[4].size_y = 0; }, "size_y"],
    ];
    for (const [change, field] of changes) {
      const doc = acceptance("basic");
      change(doc);
      expect(() => toJwwBytes(doc)).toThrow(field);
      expect(() => raw.toJwwBytes(doc)).toThrow(field);
    }
  });

  it("exposes a discriminated union with required fields", () => {
    const doc = acceptance("basic");
    for (const entity of doc.entities) {
      if (entity.type === "TEXT") expect(entity.content.toUpperCase()).toBeTypeOf("string");
      if (entity.type === "CIRCLE") {
        const full: true = entity.is_full_circle;
        expect(full).toBe(true);
      }
    }
    // Compile-time checks, without sending intentionally invalid data to WASM.
    // @ts-expect-error Writer line geometry is required.
    const incomplete: JwwWriteLine = { type: "LINE", base: base() };
    // @ts-expect-error Circle discriminator requires is_full_circle: true.
    const wrongCircle: JwwWriteCircle = { ...circle(0, 0, 1), is_full_circle: false };
    // @ts-expect-error A parsed document lacks writer options.
    const parsed: JwwWriteDocument = readDocument(toJwwBytes(doc));
    void [incomplete, wrongCircle, parsed];
  });
});
