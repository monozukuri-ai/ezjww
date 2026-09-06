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

export interface JwwPalette {
  pen_colors: number[];
  extended_colors: number[] | null;
}

export interface JwwHeader {
  version: number;
  memo: string;
  paper_size: number;
  write_layer_group: number;
  layer_groups: LayerGroupHeader[];
  palette: JwwPalette | null;
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

export interface DecodeDiagnosticDetails {
  encoding: "cp932";
  field: string;
  byte_offset: number;
  byte_length: number;
  replacement_characters: number;
  had_errors: boolean;
}

export interface TruncationDiagnosticDetails {
  byte_offset: number;
  expected_entities: number;
  parsed_entities: number;
  error: string;
}

export interface DecodeDiagnostic {
  code: string;
  severity: "info" | "warning" | "error";
  message: string;
  action: string;
  details: DecodeDiagnosticDetails | TruncationDiagnosticDetails;
}

export interface JwwDocument {
  header: JwwHeader;
  entities: JwwEntity[];
  block_defs: BlockDef[];
  block_def_names: Record<string, string>;
  entity_counts: Record<string, number>;
  validation: BlockReferenceValidation;
  diagnostics: DecodeDiagnostic[];
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
  jwc_conversion_report?: JwcConversionReport;
  text_width_factors?: number[];
}

export interface DxfOptions {
  explodeInserts?: boolean;
  maxBlockNesting?: number;
  jwcCoordinates?: JwcCoordinateSpace;
  targetVersion?: "AC1015" | "AC1024";
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

export type CadInput = JwwInput;

export function isJwcFile(input: CadInput): boolean {
  return wasm.isJwcFile(toUint8Array(input));
}

export function detectFileFormat(input: CadInput): "jww" | "jwc" | null {
  return wasm.detectFileFormat(toUint8Array(input)) as "jww" | "jwc" | null;
}

export function readJwcHeader(input: CadInput): JwcHeader {
  return wasm.readJwcHeader(toUint8Array(input)) as JwcHeader;
}

export function readJwcDocument(input: CadInput): JwcDocument {
  return wasm.readJwcDocument(toUint8Array(input)) as JwcDocument;
}

export function readCadDocument(input: CadInput): CadDocument {
  return wasm.readCadDocument(toUint8Array(input)) as CadDocument;
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
    normalized.jwcCoordinates,
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
    normalized.jwcCoordinates,
    normalized.targetVersion,
  ) as string;
}

export const toDxfString = readDxfString;

function normalizeDxfOptions(options: DxfOptions): Required<DxfOptions> {
  const maxBlockNesting = options.maxBlockNesting ?? 32;
  if (!Number.isInteger(maxBlockNesting) || maxBlockNesting < 1) {
    throw new RangeError("maxBlockNesting must be an integer >= 1");
  }
  const jwcCoordinates = options.jwcCoordinates ?? "paper_millimeters";
  if (!["paper_millimeters", "model_millimeters"].includes(jwcCoordinates)) {
    throw new RangeError("jwcCoordinates must be paper_millimeters or model_millimeters");
  }
  const targetVersion = options.targetVersion ?? "AC1015";
  if (!["AC1015", "AC1024"].includes(targetVersion)) {
    throw new RangeError("targetVersion must be AC1015 or AC1024");
  }
  return {
    explodeInserts: options.explodeInserts ?? false,
    maxBlockNesting,
    jwcCoordinates,
    targetVersion,
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


export type JwcCoordinateSpace = "paper_millimeters" | "model_millimeters";

export interface JwcSection {
  byte_offset: number;
  byte_length: number;
}

export interface JwcLayout {
  lines: JwcSection;
  arcs: JwcSection;
  text_records: JwcSection;
  string_pool: JwcSection;
  points: JwcSection;
  names: JwcSection;
}

export interface JwcName {
  text: string;
  raw_bytes: number[];
  byte_offset: number;
}

export interface JwcLayerState {
  editable: boolean;
  visible: boolean;
  protected: boolean;
}

export interface JwcLayer {
  state: JwcLayerState;
  name: JwcName;
}

export interface JwcLayerGroup {
  state: JwcLayerState;
  scale: number;
  write_layer: number;
  name: JwcName;
  layers: JwcLayer[];
}

export interface JwcTextPreset {
  pen_color: number;
  width_tenths: number;
  height_tenths: number;
  spacing_tenths: number;
}

export interface JwcTemporaryPoint {
  array_index: number;
  raw_x: number;
  raw_y: number;
  layer_group: number;
  layer: number;
}

export interface JwcEntityCounts {
  lines: number;
  arcs: number;
  texts: number;
  points: number;
  temporary_points: number;
}

export interface JwcHeader {
  profile_id: "fixed2421_csv32_v1";
  source_version: string | null;
  counts: JwcEntityCounts;
  paper: "A0" | "A1" | "A2" | "A3" | "A4";
  coordinate_extent: number;
  write_layer_group: number;
  layer_groups: JwcLayerGroup[];
  text_presets: JwcTextPreset[];
  temporary_points: JwcTemporaryPoint[];
  layout: JwcLayout;
  raw_fixed_header: number[];
  diagnostics: DecodeDiagnostic[];
}

export interface JwcLayerAddress {
  group: number;
  layer: number;
}

export interface JwcCoord {
  x: number;
  y: number;
}

export interface JwcEntitySource {
  spans: JwcSection[];
  raw_bytes: number[];
}

export interface JwcStrokeAttributes {
  pen_style: number;
  pen_color: number;
  layer: JwcLayerAddress;
  flags_raw: number;
}

export interface JwcLine {
  type: "line";
  source: JwcEntitySource;
  start: JwcCoord;
  end: JwcCoord;
  attributes: JwcStrokeAttributes;
}

export interface JwcArc {
  type: "arc";
  source: JwcEntitySource;
  center: JwcCoord;
  radius: number;
  flatness: number;
  start_angle_degrees: number;
  end_angle_degrees: number;
  tilt_angle_degrees: number;
  is_full_circle: boolean;
  attributes: JwcStrokeAttributes;
}

export interface JwcPoint {
  type: "point";
  source: JwcEntitySource;
  position: JwcCoord;
  layer: JwcLayerAddress;
  pen_color: number;
  flags_raw: number;
}

export interface JwcTemporaryPointEntity {
  type: "temporary_point";
  source: JwcEntitySource;
  position: JwcCoord;
  layer: JwcLayerAddress;
  array_index: number;
}

export interface JwcText {
  type: "text";
  source: JwcEntitySource;
  start: JwcCoord;
  end: JwcCoord;
  layer: JwcLayerAddress;
  text_preset: number;
  content: string;
  raw_content: number[];
  string_source: JwcSection;
}


export type JwcEntity = JwcLine | JwcArc | JwcPoint | JwcTemporaryPointEntity | JwcText;

export interface JwcDocument {
  profile_id: "fixed2421_basic_v1";
  header: JwcHeader;
  entities: JwcEntity[];
  diagnostics: DecodeDiagnostic[];
  entity_counts: Record<string, number>;
}

export interface JwcEntityMapping {
  source_entity_index: number;
  source_spans: JwcSection[];
  applied_group_scale: number;
}

export interface JwcConversionNotice {
  entity_index: number | null;
  field: string;
  kind: "default" | "derived" | "dxf_limitation";
  detail: string;
}

export interface JwcConversionReport {
  coordinate_space: JwcCoordinateSpace;
  rendering_policy: string;
  aci_colors: number[];
  mappings: JwcEntityMapping[];
  notices: JwcConversionNotice[];
  diagnostics: DecodeDiagnostic[];
}

export interface JwwCadDocument {
  format: "jww";
  document: JwwDocument;
}

export interface JwcCadDocument {
  format: "jwc";
  document: JwcDocument;
}

export type CadDocument = JwwCadDocument | JwcCadDocument;
