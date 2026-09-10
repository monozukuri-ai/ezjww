// Install a packed archive and exercise public exports from outside this repo.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const { values } = parseArgs({ options: { archive: { type: "string" }, report: { type: "string" } } });
assert.ok(values.archive && values.report, "--archive and --report are required");
const archive = resolve(values.archive);
const workdir = mkdtempSync(join(tmpdir(), "ezjww-npm-"));
execFileSync("npm", ["install", "--offline", "--ignore-scripts", "--no-audit", "--no-fund", archive], {
  cwd: workdir, env: { ...process.env, npm_config_cache: join(workdir, "npm-cache") }, stdio: "inherit",
});
for (const name of ["q032", "q054", "r013", "r011", "r080"]) {
  copyFileSync(join(root, "jwc_samples/generated", `${name}.jwc`), join(workdir, `${name}.jwc`));
}
copyFileSync(join(root, "jww_samples/Test1.jww"), join(workdir, "Test1.jww"));
const probe = String.raw`
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const ez = require('ezjww');
const entry = require.resolve('ezjww');
assert.equal(entry, path.join(process.cwd(), 'node_modules/ezjww/dist/index.js'));
assert.ok(fs.existsSync(path.join(process.cwd(), 'node_modules/ezjww/dist/index.d.ts')));
assert.equal(fs.readFileSync(path.join(process.cwd(), 'node_modules/ezjww/LICENSE'), 'utf8'), fs.readFileSync('expected-license.txt', 'utf8'));
const input = fs.readFileSync('q032.jwc');
assert.equal(ez.detectFileFormat(input), 'jwc');
const cad = ez.readCadDocument(input);
assert.equal(cad.document.header.source_version, null);
assert.equal(cad.document.entities[0].content, '日本語');
assert.equal(ez.readDocument(fs.readFileSync('Test1.jww')).header.version, 600);
assert.ok(ez.toDxfString(fs.readFileSync('Test1.jww')).includes('SECTION'));
const scaled = fs.readFileSync('q054.jwc');
const paper = ez.readDxfDocument(scaled).entities[0];
const model = ez.readDxfDocument(scaled, {jwcCoordinates:'model_millimeters'}).entities[0];
assert.ok(Math.abs(model.x1 - 50 * paper.x1) < 1e-10);
assert.equal(ez.readDxfDocument(fs.readFileSync('r013.jwc')).entities[0].type, 'ELLIPSE');
const dxf = ez.readDxfString(input, {targetVersion:'AC1024'});
assert.ok(dxf.includes('AC1024'));
const tags = dxf.split(/\r?\n/).map(line => line.trim());
const units = tags.indexOf('$INSUNITS');
assert.deepEqual(tags.slice(units + 1, units + 3), ['70', '4']);
assert.ok(ez.readDxfDocument(input).text_width_factors.length > 0);
const flaggedInput = fs.readFileSync('r011.jwc');
const flagged = ez.readJwcDocument(flaggedInput);
assert.equal(flagged.entities.length, 1);
assert.equal(flagged.entities[0].attributes.flags_raw, 2);
assert.equal(flagged.diagnostics.length, 1);
assert.equal(flagged.diagnostics[0].code, 'JWC_ATTRIBUTE_UNVERIFIED');
assert.equal(flagged.diagnostics[0].severity, 'warning');
assert.equal(flagged.diagnostics[0].action, 'retained');
assert.deepEqual(flagged.diagnostics[0].details, {
  field: 'line.flags', byte_offset: 2441, count: 1, values: ['0x0002'],
});
assert.deepEqual(ez.readCadDocument(flaggedInput), {format: 'jwc', document: flagged});
const flaggedDxf = ez.readDxfDocument(flaggedInput);
assert.equal(flaggedDxf.entities.length, 1);
assert.equal(flaggedDxf.entities[0].type, 'LINE');
assert.deepEqual(flaggedDxf.jwc_conversion_report.diagnostics, flagged.diagnostics);
assert.ok(ez.readDxfString(flaggedInput).includes('\nLINE\n'));
for (const [name, offset] of [['r080',2483]]) {
  const data = fs.readFileSync(name + '.jwc');
  for (const read of [ez.readJwcDocument, ez.readCadDocument, ez.readDxfString]) {
    assert.throws(() => read(data), e => String(e).includes('byte ' + offset));
  }
}
fs.writeFileSync('q032.dxf', dxf);
console.log(JSON.stringify({entry, node:process.version, source_tree_imported:false}));
`;
const script = join(workdir, "probe.cjs");
copyFileSync(join(root, "LICENSE"), join(workdir, "expected-license.txt"));
writeFileSync(script, probe);
const result = JSON.parse(execFileSync(process.execPath, [script], { cwd: workdir, encoding: "utf8" }));
const typeProbe = join(workdir, "probe.ts");
writeFileSync(typeProbe, `
import { readCadDocument, readDocument, readDxfDocument, readDxfString } from "ezjww";
const input = new Uint8Array();
const cad = readCadDocument(input);
if (cad.format === "jwc") {
  const version: string | null = cad.document.header.source_version;
} else {
  const version: number = cad.document.header.version;
}
const version: number = readDocument(input).header.version;
const dxf = readDxfDocument(input, { jwcCoordinates: "model_millimeters" });
const factors: number[] | undefined = dxf.text_width_factors;
const output: string = readDxfString(input, { targetVersion: "AC1024" });
`);
execFileSync(process.execPath, [
  join(root, "packages/ezjww/node_modules/typescript/bin/tsc"), "--strict", "--noEmit",
  "--target", "ES2020", "--module", "Node16", typeProbe,
], { cwd: workdir, stdio: "inherit" });
const sha = p => createHash("sha256").update(readFileSync(p)).digest("hex");
const report = {
  ...result, workdir, archive, archive_sha256: sha(archive),
  jwc_dxf_sha256: sha(join(workdir, "q032.dxf")),
  checked: ["JWW", "Japanese JWC", "both coordinate spaces", "ellipse", "DXF AC1024", "width factors", "unverified line flags and diagnostics", "structural rejection", "bundled WASM and LICENSE", "compiled consumer of installed declarations"],
};
mkdirSync(dirname(resolve(values.report)), { recursive: true });
writeFileSync(values.report, JSON.stringify(report, null, 2) + "\n");
console.log("Installed npm package JWW/JWC smoke passed.");
