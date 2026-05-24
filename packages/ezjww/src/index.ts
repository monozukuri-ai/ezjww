import * as wasm from "../wasm/ezjww_wasm.js";

export interface LayerHeader {
  state: number;
  protect: number;
  name: string;
}

export interface LayerGroupHeader {
  state: number;
  write_layer: number;
  scale: number;
  protect: number;
  name: string;
  layers: LayerHeader[];
}

export interface JwwHeader {
  version: number;
  memo: string;
  paper_size: number;
  write_layer_group: number;
  layer_groups: LayerGroupHeader[];
}

export interface EntityBase {
  group: number;
  pen_style: number;
  pen_color: number;
  pen_width: number;
  layer: number;
  layer_group: number;
  flag: number;
}

export interface LinePayload {
  start_x: number;
  start_y: number;
  end_x: number;
  end_y: number;
}

export interface PointPayload {
  x: number;
  y: number;
  is_temporary: boolean;
  code: number;
  angle: number;
  scale: number;
}

export interface TextPayload {
  start_x: number;
  start_y: number;
  end_x: number;
  end_y: number;
  text_type: number;
  size_x: number;
  size_y: number;
  spacing: number;
  angle: number;
  font_name: string;
  content: string;
}

export interface JwwEntity {
  type: string;
  base: EntityBase;
  start_x?: number;
  start_y?: number;
  end_x?: number;
  end_y?: number;
  center_x?: number;
  center_y?: number;
  radius?: number;
  start_angle?: number;
  arc_angle?: number;
  tilt_angle?: number;
  flatness?: number;
  is_full_circle?: boolean;
  x?: number;
  y?: number;
  is_temporary?: boolean;
  code?: number;
  angle?: number;
  scale?: number;
  text_type?: number;
  size_x?: number;
  size_y?: number;
  spacing?: number;
  font_name?: string;
  content?: string;
  point1_x?: number;
  point1_y?: number;
  point2_x?: number;
  point2_y?: number;
  point3_x?: number;
  point3_y?: number;
  point4_x?: number;
  point4_y?: number;
  color?: number | null;
  ref_x?: number;
  ref_y?: number;
  scale_x?: number;
  scale_y?: number;
  rotation?: number;
  def_number?: number;
  line?: LinePayload;
  text?: TextPayload;
  sxf_mode?: number | null;
  aux_lines?: LinePayload[];
  aux_points?: PointPayload[];
}

export interface BlockDef {
  number: number;
  is_referenced: boolean;
  name: string;
  base: EntityBase;
  entities: JwwEntity[];
}

export interface BlockReferenceValidation {
  total_references: number;
  resolved_references: number;
  unresolved_def_numbers: number[];
  has_unresolved: boolean;
}

export interface JwwDocument {
  header: JwwHeader;
  entities: JwwEntity[];
  block_defs: BlockDef[];
  block_def_names: Record<string, string>;
  entity_counts: Record<string, number>;
  validation: BlockReferenceValidation;
}

export interface DxfLayer {
  name: string;
  color: number;
  line_type: string;
  frozen: boolean;
  locked: boolean;
}

export interface DxfEntity {
  type: string;
  layer: string;
  color: number;
  line_type: string;
  x1?: number;
  y1?: number;
  x2?: number;
  y2?: number;
  center_x?: number;
  center_y?: number;
  radius?: number;
  start_angle?: number;
  end_angle?: number;
  major_axis_x?: number;
  major_axis_y?: number;
  minor_ratio?: number;
  start_param?: number;
  end_param?: number;
  x?: number;
  y?: number;
  height?: number;
  rotation?: number;
  content?: string;
  style?: string;
  x3?: number;
  y3?: number;
  x4?: number;
  y4?: number;
  block_name?: string;
  scale_x?: number;
  scale_y?: number;
}

export interface DxfBlock {
  name: string;
  base_x: number;
  base_y: number;
  entities: DxfEntity[];
}

export interface DxfDocument {
  layers: DxfLayer[];
  entities: DxfEntity[];
  blocks: DxfBlock[];
  unsupported_entities: string[];
}

export interface DxfOptions {
  explodeInserts?: boolean;
  maxBlockNesting?: number;
}

export type JwwInput = Uint8Array | ArrayBuffer | ArrayBufferView;

export function isJwwFile(input: JwwInput): boolean {
  return wasm.isJwwFile(toUint8Array(input));
}

export function readHeader(input: JwwInput): JwwHeader {
  return wasm.readHeader(toUint8Array(input)) as JwwHeader;
}

export function readDocument(input: JwwInput): JwwDocument {
  return wasm.readDocument(toUint8Array(input)) as JwwDocument;
}

export function readDxfDocument(
  input: JwwInput,
  options: DxfOptions = {},
): DxfDocument {
  const normalized = normalizeDxfOptions(options);
  return wasm.readDxfDocument(
    toUint8Array(input),
    normalized.explodeInserts,
    normalized.maxBlockNesting,
  ) as DxfDocument;
}

export function readDxfString(
  input: JwwInput,
  options: DxfOptions = {},
): string {
  const normalized = normalizeDxfOptions(options);
  return wasm.readDxfString(
    toUint8Array(input),
    normalized.explodeInserts,
    normalized.maxBlockNesting,
  ) as string;
}

export const toDxfString = readDxfString;

function normalizeDxfOptions(options: DxfOptions): Required<DxfOptions> {
  const maxBlockNesting = options.maxBlockNesting ?? 32;
  if (!Number.isInteger(maxBlockNesting) || maxBlockNesting < 1) {
    throw new RangeError("maxBlockNesting must be an integer >= 1");
  }
  return {
    explodeInserts: options.explodeInserts ?? false,
    maxBlockNesting,
  };
}

function toUint8Array(input: JwwInput): Uint8Array {
  if (input instanceof Uint8Array) {
    return input;
  }
  if (input instanceof ArrayBuffer) {
    return new Uint8Array(input);
  }
  return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
}
