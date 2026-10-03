import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { gunzipSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import * as raw from "../wasm/ezjww_wasm";
import {
  newJwwDocument, readDocument, toJwwBytes, type JwwWriteEntity,
} from "../src/index";

const fixtures = resolve(__dirname, "../../../jww_samples/writer/compatibility");
const manifest = JSON.parse(readFileSync(resolve(fixtures, "manifest.json"), "utf8")) as {
  cases: { id: string; input_entities: number; native_memo: string }[];
};
const bytes = (name: string): Uint8Array =>
  gunzipSync(readFileSync(resolve(fixtures, `${name}.jww.gz`)));

describe("recorded Jw_cad compatibility matrix", () => {
  for (const entry of manifest.cases) {
    it(entry.id, () => {
      const input = bytes(entry.id);
      const source = readDocument(input);
      expect(source.diagnostics).toEqual([]);
      expect(source.entities).toHaveLength(entry.input_entities);
      // Restore this matrix's known writer settings. Readers synthesize display
      // names for blank layers, so parsed headers are not lossless writer input.
      const drawing = newJwwDocument();
      const { version, memo, paper_size, write_layer_group, layer_groups } = source.header;
      Object.assign(drawing.options, { version, memo, paper_size, write_layer_group });
      if (entry.id === "settings") {
        drawing.options.layer_groups = layer_groups;
      } else if (entry.id === "unicode" || entry.id === "cstring_boundaries") {
        const groups = entry.id === "unicode" ? 1 : 7;
        for (let g = 0; g < groups; g++) {
          drawing.options.layer_groups[g].name = layer_groups[g].name;
          drawing.options.layer_groups[g].layers[2].name = layer_groups[g].layers[2].name;
        }
      }
      drawing.entities = source.entities as JwwWriteEntity[];
      expect(Buffer.from(toJwwBytes(drawing)).equals(Buffer.from(input))).toBe(true);
      expect(Buffer.from(raw.toJwwBytes(drawing)).equals(Buffer.from(input))).toBe(true);
      for (const suffix of ["_saved", "_reopened"]) {
        const nativeBytes = bytes(entry.id + suffix);
        const native = readDocument(nativeBytes);
        expect(native).toEqual(raw.readDocument(nativeBytes));
        expect(native.diagnostics).toEqual([]);
        expect(native.header.layer_groups).toEqual(source.header.layer_groups);
        expect(native.header.palette).toEqual(source.header.palette);
        expect(native.header.memo).toBe(entry.native_memo);
        const removed = source.entities.filter(e => e.type === "TEXT" && e.content === "").length;
        expect(native.entities).toHaveLength(entry.input_entities - removed + 6);
      }
    }, 30_000);
  }
});
