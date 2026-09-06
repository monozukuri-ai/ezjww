from typing import Literal, TypedDict

from typing_extensions import TypeAlias

class LayerHeader(TypedDict):
    state: int
    protect: int
    name: str

class LayerGroupHeader(TypedDict):
    state: int
    write_layer: int
    scale: float
    protect: int
    name: str
    layers: list[LayerHeader]

class JwwPalette(TypedDict):
    pen_colors: list[int]
    extended_colors: list[int] | None

class JwwHeader(TypedDict):
    version: int
    memo: str
    paper_size: int
    write_layer_group: int
    layer_groups: list[LayerGroupHeader]
    palette: JwwPalette | None

class EntityBase(TypedDict):
    group: int
    pen_style: int
    pen_color: int
    pen_width: int
    layer: int
    layer_group: int
    flag: int

class LinePayload(TypedDict):
    start_x: float
    start_y: float
    end_x: float
    end_y: float

class PointPayload(TypedDict):
    x: float
    y: float
    is_temporary: bool
    code: int
    angle: float
    scale: float

class TextPayload(TypedDict):
    start_x: float
    start_y: float
    end_x: float
    end_y: float
    text_type: int
    size_x: float
    size_y: float
    spacing: float
    angle: float
    font_name: str
    content: str

class MetadataSetting(TypedDict):
    entity_index: int
    key: str
    value: str
    raw: str

class JwwEntity(TypedDict, total=False):
    type: str
    base: EntityBase
    start_x: float
    start_y: float
    end_x: float
    end_y: float
    center_x: float
    center_y: float
    radius: float
    start_angle: float
    arc_angle: float
    tilt_angle: float
    flatness: float
    is_full_circle: bool
    x: float
    y: float
    is_temporary: bool
    code: int
    angle: float
    scale: float
    text_type: int
    size_x: float
    size_y: float
    spacing: float
    font_name: str
    content: str
    point1_x: float
    point1_y: float
    point2_x: float
    point2_y: float
    point3_x: float
    point3_y: float
    point4_x: float
    point4_y: float
    color: int | None
    solid_mode: float
    ref_x: float
    ref_y: float
    scale_x: float
    scale_y: float
    rotation: float
    def_number: int
    block_name: str | None
    line: LinePayload
    text: TextPayload
    sxf_mode: int | None
    aux_lines: list[LinePayload]
    aux_points: list[PointPayload]

class BlockDef(TypedDict):
    number: int
    is_referenced: bool
    name: str
    base: EntityBase
    entities: list[JwwEntity]

class BlockReferenceValidation(TypedDict):
    total_references: int
    resolved_references: int
    unresolved_def_numbers: list[int]
    has_unresolved: bool

class DecodeDiagnosticDetails(TypedDict):
    encoding: str
    field: str
    byte_offset: int
    byte_length: int
    replacement_characters: int
    had_errors: bool

class TruncationDiagnosticDetails(TypedDict):
    byte_offset: int
    expected_entities: int
    parsed_entities: int
    error: str

class DecodeDiagnostic(TypedDict):
    code: str
    severity: str
    message: str
    action: str
    details: DecodeDiagnosticDetails | TruncationDiagnosticDetails

class DxfWriteReport(TypedDict):
    target_version: str
    source_version: int
    source_entities: int
    source_entity_counts: dict[str, int]
    source_block_definitions: int
    source_block_entities: int
    all_source_entity_counts: dict[str, int]
    converted_entities: int
    converted_block_entities: int
    converted_entity_counts: dict[str, int]
    unsupported_entity_counts: dict[str, int]
    normalized_layer_names: int
    normalized_block_names: int
    diagnostics: list[DecodeDiagnostic]
    validation: BlockReferenceValidation

class JwwDocument(TypedDict):
    header: JwwHeader
    entities: list[JwwEntity]
    metadata_settings: list[MetadataSetting]
    block_defs: list[BlockDef]
    block_def_names: dict[int, str]
    entity_counts: dict[str, int]
    validation: BlockReferenceValidation
    diagnostics: list[DecodeDiagnostic]

class DxfLayer(TypedDict):
    name: str
    color: int
    line_type: str
    frozen: bool
    locked: bool

class DxfVertex(TypedDict):
    x: float
    y: float

class DxfEntity(TypedDict, total=False):
    type: str
    layer: str
    color: int
    line_type: str
    line_weight: int
    x1: float
    y1: float
    x2: float
    y2: float
    center_x: float
    center_y: float
    radius: float
    start_angle: float
    end_angle: float
    major_axis_x: float
    major_axis_y: float
    minor_ratio: float
    start_param: float
    end_param: float
    x: float
    y: float
    end_x: float
    end_y: float
    height: float
    width_factor: float
    rotation: float
    content: str
    style: str
    x3: float
    y3: float
    x4: float
    y4: float
    points: list[DxfVertex]
    block_name: str
    scale_x: float
    scale_y: float

class DxfBlock(TypedDict):
    name: str
    base_x: float
    base_y: float
    entities: list[DxfEntity]

class _DxfDocumentGeometry(TypedDict):
    layers: list[DxfLayer]
    entities: list[DxfEntity]
    blocks: list[DxfBlock]
    unsupported_entities: list[str]

class DxfDocument(_DxfDocumentGeometry, total=False):
    jwc_conversion_report: JwcConversionReport
    text_width_factors: list[float]

JwcCoordinateSpace: TypeAlias = Literal["paper_millimeters", "model_millimeters"]

class JwcSection(TypedDict):
    byte_offset: int
    byte_length: int

class JwcLayout(TypedDict):
    lines: JwcSection
    arcs: JwcSection
    text_records: JwcSection
    string_pool: JwcSection
    points: JwcSection
    names: JwcSection

class JwcName(TypedDict):
    text: str
    raw_bytes: list[int]
    byte_offset: int

class JwcLayerState(TypedDict):
    editable: bool
    visible: bool
    protected: bool

class JwcLayer(TypedDict):
    state: JwcLayerState
    name: JwcName

class JwcLayerGroup(TypedDict):
    state: JwcLayerState
    scale: float
    write_layer: int
    name: JwcName
    layers: list[JwcLayer]

class JwcTextPreset(TypedDict):
    pen_color: int
    width_tenths: int
    height_tenths: int
    spacing_tenths: int

class JwcTemporaryPoint(TypedDict):
    array_index: int
    raw_x: float
    raw_y: float
    layer_group: int
    layer: int

class JwcEntityCounts(TypedDict):
    lines: int
    arcs: int
    texts: int
    points: int
    temporary_points: int

class JwcHeader(TypedDict):
    profile_id: Literal["fixed2421_csv32_v1"]
    source_version: str | None
    counts: JwcEntityCounts
    paper: Literal["A0", "A1", "A2", "A3", "A4"]
    coordinate_extent: float
    write_layer_group: int
    layer_groups: list[JwcLayerGroup]
    text_presets: list[JwcTextPreset]
    temporary_points: list[JwcTemporaryPoint]
    layout: JwcLayout
    raw_fixed_header: list[int]
    diagnostics: list[DecodeDiagnostic]

class JwcLayerAddress(TypedDict):
    group: int
    layer: int

class JwcCoord(TypedDict):
    x: float
    y: float

class JwcEntitySource(TypedDict):
    spans: list[JwcSection]
    raw_bytes: list[int]

class JwcStrokeAttributes(TypedDict):
    pen_style: int
    pen_color: int
    layer: JwcLayerAddress
    flags_raw: int

class JwcLine(TypedDict):
    type: Literal["line"]
    source: JwcEntitySource
    start: JwcCoord
    end: JwcCoord
    attributes: JwcStrokeAttributes

class JwcArc(TypedDict):
    type: Literal["arc"]
    source: JwcEntitySource
    center: JwcCoord
    radius: float
    flatness: float
    start_angle_degrees: float
    end_angle_degrees: float
    tilt_angle_degrees: float
    is_full_circle: bool
    attributes: JwcStrokeAttributes

class JwcPoint(TypedDict):
    type: Literal["point"]
    source: JwcEntitySource
    position: JwcCoord
    layer: JwcLayerAddress
    pen_color: int
    flags_raw: int

class JwcTemporaryPointEntity(TypedDict):
    type: Literal["temporary_point"]
    source: JwcEntitySource
    position: JwcCoord
    layer: JwcLayerAddress
    array_index: int

class JwcText(TypedDict):
    type: Literal["text"]
    source: JwcEntitySource
    start: JwcCoord
    end: JwcCoord
    layer: JwcLayerAddress
    text_preset: int
    content: str
    raw_content: list[int]
    string_source: JwcSection

JwcEntity: TypeAlias = JwcLine | JwcArc | JwcPoint | JwcTemporaryPointEntity | JwcText

class JwcDocument(TypedDict):
    profile_id: Literal["fixed2421_basic_v1"]
    header: JwcHeader
    entities: list[JwcEntity]
    diagnostics: list[DecodeDiagnostic]
    entity_counts: dict[str, int]

class JwcEntityMapping(TypedDict):
    source_entity_index: int
    source_spans: list[JwcSection]
    applied_group_scale: float

class JwcConversionNotice(TypedDict):
    entity_index: int | None
    field: str
    kind: Literal["default", "derived", "dxf_limitation"]
    detail: str

class JwcConversionReport(TypedDict):
    coordinate_space: JwcCoordinateSpace
    rendering_policy: str
    aci_colors: list[int]
    mappings: list[JwcEntityMapping]
    notices: list[JwcConversionNotice]
    diagnostics: list[DecodeDiagnostic]

class JwwCadDocument(TypedDict):
    format: Literal["jww"]
    document: JwwDocument

class JwcCadDocument(TypedDict):
    format: Literal["jwc"]
    document: JwcDocument

CadDocument: TypeAlias = JwwCadDocument | JwcCadDocument

class JwcDxfWriteReport(TypedDict):
    source_format: Literal["jwc"]
    source_profile: str
    source_version: None
    target_version: str
    source_entities: int
    source_entity_counts: dict[str, int]
    all_source_entity_counts: dict[str, int]
    source_block_definitions: int
    source_block_entities: int
    converted_entities: int
    converted_block_entities: int
    converted_entity_counts: dict[str, int]
    unsupported_entity_counts: dict[str, int]
    normalized_layer_names: int
    normalized_block_names: int
    diagnostics: list[DecodeDiagnostic]
    validation: BlockReferenceValidation
    jwc_conversion_report: JwcConversionReport

def hello_from_bin() -> str: ...
def is_jww_file(path: str) -> bool: ...
def read_header(path: str) -> JwwHeader: ...
def read_document(path: str) -> JwwDocument: ...
def is_jwc_file(path: str) -> bool: ...
def detect_file_format(path: str) -> Literal["jww", "jwc"] | None: ...
def read_jwc_header(path: str) -> JwcHeader: ...
def read_jwc_document(path: str) -> JwcDocument: ...
def read_cad_document(path: str) -> CadDocument: ...
def read_dxf_document(
    path: str,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
    text_em_scale: float = 1.0,
    *,
    jwc_coordinates: JwcCoordinateSpace = "paper_millimeters",
) -> DxfDocument: ...
def read_dxf_string(
    path: str,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
    target_version: str = "AC1015",
    text_em_scale: float = 1.0,
    *,
    jwc_coordinates: JwcCoordinateSpace = "paper_millimeters",
) -> str: ...
def write_dxf(
    path: str,
    output_path: str,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
    target_version: str = "AC1015",
    text_em_scale: float = 1.0,
    *,
    jwc_coordinates: JwcCoordinateSpace = "paper_millimeters",
) -> None: ...
def write_dxf_with_report(
    path: str,
    output_path: str,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
    target_version: str = "AC1015",
    text_em_scale: float = 1.0,
    *,
    jwc_coordinates: JwcCoordinateSpace = "paper_millimeters",
) -> DxfWriteReport | JwcDxfWriteReport: ...
