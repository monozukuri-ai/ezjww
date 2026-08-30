pub mod diagnostics;
pub mod dxf;
pub mod error;
pub mod header;
pub mod model;
pub mod parser;
pub mod reader;
pub mod schema;

pub use diagnostics::{
    DecodeDiagnostic, DecodeDiagnosticDetails, Diagnostic, DiagnosticDetails,
    TruncationDiagnosticDetails, CP932_DECODE_REPLACED, ENTITY_LIST_TRUNCATED,
};
pub use dxf::{
    convert_document, convert_document_with_options, document_to_string,
    document_to_string_with_version, write_document_to_file, write_document_to_file_with_version,
    ConvertOptions, DxfArc, DxfBlock, DxfCircle, DxfDocument, DxfEllipse, DxfEntity,
    DxfFilledPolygon, DxfInsert, DxfLayer, DxfLine, DxfPoint, DxfSolid, DxfTargetVersion, DxfText,
    DxfVertex,
};
pub use error::JwwError;
pub use header::{
    is_jww_signature, parse_header, read_header_from_file, JwwHeader, JwwPalette, LayerGroupHeader,
    LayerHeader,
};
pub use model::{
    collect_entity_coordinates, collect_metadata_settings, coordinates_bbox,
    metadata_setting_from_text, Arc, Block, BlockDef, CircleSolid, Coord2D, Dimension, Entity,
    EntityBase, JwwDocument, Line, MetadataSetting, Point, Solid, Text,
};
pub use parser::{
    block_def_name_map, entity_counts, parse_document, parse_document_with_diagnostics,
    read_document_from_file, read_document_from_file_with_diagnostics, resolve_block_name,
    validate_block_references, BlockReferenceValidation, ParsedJwwDocument,
};
pub use schema::{
    jww_document_to_dto, jww_document_to_dto_with_diagnostics, BlockReferenceValidationDto,
    DxfDocumentDto, JwwDocumentDto,
};
