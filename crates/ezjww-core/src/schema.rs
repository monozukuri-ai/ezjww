use std::collections::BTreeMap;

use serde::Serialize;

use crate::dxf::DxfDocument;
use crate::header::JwwHeader;
use crate::model::{BlockDef, JwwDocument};
use crate::parser::{block_def_name_map, entity_counts, validate_block_references};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlockReferenceValidationDto {
    pub total_references: usize,
    pub resolved_references: usize,
    pub unresolved_def_numbers: Vec<u32>,
    pub has_unresolved: bool,
}

#[derive(Debug, Serialize)]
pub struct JwwDocumentDto<'a> {
    pub header: &'a JwwHeader,
    pub entities: &'a [crate::model::Entity],
    pub block_defs: &'a [BlockDef],
    pub block_def_names: BTreeMap<u32, String>,
    pub entity_counts: BTreeMap<String, usize>,
    pub validation: BlockReferenceValidationDto,
}

pub type DxfDocumentDto = DxfDocument;

pub fn jww_document_to_dto(document: &JwwDocument) -> JwwDocumentDto<'_> {
    let block_def_names = block_def_name_map(&document.block_defs)
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let entity_counts = entity_counts(&document.entities)
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect::<BTreeMap<_, _>>();
    let validation = validate_block_references(document);

    JwwDocumentDto {
        header: &document.header,
        entities: &document.entities,
        block_defs: &document.block_defs,
        block_def_names,
        entity_counts,
        validation: BlockReferenceValidationDto {
            total_references: validation.total_references,
            resolved_references: validation.resolved_references,
            has_unresolved: validation.has_unresolved(),
            unresolved_def_numbers: validation.unresolved_def_numbers,
        },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::dxf::{DxfEntity, DxfInsert};
    use crate::header::{JwwHeader, LayerGroupHeader, LayerHeader};
    use crate::model::{Dimension, Entity, EntityBase, JwwDocument, Line, Point, Text};

    use super::jww_document_to_dto;

    #[test]
    fn serializes_jww_entity_as_flat_tagged_object() {
        let entity = Entity::Line(Line {
            base: EntityBase::default(),
            start_x: 1.0,
            start_y: 2.0,
            end_x: 3.0,
            end_y: 4.0,
        });

        let value = serde_json::to_value(entity).unwrap();

        assert_eq!(value["type"], "LINE");
        assert_eq!(value["start_x"], 1.0);
        assert!(value.get("Line").is_none());
        assert!(value["base"].is_object());
    }

    #[test]
    fn serializes_dimension_nested_payloads_without_base() {
        let base = EntityBase::default();
        let line = Line {
            base,
            start_x: 1.0,
            start_y: 2.0,
            end_x: 3.0,
            end_y: 4.0,
        };
        let text = Text {
            base,
            start_x: 5.0,
            start_y: 6.0,
            end_x: 7.0,
            end_y: 8.0,
            text_type: 1,
            size_x: 2.5,
            size_y: 3.5,
            spacing: 0.5,
            angle: 0.0,
            font_name: "Gothic".to_string(),
            content: "1000".to_string(),
        };
        let point = Point {
            base,
            x: 9.0,
            y: 10.0,
            is_temporary: false,
            code: 0,
            angle: 0.0,
            scale: 0.0,
        };
        let entity = Entity::Dimension(Dimension {
            base,
            line: line.clone(),
            text,
            sxf_mode: Some(2),
            aux_lines: vec![line],
            aux_points: vec![point],
        });

        let value = serde_json::to_value(entity).unwrap();

        assert_eq!(value["type"], "DIMENSION");
        assert_eq!(value["sxf_mode"], 2);
        assert!(value["line"].get("base").is_none());
        assert!(value["text"].get("base").is_none());
        assert!(value["aux_lines"][0].get("base").is_none());
        assert!(value["aux_points"][0].get("base").is_none());
    }

    #[test]
    fn jww_document_dto_includes_python_metadata_fields() {
        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![Entity::Line(Line {
                base: EntityBase::default(),
                start_x: 1.0,
                start_y: 2.0,
                end_x: 3.0,
                end_y: 4.0,
            })],
            block_defs: Vec::new(),
        };

        let value = serde_json::to_value(jww_document_to_dto(&doc)).unwrap();

        assert!(value["header"].is_object());
        assert_eq!(value["entities"][0]["type"], "LINE");
        assert_eq!(value["block_defs"], json!([]));
        assert_eq!(value["entity_counts"]["LINE"], 1);
        assert_eq!(value["validation"]["has_unresolved"], false);
        assert!(value["block_def_names"].is_object());
    }

    #[test]
    fn serializes_dxf_entity_as_flat_tagged_object() {
        let entity = DxfEntity::Insert(DxfInsert {
            layer: "#lv0".to_string(),
            color: 7,
            line_type: "CONTINUOUS".to_string(),
            block_name: "BLK".to_string(),
            x: 1.0,
            y: 2.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        });

        let value = serde_json::to_value(entity).unwrap();

        assert_eq!(value["type"], "INSERT");
        assert_eq!(value["block_name"], "BLK");
        assert!(value.get("Insert").is_none());
    }

    fn empty_header() -> JwwHeader {
        JwwHeader {
            version: 600,
            memo: String::new(),
            paper_size: 0,
            write_layer_group: 0,
            layer_groups: std::array::from_fn(|_| LayerGroupHeader {
                layers: std::array::from_fn(|_| LayerHeader::default()),
                ..LayerGroupHeader::default()
            }),
        }
    }
}
