pub mod dxf;
pub mod error;
pub mod header;
pub mod model;
pub mod parser;
pub mod reader;
pub mod schema;

pub use dxf::{
    convert_document, convert_document_with_options, document_to_string, write_document_to_file,
    ConvertOptions, DxfArc, DxfBlock, DxfCircle, DxfDocument, DxfEllipse, DxfEntity, DxfInsert,
    DxfLayer, DxfLine, DxfPoint, DxfSolid, DxfText,
};
pub use error::JwwError;
pub use header::{
    is_jww_signature, parse_header, read_header_from_file, JwwHeader, LayerGroupHeader, LayerHeader,
};
pub use model::{
    collect_entity_coordinates, coordinates_bbox, Arc, Block, BlockDef, Coord2D, Dimension, Entity,
    EntityBase, JwwDocument, Line, Point, Solid, Text,
};
pub use parser::{
    block_def_name_map, entity_counts, parse_document, read_document_from_file, resolve_block_name,
    validate_block_references, BlockReferenceValidation,
};
pub use schema::{
    jww_document_to_dto, BlockReferenceValidationDto, DxfDocumentDto, JwwDocumentDto,
};
