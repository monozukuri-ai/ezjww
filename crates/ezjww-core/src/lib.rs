pub mod diagnostics;
pub mod dxf;
pub mod error;
pub mod header;
pub mod model;
pub mod parser;
pub mod reader;
pub mod schema;

pub use diagnostics::{DecodeDiagnostic, DecodeDiagnosticDetails, CP932_DECODE_REPLACED};
pub use dxf::{
    convert_document, convert_document_with_options, document_to_string, write_document_to_file,
    ConvertOptions, DxfArc, DxfBlock, DxfCircle, DxfDocument, DxfEllipse, DxfEntity,
    DxfFilledPolygon, DxfInsert, DxfLayer, DxfLine, DxfPoint, DxfSolid, DxfText, DxfVertex,
};
pub use error::JwwError;
pub use header::{
    is_jww_signature, parse_header, read_header_from_file, JwwHeader, LayerGroupHeader, LayerHeader,
};
pub use model::{
    collect_entity_coordinates, coordinates_bbox, Arc, Block, BlockDef, CircleSolid, Coord2D,
    Dimension, Entity, EntityBase, JwwDocument, Line, Point, Solid, Text,
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
