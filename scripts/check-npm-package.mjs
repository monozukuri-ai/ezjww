// Install a packed archive and exercise public exports from outside this repo.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const { values } = parseArgs({ options: { archive: { type: "string" }, report: { type: "string" }, "expected-version": { type: "string" } } });
assert.ok(values.archive && values.report, "--archive and --report are required");
const archive = resolve(values.archive);
const workdir = mkdtempSync(join(tmpdir(), "ezjww-npm-"));
execFileSync("npm", ["install", "--offline", "--ignore-scripts", "--no-audit", "--no-fund", archive], {
  cwd: workdir, env: { ...process.env, npm_config_cache: join(workdir, "npm-cache") }, stdio: "inherit",
});
const packageRoot = join(workdir, "node_modules/ezjww");
const metadata = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));
const expectedVersion = values["expected-version"] ?? JSON.parse(readFileSync(join(root, "packages/ezjww/package.json"), "utf8")).version;
assert.equal(metadata.version, expectedVersion);
assert.equal(metadata.license, "MIT");
assert.deepEqual(metadata.dependencies ?? {}, {});
function payloadFiles(directory, prefix = "") {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    assert.ok(entry.isDirectory() || entry.isFile(), "unexpected package link");
    const name = prefix + entry.name;
    return entry.isDirectory() ? payloadFiles(join(directory, entry.name), name + "/") : [name];
  });
}
const files = payloadFiles(packageRoot);
assert.ok(files.every(name => /^(dist\/|wasm\/|package\.json$|README\.md$|LICENSE$)/.test(name)), "unexpected npm payload");
for (const required of ["dist/index.js", "dist/index.d.ts", "wasm/ezjww_wasm_bg.wasm", "LICENSE", "README.md"]) {
  assert.ok(files.includes(required), "missing npm file: " + required);
}
for (const name of ["q032", "q054", "r013", "r011", "r080"]) {
  copyFileSync(join(root, "jwc_samples/generated", `${name}.jwc`), join(workdir, `${name}.jwc`));
}
copyFileSync(join(root, "jww_samples/Test1.jww"), join(workdir, "Test1.jww"));
copyFileSync(join(root, "jww_samples/writer/empty.jww"), join(workdir, "empty.jww"));
copyFileSync(join(root, "jww_samples/writer/basic/basic.jww"), join(workdir, "basic.jww"));
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
const empty = fs.readFileSync('empty.jww');
assert.equal(ez.readDocument(empty).header.version, 700);
assert.deepEqual(ez.readDocument(empty).entities, []);
assert.deepEqual(ez.readDxfDocument(empty).entities, []);
const created = ez.newJwwDocument();
assert.deepEqual(Buffer.from(ez.toJwwBytes(created)), empty);
created.options.memo = '新規図面𠮷';
created.entities.push({
  type: 'LINE',
  base: {group:0, pen_style:1, pen_color:3, pen_width:25, layer:2, layer_group:0, flag:0},
  start_x:0, start_y:0, end_x:100, end_y:10,
});
const createdBytes = ez.toJwwBytes(created);
assert.ok(createdBytes instanceof Uint8Array);
fs.writeFileSync('created.jww', createdBytes);
const reopened = ez.readDocument(fs.readFileSync('created.jww'));
assert.equal(reopened.header.memo, created.options.memo);
assert.deepEqual(reopened.entities, created.entities);
assert.ok(ez.readDxfString(createdBytes).includes('\nLINE\n'));
created.entities[0].base.layer = 65536;
assert.throws(() => ez.toJwwBytes(created), e => String(e).includes('base.layer'));
assert.throws(() => ez.toJwwBytes(reopened), e => String(e).includes('document'));
const basic = ez.newJwwDocument();
const base = () => ({group:0, pen_style:1, pen_color:1, pen_width:0, layer:0, layer_group:0, flag:0});
const circle = (x, y, radius) => ({type:'CIRCLE', base:base(), center_x:x, center_y:y, radius, start_angle:0, arc_angle:2*Math.PI, tilt_angle:0, flatness:1, is_full_circle:true});
const text = (content, x1, y1, x2, y2) => ({type:'TEXT', base:base(), start_x:x1, start_y:y1, end_x:x2, end_y:y2, text_type:0, size_x:3, size_y:3, spacing:0, angle:0, font_name:'ＭＳ ゴシック', content});
const c = circle(-30,20,10); c.base.pen_color = 2;
const t = text('日本語 ABC',-50,-40,-20,-40); t.spacing = 0.5; t.base.pen_color = 5;
const rotated = text('縦方向',30,-40,30,-20); rotated.angle = 90; rotated.size_y = 5;
basic.entities.push(
  {type:'LINE',base:{...base(),pen_style:3,pen_color:3,pen_width:25,layer:2},start_x:-50,start_y:-20,end_x:50,end_y:-20},
  c, {...circle(0,20,10),type:'ARC',is_full_circle:false,start_angle:350*(Math.PI/180),arc_angle:30*(Math.PI/180)},
  {type:'POINT',base:{...base(),pen_color:4},x:30,y:20,is_temporary:false,code:0,angle:0,scale:0}, t, rotated,
);
const basicBytes = ez.toJwwBytes(basic);
assert.ok(Buffer.from(basicBytes).equals(fs.readFileSync('basic.jww')));
fs.writeFileSync('native-qualified.jww', basicBytes);
assert.deepEqual(ez.readDocument(fs.readFileSync('native-qualified.jww')).entities, basic.entities);
assert.equal(ez.readDxfDocument(basicBytes).entities.length, 6);
const extended = ez.newJwwDocument();
extended.entities.push({...basic.entities[0], color:0x1256AB, base:{...base(),pen_style:47}},
  {...circle(0,0,10), flatness:0.5, tilt_angle:Math.PI/6},
  {type:'SOLID',base:{...base(),pen_color:10},color:0xA53212,
   point1_x:0,point1_y:0,point4_x:10,point4_y:0,point2_x:10,point2_y:10,point3_x:0,point3_y:10});
const extendedParsed = ez.readDocument(ez.toJwwBytes(extended));
assert.deepEqual(extendedParsed.entities.map(e=>e.type), ['LINE','CIRCLE','SOLID']);
assert.equal(extendedParsed.header.palette.extended_colors[17],0x1256AB);
const converted = ez.toWriteDocument(extendedParsed);
assert.ok(converted.document);
assert.deepEqual(ez.readDocument(ez.toJwwBytes(converted.document)).entities,extendedParsed.entities);
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
import {
  readCadDocument, readDocument, readDxfDocument, readDxfString,
  newJwwDocument, toJwwBytes, toWriteDocument, type JwwWriteDocument, type JwwWriteEntity,
} from "ezjww";
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
const created: JwwWriteDocument = newJwwDocument();
const entity: JwwWriteEntity = {
  type: "LINE",
  base: {group:0, pen_style:1, pen_color:1, pen_width:0, layer:0, layer_group:0, flag:0},
  start_x:0, start_y:0, end_x:100, end_y:0,
};
created.entities.push(entity);
const bytes: Uint8Array = toJwwBytes(created);
const conversion: JwwWriteDocument | null = toWriteDocument(readDocument(bytes)).document;
created.entities.push({type:"SOLID",base:{...entity.base,pen_color:10},color:0x1256AB,
  point1_x:0,point1_y:0,point4_x:10,point4_y:0,point2_x:10,point2_y:10,point3_x:0,point3_y:10});
for (const e of created.entities) {
  if (e.type === "TEXT") { const content: string = e.content; }
  if (e.type === "ARC") { const full: false = e.is_full_circle; }
}
// @ts-expect-error A parsed document is not a writer input.
toJwwBytes(readDocument(input));
// @ts-expect-error Writer line geometry is required.
const incomplete: JwwWriteEntity = { type: "LINE", base: entity.base };
`);
execFileSync(process.execPath, [
  join(root, "packages/ezjww/node_modules/typescript/bin/tsc"), "--strict", "--noEmit",
  "--target", "ES2020", "--module", "Node16", typeProbe,
], { cwd: workdir, stdio: "inherit" });
const sha = p => createHash("sha256").update(readFileSync(p)).digest("hex");
const report = {
  ...result, version: metadata.version, files, workdir, archive, archive_sha256: sha(archive),
  writer_fixture_sha256: sha(join(workdir, "native-qualified.jww")),
  jwc_dxf_sha256: sha(join(workdir, "q032.dxf")),
  checked: ["JWW", "empty version-700 JWW", "new JWW generation and readback", "exact native-qualified mixed geometry bytes", "strict writer validation", "Japanese JWC", "both coordinate spaces", "ellipse", "DXF AC1024", "width factors", "unverified line flags and diagnostics", "structural rejection", "bundled WASM and LICENSE", "compiled consumer of installed declarations"],
};
mkdirSync(dirname(resolve(values.report)), { recursive: true });
writeFileSync(values.report, JSON.stringify(report, null, 2) + "\n");
console.log("Installed npm package JWW/JWC smoke passed.");
