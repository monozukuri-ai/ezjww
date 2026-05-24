import initWasm, {
  isJwwFile,
  readDocument,
  readDxfDocument,
  readDxfString,
} from "./wasm/ezjww_wasm.js";
import type {
  DxfDocument,
  DxfEntity,
  JwwDocument,
  JwwEntity,
  JwwHeader,
} from "../../../src/index";
import "./styles.css";

type ViewMode = "summary" | "entities" | "json";

interface ParsedState {
  fileName: string;
  fileSize: number;
  document: JwwDocument;
  dxf: DxfDocument;
  dxfText: string;
  elapsedMs: number;
  explodeInserts: boolean;
}

interface CurrentInput {
  name: string;
  size: number;
  bytes: Uint8Array;
}

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) {
  throw new Error("missing #app");
}

let wasmReady: Promise<void> | null = null;
let parsedState: ParsedState | null = null;
let currentInput: CurrentInput | null = null;
let viewMode: ViewMode = "summary";
let explodeInserts = true;

app.innerHTML = `
  <header class="topbar">
    <div>
      <div class="eyebrow">ezjww</div>
      <h1>Browser Parser</h1>
    </div>
    <div class="actions">
      <label class="toggle">
        <input id="explode-toggle" type="checkbox" checked />
        <span>INSERT展開</span>
      </label>
      <button id="sample-button" class="tool-button" type="button">サンプル読込</button>
      <label class="file-button">
        <input id="file-input" type="file" accept=".jww,application/octet-stream" />
        ファイル選択
      </label>
    </div>
  </header>

  <main class="shell">
    <section class="left-pane">
      <div id="dropzone" class="dropzone" tabindex="0">
        <div class="drop-title">JWWファイルをドロップ</div>
        <div id="status-line" class="drop-status">未読込</div>
      </div>

      <div id="metrics" class="metrics"></div>
      <div id="counts" class="panel"></div>
      <div id="layers" class="panel"></div>
    </section>

    <section class="right-pane">
      <div class="preview-head">
        <div>
          <div class="section-label">Preview</div>
          <div id="preview-meta" class="muted">No drawing</div>
        </div>
        <div class="tabs" role="tablist" aria-label="表示切替">
          <button class="tab active" data-tab="summary" type="button">概要</button>
          <button class="tab" data-tab="entities" type="button">一覧</button>
          <button class="tab" data-tab="json" type="button">JSON</button>
        </div>
      </div>
      <canvas id="preview-canvas" class="preview" width="1200" height="760"></canvas>
      <div id="detail" class="detail"></div>
    </section>
  </main>
`;

const fileInput = document.querySelector<HTMLInputElement>("#file-input")!;
const dropzone = document.querySelector<HTMLDivElement>("#dropzone")!;
const statusLine = document.querySelector<HTMLDivElement>("#status-line")!;
const explodeToggle = document.querySelector<HTMLInputElement>("#explode-toggle")!;
const sampleButton = document.querySelector<HTMLButtonElement>("#sample-button")!;
const canvas = document.querySelector<HTMLCanvasElement>("#preview-canvas")!;

void ensureWasm().then(() => {
  statusLine.textContent = "待機中";
});

fileInput.addEventListener("change", () => {
  const file = fileInput.files?.[0];
  if (file) {
    void parseFile(file);
  }
});

explodeToggle.addEventListener("change", () => {
  explodeInserts = explodeToggle.checked;
  if (parsedState) {
    void reparseCurrentInput();
  }
});

sampleButton.addEventListener("click", () => {
  void loadSample();
});

dropzone.addEventListener("dragover", (event) => {
  event.preventDefault();
  dropzone.classList.add("dragging");
});

dropzone.addEventListener("dragleave", () => {
  dropzone.classList.remove("dragging");
});

dropzone.addEventListener("drop", (event) => {
  event.preventDefault();
  dropzone.classList.remove("dragging");
  const file = event.dataTransfer?.files[0];
  if (file) {
    fileInput.files = event.dataTransfer?.files ?? null;
    void parseFile(file);
  }
});

for (const button of document.querySelectorAll<HTMLButtonElement>(".tab")) {
  button.addEventListener("click", () => {
    viewMode = button.dataset.tab as ViewMode;
    document
      .querySelectorAll<HTMLButtonElement>(".tab")
      .forEach((node) => node.classList.toggle("active", node === button));
    render();
  });
}

async function reparseCurrentInput(): Promise<void> {
  if (!currentInput) {
    render();
    return;
  }
  await parseBytes(currentInput);
}

async function parseFile(file: File): Promise<void> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  currentInput = {
    name: file.name,
    size: file.size,
    bytes,
  };
  await parseBytes(currentInput);
}

async function loadSample(): Promise<void> {
  statusLine.textContent = "サンプル読込中";
  try {
    const response = await fetch("/samples/Test1.jww");
    if (!response.ok) {
      throw new Error(`sample load failed: ${response.status}`);
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    currentInput = {
      name: "Test1.jww",
      size: bytes.byteLength,
      bytes,
    };
    await parseBytes(currentInput);
  } catch (error) {
    statusLine.textContent = error instanceof Error ? error.message : String(error);
  }
}

async function parseBytes(input: CurrentInput): Promise<void> {
  statusLine.textContent = "読込中";
  try {
    await ensureWasm();
    const started = performance.now();
    if (!isJwwFile(input.bytes)) {
      throw new Error("JWWシグネチャを確認できません");
    }

    const document = readDocument(input.bytes) as JwwDocument;
    const dxf = readDxfDocument(input.bytes, explodeInserts, 32) as DxfDocument;
    const dxfText = readDxfString(input.bytes, explodeInserts, 32);

    parsedState = {
      fileName: input.name,
      fileSize: input.size,
      document,
      dxf,
      dxfText,
      elapsedMs: performance.now() - started,
      explodeInserts,
    };
    statusLine.textContent = `${input.name} / ${formatBytes(input.size)}`;
    render();
  } catch (error) {
    parsedState = null;
    statusLine.textContent = error instanceof Error ? error.message : String(error);
    render();
  }
}

async function ensureWasm(): Promise<void> {
  if (!wasmReady) {
    wasmReady = initWasm().then(() => undefined);
  }
  return wasmReady;
}

function render(): void {
  renderMetrics(parsedState);
  renderCounts(parsedState);
  renderLayers(parsedState?.document.header ?? null);
  renderPreview(parsedState);
  renderDetail(parsedState);
}

function renderMetrics(state: ParsedState | null): void {
  const target = document.querySelector<HTMLDivElement>("#metrics")!;
  if (!state) {
    target.innerHTML = metricMarkup([
      ["Version", "-"],
      ["Entities", "-"],
      ["DXF", "-"],
      ["Parse", "-"],
    ]);
    return;
  }

  target.innerHTML = metricMarkup([
    ["Version", String(state.document.header.version)],
    ["Entities", formatNumber(state.document.entities.length)],
    ["DXF", formatNumber(state.dxf.entities.length)],
    ["Parse", `${state.elapsedMs.toFixed(1)} ms`],
  ]);
}

function renderCounts(state: ParsedState | null): void {
  const target = document.querySelector<HTMLDivElement>("#counts")!;
  if (!state) {
    target.innerHTML = panelTitle("Entity Counts") + emptyLine();
    return;
  }

  const rows = Object.entries(state.document.entity_counts)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(
      ([type, count]) => `
        <tr>
          <td>${escapeHtml(type)}</td>
          <td class="num">${formatNumber(count)}</td>
        </tr>
      `,
    )
    .join("");

  const validation = state.document.validation;
  target.innerHTML = `
    ${panelTitle("Entity Counts")}
    <table class="data-table">
      <tbody>${rows}</tbody>
    </table>
    <div class="status-grid">
      <span>Block refs</span>
      <strong>${validation.resolved_references}/${validation.total_references}</strong>
      <span>Unsupported</span>
      <strong>${state.dxf.unsupported_entities.length}</strong>
    </div>
  `;
}

function renderLayers(header: JwwHeader | null): void {
  const target = document.querySelector<HTMLDivElement>("#layers")!;
  if (!header) {
    target.innerHTML = panelTitle("Layer Groups") + emptyLine();
    return;
  }

  const groups = header.layer_groups
    .map((group, index) => {
      const namedLayers = group.layers.filter((layer) => layer.name.trim()).length;
      return `
        <li>
          <span>${index.toString(16).toUpperCase()}</span>
          <strong>${escapeHtml(group.name)}</strong>
          <em>${namedLayers}</em>
        </li>
      `;
    })
    .join("");

  target.innerHTML = `
    ${panelTitle("Layer Groups")}
    <ul class="layer-list">${groups}</ul>
  `;
}

function renderPreview(state: ParsedState | null): void {
  const meta = document.querySelector<HTMLDivElement>("#preview-meta")!;
  const context = canvas.getContext("2d");
  if (!context) {
    return;
  }

  const rect = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.max(1, Math.floor(rect.width * dpr));
  canvas.height = Math.max(1, Math.floor(rect.height * dpr));
  context.setTransform(dpr, 0, 0, dpr, 0, 0);
  context.clearRect(0, 0, rect.width, rect.height);
  context.fillStyle = "#f7f8fb";
  context.fillRect(0, 0, rect.width, rect.height);

  if (!state) {
    meta.textContent = "No drawing";
    drawEmptyPreview(context, rect.width, rect.height);
    return;
  }

  meta.textContent = `${state.fileName} / ${state.explodeInserts ? "exploded" : "inserted"}`;
  drawDxf(context, rect.width, rect.height, state.dxf);
}

function renderDetail(state: ParsedState | null): void {
  const target = document.querySelector<HTMLDivElement>("#detail")!;
  if (!state) {
    target.innerHTML = `<div class="empty-detail">No data</div>`;
    return;
  }

  if (viewMode === "entities") {
    target.innerHTML = renderEntityTable(state.document.entities);
    return;
  }

  if (viewMode === "json") {
    const json = JSON.stringify(state.document, null, 2);
    target.innerHTML = `<pre class="json-view">${escapeHtml(json)}</pre>`;
    return;
  }

  target.innerHTML = `
    <div class="summary-grid">
      <div>
        <span>File</span>
        <strong>${escapeHtml(state.fileName)}</strong>
      </div>
      <div>
        <span>Size</span>
        <strong>${formatBytes(state.fileSize)}</strong>
      </div>
      <div>
        <span>Blocks</span>
        <strong>${formatNumber(state.document.block_defs.length)}</strong>
      </div>
      <div>
        <span>DXF text</span>
        <strong>${formatBytes(new Blob([state.dxfText]).size)}</strong>
      </div>
    </div>
    <div class="issue-line ${state.document.validation.has_unresolved ? "warn" : ""}">
      ${state.document.validation.has_unresolved ? "未解決のBLOCK参照あり" : "BLOCK参照は解決済み"}
    </div>
  `;
}

function renderEntityTable(entities: JwwEntity[]): string {
  const rows = entities
    .slice(0, 200)
    .map((entity, index) => {
      const point = firstPoint(entity);
      return `
        <tr>
          <td class="num">${index + 1}</td>
          <td>${escapeHtml(entity.type)}</td>
          <td class="num">${entity.base.layer_group}:${entity.base.layer}</td>
          <td class="num">${entity.base.pen_color}</td>
          <td>${point}</td>
        </tr>
      `;
    })
    .join("");

  return `
    <table class="entity-table">
      <thead>
        <tr>
          <th>#</th>
          <th>Type</th>
          <th>Layer</th>
          <th>Color</th>
          <th>Point</th>
        </tr>
      </thead>
      <tbody>${rows}</tbody>
    </table>
  `;
}

function drawDxf(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  dxf: DxfDocument,
): void {
  const bounds = dxfBounds(dxf.entities);
  if (!bounds) {
    drawEmptyPreview(context, width, height);
    return;
  }

  const padding = 28;
  const modelWidth = Math.max(1, bounds.maxX - bounds.minX);
  const modelHeight = Math.max(1, bounds.maxY - bounds.minY);
  const scale = Math.min(
    (width - padding * 2) / modelWidth,
    (height - padding * 2) / modelHeight,
  );

  const toScreen = (x: number, y: number): [number, number] => [
    padding + (x - bounds.minX) * scale,
    height - padding - (y - bounds.minY) * scale,
  ];

  drawGrid(context, width, height);
  context.lineCap = "round";
  context.lineJoin = "round";

  for (const entity of dxf.entities) {
    context.strokeStyle = colorFor(entity.color);
    context.fillStyle = context.strokeStyle;
    context.lineWidth = 1.2;
    drawEntity(context, entity, toScreen, scale);
  }
}

function drawEntity(
  context: CanvasRenderingContext2D,
  entity: DxfEntity,
  toScreen: (x: number, y: number) => [number, number],
  scale: number,
): void {
  if (entity.type === "LINE" && hasNumbers(entity, "x1", "y1", "x2", "y2")) {
    const [x1, y1] = toScreen(entity.x1, entity.y1);
    const [x2, y2] = toScreen(entity.x2, entity.y2);
    context.beginPath();
    context.moveTo(x1, y1);
    context.lineTo(x2, y2);
    context.stroke();
    return;
  }

  if (entity.type === "CIRCLE" && hasNumbers(entity, "center_x", "center_y", "radius")) {
    const [x, y] = toScreen(entity.center_x, entity.center_y);
    context.beginPath();
    context.arc(x, y, Math.max(0.8, entity.radius * scale), 0, Math.PI * 2);
    context.stroke();
    return;
  }

  if (entity.type === "ARC" && hasNumbers(entity, "center_x", "center_y", "radius", "start_angle", "end_angle")) {
    drawArcPolyline(context, entity, toScreen);
    return;
  }

  if (entity.type === "SOLID" && hasNumbers(entity, "x1", "y1", "x2", "y2", "x3", "y3", "x4", "y4")) {
    const p1 = toScreen(entity.x1, entity.y1);
    const p2 = toScreen(entity.x2, entity.y2);
    const p3 = toScreen(entity.x3, entity.y3);
    const p4 = toScreen(entity.x4, entity.y4);
    context.globalAlpha = 0.22;
    context.beginPath();
    context.moveTo(...p1);
    context.lineTo(...p2);
    context.lineTo(...p3);
    context.lineTo(...p4);
    context.closePath();
    context.fill();
    context.globalAlpha = 1;
    context.stroke();
    return;
  }

  if (entity.type === "POINT" && hasNumbers(entity, "x", "y")) {
    const [x, y] = toScreen(entity.x, entity.y);
    context.beginPath();
    context.arc(x, y, 2.4, 0, Math.PI * 2);
    context.fill();
    return;
  }

  if (entity.type === "TEXT" && hasNumbers(entity, "x", "y")) {
    const [x, y] = toScreen(entity.x, entity.y);
    context.fillRect(x - 2, y - 2, 4, 4);
  }
}

function drawArcPolyline(
  context: CanvasRenderingContext2D,
  entity: DxfEntity,
  toScreen: (x: number, y: number) => [number, number],
): void {
  let start = degToRad(entity.start_angle ?? 0);
  let end = degToRad(entity.end_angle ?? 0);
  if (end < start) {
    end += Math.PI * 2;
  }

  const steps = Math.max(8, Math.ceil((end - start) / (Math.PI / 24)));
  context.beginPath();
  for (let i = 0; i <= steps; i += 1) {
    const t = start + ((end - start) * i) / steps;
    const x = (entity.center_x ?? 0) + (entity.radius ?? 0) * Math.cos(t);
    const y = (entity.center_y ?? 0) + (entity.radius ?? 0) * Math.sin(t);
    const point = toScreen(x, y);
    if (i === 0) {
      context.moveTo(...point);
    } else {
      context.lineTo(...point);
    }
  }
  context.stroke();
}

function dxfBounds(entities: DxfEntity[]): Bounds | null {
  const points: Array<[number, number]> = [];
  for (const entity of entities) {
    collectDxfPoints(entity, points);
  }
  if (points.length === 0) {
    return null;
  }

  let minX = points[0][0];
  let minY = points[0][1];
  let maxX = minX;
  let maxY = minY;
  for (const [x, y] of points) {
    minX = Math.min(minX, x);
    minY = Math.min(minY, y);
    maxX = Math.max(maxX, x);
    maxY = Math.max(maxY, y);
  }
  return { minX, minY, maxX, maxY };
}

interface Bounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

function collectDxfPoints(entity: DxfEntity, out: Array<[number, number]>): void {
  if (hasNumbers(entity, "x1", "y1", "x2", "y2")) {
    out.push([entity.x1, entity.y1], [entity.x2, entity.y2]);
  }
  if (hasNumbers(entity, "x3", "y3", "x4", "y4")) {
    out.push([entity.x3, entity.y3], [entity.x4, entity.y4]);
  }
  if (hasNumbers(entity, "x", "y")) {
    out.push([entity.x, entity.y]);
  }
  if (hasNumbers(entity, "center_x", "center_y", "radius")) {
    out.push(
      [entity.center_x - entity.radius, entity.center_y - entity.radius],
      [entity.center_x + entity.radius, entity.center_y + entity.radius],
    );
  }
}

function drawGrid(context: CanvasRenderingContext2D, width: number, height: number): void {
  context.strokeStyle = "#d9dde5";
  context.lineWidth = 1;
  context.beginPath();
  for (let x = 0; x <= width; x += 48) {
    context.moveTo(x, 0);
    context.lineTo(x, height);
  }
  for (let y = 0; y <= height; y += 48) {
    context.moveTo(0, y);
    context.lineTo(width, y);
  }
  context.stroke();
}

function drawEmptyPreview(context: CanvasRenderingContext2D, width: number, height: number): void {
  drawGrid(context, width, height);
  context.fillStyle = "#6b7280";
  context.font = "14px system-ui, sans-serif";
  context.textAlign = "center";
  context.fillText("No JWW data", width / 2, height / 2);
}

function hasNumbers<T extends string>(
  entity: DxfEntity,
  ...keys: T[]
): entity is DxfEntity & Record<T, number> {
  return keys.every((key) => typeof entity[key as keyof DxfEntity] === "number");
}

function firstPoint(entity: JwwEntity): string {
  const x = entity.start_x ?? entity.center_x ?? entity.x ?? entity.ref_x ?? entity.point1_x;
  const y = entity.start_y ?? entity.center_y ?? entity.y ?? entity.ref_y ?? entity.point1_y;
  if (typeof x !== "number" || typeof y !== "number") {
    return "-";
  }
  return `${x.toFixed(2)}, ${y.toFixed(2)}`;
}

function metricMarkup(items: Array<[string, string]>): string {
  return items
    .map(
      ([label, value]) => `
        <div class="metric">
          <span>${label}</span>
          <strong>${escapeHtml(value)}</strong>
        </div>
      `,
    )
    .join("");
}

function panelTitle(title: string): string {
  return `<div class="panel-title">${title}</div>`;
}

function emptyLine(): string {
  return `<div class="muted line">-</div>`;
}

function colorFor(color: number): string {
  const palette = [
    "#111827",
    "#b91c1c",
    "#15803d",
    "#0f766e",
    "#1d4ed8",
    "#7c2d12",
    "#6d28d9",
    "#374151",
  ];
  return palette[Math.abs(color) % palette.length];
}

function degToRad(value: number): number {
  return (value * Math.PI) / 180;
}

function formatNumber(value: number): string {
  return new Intl.NumberFormat("ja-JP").format(value);
}

function formatBytes(value: number): string {
  if (value < 1024) {
    return `${value} B`;
  }
  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`;
  }
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

render();
window.addEventListener("resize", () => renderPreview(parsedState));
