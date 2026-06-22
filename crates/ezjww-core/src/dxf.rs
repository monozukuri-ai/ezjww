use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::f64::consts::PI;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;
use serde::Serializer;

use crate::model::{Arc, Block, BlockDef, CircleSolid, Entity, JwwDocument, Solid, Text};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfLayer {
    pub name: String,
    pub color: i32,
    pub line_type: String,
    pub frozen: bool,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfLine {
    pub layer: String,
    pub color: i32,
    pub line_type: String,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfCircle {
    pub layer: String,
    pub color: i32,
    pub line_type: String,
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfArc {
    pub layer: String,
    pub color: i32,
    pub line_type: String,
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfEllipse {
    pub layer: String,
    pub color: i32,
    pub line_type: String,
    pub center_x: f64,
    pub center_y: f64,
    pub major_axis_x: f64,
    pub major_axis_y: f64,
    pub minor_ratio: f64,
    pub start_param: f64,
    pub end_param: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfPoint {
    pub layer: String,
    pub color: i32,
    pub line_type: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfText {
    pub layer: String,
    pub color: i32,
    pub line_type: String,
    pub x: f64,
    pub y: f64,
    pub height: f64,
    pub rotation: f64,
    pub content: String,
    pub style: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfSolid {
    pub layer: String,
    pub color: i32,
    pub line_type: String,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub x3: f64,
    pub y3: f64,
    pub x4: f64,
    pub y4: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct DxfVertex {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfFilledPolygon {
    pub layer: String,
    pub color: i32,
    pub line_type: String,
    pub points: Vec<DxfVertex>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfInsert {
    pub layer: String,
    pub color: i32,
    pub line_type: String,
    pub block_name: String,
    pub x: f64,
    pub y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DxfEntity {
    Line(DxfLine),
    Circle(DxfCircle),
    Arc(DxfArc),
    Ellipse(DxfEllipse),
    Point(DxfPoint),
    Text(DxfText),
    Solid(DxfSolid),
    FilledPolygon(DxfFilledPolygon),
    Insert(DxfInsert),
}

#[derive(Serialize)]
struct TaggedDxfPayload<'a, T: Serialize + ?Sized> {
    #[serde(rename = "type")]
    entity_type: &'static str,
    #[serde(flatten)]
    payload: &'a T,
}

impl Serialize for DxfEntity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Line(v) => TaggedDxfPayload {
                entity_type: self.entity_type(),
                payload: v,
            }
            .serialize(serializer),
            Self::Circle(v) => TaggedDxfPayload {
                entity_type: self.entity_type(),
                payload: v,
            }
            .serialize(serializer),
            Self::Arc(v) => TaggedDxfPayload {
                entity_type: self.entity_type(),
                payload: v,
            }
            .serialize(serializer),
            Self::Ellipse(v) => TaggedDxfPayload {
                entity_type: self.entity_type(),
                payload: v,
            }
            .serialize(serializer),
            Self::Point(v) => TaggedDxfPayload {
                entity_type: self.entity_type(),
                payload: v,
            }
            .serialize(serializer),
            Self::Text(v) => TaggedDxfPayload {
                entity_type: self.entity_type(),
                payload: v,
            }
            .serialize(serializer),
            Self::Solid(v) => TaggedDxfPayload {
                entity_type: self.entity_type(),
                payload: v,
            }
            .serialize(serializer),
            Self::FilledPolygon(v) => TaggedDxfPayload {
                entity_type: self.entity_type(),
                payload: v,
            }
            .serialize(serializer),
            Self::Insert(v) => TaggedDxfPayload {
                entity_type: self.entity_type(),
                payload: v,
            }
            .serialize(serializer),
        }
    }
}

impl DxfEntity {
    pub fn entity_type(&self) -> &'static str {
        match self {
            Self::Line(_) => "LINE",
            Self::Circle(_) => "CIRCLE",
            Self::Arc(_) => "ARC",
            Self::Ellipse(_) => "ELLIPSE",
            Self::Point(_) => "POINT",
            Self::Text(_) => "TEXT",
            Self::Solid(_) => "SOLID",
            Self::FilledPolygon(_) => "FILLED_POLYGON",
            Self::Insert(_) => "INSERT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfBlock {
    pub name: String,
    pub base_x: f64,
    pub base_y: f64,
    pub entities: Vec<DxfEntity>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfDocument {
    pub layers: Vec<DxfLayer>,
    pub entities: Vec<DxfEntity>,
    pub blocks: Vec<DxfBlock>,
    pub unsupported_entities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ConvertOptions {
    pub explode_inserts: bool,
    pub max_block_nesting: usize,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            explode_inserts: false,
            max_block_nesting: 32,
        }
    }
}

pub fn convert_document(doc: &JwwDocument) -> DxfDocument {
    convert_document_with_options(doc, ConvertOptions::default())
}

pub fn convert_document_with_options(doc: &JwwDocument, options: ConvertOptions) -> DxfDocument {
    let layers = convert_layers(doc);
    let block_name_map = block_name_map(doc);
    let block_defs = block_defs_by_number(&doc.block_defs);

    let mut unsupported_entities = Vec::<String>::new();
    let entities = if options.explode_inserts {
        let mut expanding_stack = Vec::new();
        let mut context = ExplodeContext {
            doc,
            block_name_map: &block_name_map,
            block_defs: &block_defs,
            unsupported_entities: &mut unsupported_entities,
            options,
        };
        convert_entities_exploded(
            &mut context,
            &doc.entities,
            &Transform2D::identity(),
            &mut expanding_stack,
        )
    } else {
        convert_entities(
            doc,
            &doc.entities,
            &block_name_map,
            &mut unsupported_entities,
        )
    };
    let blocks = if options.explode_inserts {
        Vec::new()
    } else {
        convert_blocks(doc, &block_name_map, &mut unsupported_entities)
    };

    DxfDocument {
        layers,
        entities,
        blocks,
        unsupported_entities,
    }
}

pub fn document_to_string(doc: &DxfDocument) -> String {
    let mut writer = AsciiDxfWriter::new();
    writer.write_document(doc);
    writer.finish()
}

pub fn write_document_to_file(doc: &DxfDocument, path: impl AsRef<Path>) -> io::Result<()> {
    let data = document_to_string(doc);
    fs::write(path, data)
}

struct AsciiDxfWriter {
    out: String,
    next_handle: u32,
    block_record_order: Vec<String>,
    block_record_handles: BTreeMap<String, String>,
}

impl AsciiDxfWriter {
    fn new() -> Self {
        Self {
            out: String::with_capacity(16 * 1024),
            next_handle: 1,
            block_record_order: Vec::new(),
            block_record_handles: BTreeMap::new(),
        }
    }

    fn finish(self) -> String {
        self.out
    }

    fn write_document(&mut self, doc: &DxfDocument) {
        self.ensure_block_record_table(doc);
        self.write_header();
        self.write_tables(doc);
        self.write_blocks(doc);
        self.write_entities(doc);
        self.write_objects(doc);
        self.group_str(0, "EOF");
    }

    fn write_header(&mut self) {
        self.section_start("HEADER");
        self.group_str(9, "$ACADVER");
        self.group_str(1, "AC1015");
        self.group_str(9, "$DWGCODEPAGE");
        self.group_str(3, "ANSI_1252");
        self.group_str(9, "$MEASUREMENT");
        self.group_i32(70, 1);
        self.group_str(9, "$TEXTSTYLE");
        self.group_str(7, "STANDARD");
        self.group_str(9, "$CLAYER");
        self.group_str(8, "0");
        self.group_str(9, "$CELTYPE");
        self.group_str(6, "BYLAYER");
        self.group_str(9, "$CECOLOR");
        self.group_i32(62, 256);
        self.section_end();
    }

    fn write_tables(&mut self, doc: &DxfDocument) {
        self.section_start("TABLES");
        self.write_ltype_table(doc);
        self.write_layer_table(doc);
        self.write_style_table();
        self.write_block_record_table();
        self.section_end();
    }

    fn write_ltype_table(&mut self, doc: &DxfDocument) {
        let mut line_types = collect_line_types(doc);
        line_types.insert("BYLAYER".to_string());
        line_types.insert("BYBLOCK".to_string());
        line_types.insert("CONTINUOUS".to_string());

        self.group_str(0, "TABLE");
        self.group_str(2, "LTYPE");
        self.write_handle();
        self.group_i32(70, line_types.len() as i32);

        for name in line_types {
            let (description, pattern): (&str, &[f64]) = match name.as_str() {
                "BYLAYER" => ("", &[]),
                "BYBLOCK" => ("", &[]),
                "CONTINUOUS" => ("Solid line", &[]),
                "DASHED" => ("Dashed line", &[0.6, -0.3]),
                "DASHED2" => ("Dashed line x2", &[1.2, -0.6]),
                "DASHDOT" => ("Dash dot", &[0.6, -0.2, 0.1, -0.2]),
                "DASHDOT2" => ("Dash dot x2", &[1.2, -0.4, 0.2, -0.4]),
                "CENTER" => ("Center line", &[1.25, -0.25, 0.25, -0.25]),
                "CENTER2" => ("Center line x2", &[2.5, -0.5, 0.5, -0.5]),
                "DOT" => ("Dotted line", &[0.1, -0.1]),
                "DOT2" => ("Dotted line x2", &[0.2, -0.2]),
                _ => ("", &[]),
            };
            let length = pattern.iter().map(|v| v.abs()).sum::<f64>();
            self.group_str(0, "LTYPE");
            self.write_handle();
            self.group_str(2, &name);
            self.group_i32(70, 0);
            self.group_str(3, description);
            self.group_i32(72, 65);
            self.group_i32(73, pattern.len() as i32);
            self.group_f64(40, length);
            for value in pattern {
                self.group_f64(49, *value);
            }
        }

        self.group_str(0, "ENDTAB");
    }

    fn write_layer_table(&mut self, doc: &DxfDocument) {
        let mut layers = BTreeMap::<String, DxfLayer>::new();
        for layer in &doc.layers {
            layers
                .entry(layer.name.clone())
                .or_insert_with(|| layer.clone());
        }

        self.group_str(0, "TABLE");
        self.group_str(2, "LAYER");
        self.write_handle();
        self.group_i32(70, (layers.len() + 1) as i32);

        self.group_str(0, "LAYER");
        self.write_handle();
        self.group_str(2, "0");
        self.group_i32(70, 0);
        self.group_i32(62, 7);
        self.group_str(6, "CONTINUOUS");

        for layer in layers.values() {
            let mut flags = 0;
            if layer.frozen {
                flags |= 1;
            }
            if layer.locked {
                flags |= 4;
            }
            self.group_str(0, "LAYER");
            self.write_handle();
            self.group_str(2, &escape_unicode(&layer.name));
            self.group_i32(70, flags);
            self.group_i32(62, layer.color);
            self.group_str(6, &layer.line_type);
        }

        self.group_str(0, "ENDTAB");
    }

    fn write_style_table(&mut self) {
        self.group_str(0, "TABLE");
        self.group_str(2, "STYLE");
        self.write_handle();
        self.group_i32(70, 1);
        self.group_str(0, "STYLE");
        self.write_handle();
        self.group_str(2, "STANDARD");
        self.group_i32(70, 0);
        self.group_f64(40, 0.0);
        self.group_f64(41, 1.0);
        self.group_f64(50, 0.0);
        self.group_i32(71, 0);
        self.group_f64(42, 2.5);
        self.group_str(3, "txt");
        self.group_str(4, "");
        self.group_str(0, "ENDTAB");
    }

    fn write_block_record_table(&mut self) {
        self.group_str(0, "TABLE");
        self.group_str(2, "BLOCK_RECORD");
        self.write_handle();
        self.group_i32(70, self.block_record_order.len() as i32);

        let names = self.block_record_order.clone();
        for name in names {
            let handle = self
                .block_record_handles
                .get(&name)
                .cloned()
                .expect("BLOCK_RECORD handle should exist");
            self.group_str(0, "BLOCK_RECORD");
            self.group_str(5, &handle);
            self.group_str(330, "0");
            self.group_str(100, "AcDbSymbolTableRecord");
            self.group_str(100, "AcDbBlockTableRecord");
            self.group_str(2, &escape_unicode(&name));
        }

        self.group_str(0, "ENDTAB");
    }

    fn write_blocks(&mut self, doc: &DxfDocument) {
        self.section_start("BLOCKS");
        let model_owner = self.block_record_handle("*Model_Space").map(str::to_string);
        self.write_block_definition("*Model_Space", 0.0, 0.0, &[], model_owner.as_deref());

        let paper_owner = self.block_record_handle("*Paper_Space").map(str::to_string);
        self.write_block_definition("*Paper_Space", 0.0, 0.0, &[], paper_owner.as_deref());

        for block in &doc.blocks {
            let owner = self.block_record_handle(&block.name).map(str::to_string);
            self.write_block_definition(
                &block.name,
                block.base_x,
                block.base_y,
                &block.entities,
                owner.as_deref(),
            );
        }
        self.section_end();
    }

    fn write_entities(&mut self, doc: &DxfDocument) {
        self.section_start("ENTITIES");
        let owner = self.block_record_handle("*Model_Space").map(str::to_string);
        for entity in &doc.entities {
            self.write_entity(entity, owner.as_deref());
        }
        self.section_end();
    }

    fn write_objects(&mut self, _doc: &DxfDocument) {
        self.section_start("OBJECTS");
        self.group_str(0, "DICTIONARY");
        self.write_handle();
        self.group_str(330, "0");
        self.group_str(100, "AcDbDictionary");
        self.group_i32(281, 1);
        self.section_end();
    }

    fn write_block_definition(
        &mut self,
        name: &str,
        base_x: f64,
        base_y: f64,
        entities: &[DxfEntity],
        owner_handle: Option<&str>,
    ) {
        let block_name = escape_unicode(name);
        self.group_str(0, "BLOCK");
        self.write_handle();
        if let Some(owner) = owner_handle {
            self.group_str(330, owner);
        }
        self.group_str(100, "AcDbEntity");
        self.group_str(8, "0");
        self.group_str(100, "AcDbBlockBegin");
        self.group_str(2, &block_name);
        self.group_i32(70, 0);
        self.group_f64(10, base_x);
        self.group_f64(20, base_y);
        self.group_f64(30, 0.0);
        self.group_str(3, &block_name);
        self.group_str(1, "");

        for entity in entities {
            self.write_entity(entity, owner_handle);
        }

        self.group_str(0, "ENDBLK");
        self.write_handle();
        if let Some(owner) = owner_handle {
            self.group_str(330, owner);
        }
        self.group_str(100, "AcDbEntity");
        self.group_str(8, "0");
        self.group_str(100, "AcDbBlockEnd");
    }

    fn ensure_block_record_table(&mut self, doc: &DxfDocument) {
        if !self.block_record_order.is_empty() {
            return;
        }
        self.register_block_record("*Model_Space");
        self.register_block_record("*Paper_Space");
        for block in &doc.blocks {
            self.register_block_record(&block.name);
        }
    }

    fn register_block_record(&mut self, name: &str) {
        if self.block_record_handles.contains_key(name) {
            return;
        }
        let handle = self.alloc_handle();
        self.block_record_order.push(name.to_string());
        self.block_record_handles.insert(name.to_string(), handle);
    }

    fn block_record_handle(&self, name: &str) -> Option<&str> {
        self.block_record_handles.get(name).map(String::as_str)
    }

    fn write_entity(&mut self, entity: &DxfEntity, owner_handle: Option<&str>) {
        match entity {
            DxfEntity::Line(v) => {
                self.entity_header("LINE", &v.layer, v.color, &v.line_type, owner_handle);
                self.group_f64(10, v.x1);
                self.group_f64(20, v.y1);
                self.group_f64(30, 0.0);
                self.group_f64(11, v.x2);
                self.group_f64(21, v.y2);
                self.group_f64(31, 0.0);
            }
            DxfEntity::Circle(v) => {
                self.entity_header("CIRCLE", &v.layer, v.color, &v.line_type, owner_handle);
                self.group_f64(10, v.center_x);
                self.group_f64(20, v.center_y);
                self.group_f64(30, 0.0);
                self.group_f64(40, v.radius);
            }
            DxfEntity::Arc(v) => {
                self.entity_header("ARC", &v.layer, v.color, &v.line_type, owner_handle);
                self.group_f64(10, v.center_x);
                self.group_f64(20, v.center_y);
                self.group_f64(30, 0.0);
                self.group_f64(40, v.radius);
                self.group_f64(50, v.start_angle);
                self.group_f64(51, v.end_angle);
            }
            DxfEntity::Ellipse(v) => {
                self.entity_header("ELLIPSE", &v.layer, v.color, &v.line_type, owner_handle);
                self.group_f64(10, v.center_x);
                self.group_f64(20, v.center_y);
                self.group_f64(30, 0.0);
                self.group_f64(11, v.major_axis_x);
                self.group_f64(21, v.major_axis_y);
                self.group_f64(31, 0.0);
                self.group_f64(40, v.minor_ratio);
                self.group_f64(41, v.start_param);
                self.group_f64(42, v.end_param);
            }
            DxfEntity::Point(v) => {
                self.entity_header("POINT", &v.layer, v.color, &v.line_type, owner_handle);
                self.group_f64(10, v.x);
                self.group_f64(20, v.y);
                self.group_f64(30, 0.0);
            }
            DxfEntity::Text(v) => {
                self.entity_header("TEXT", &v.layer, v.color, &v.line_type, owner_handle);
                self.group_f64(10, v.x);
                self.group_f64(20, v.y);
                self.group_f64(30, 0.0);
                self.group_f64(40, v.height);
                self.group_str(1, &escape_unicode(&v.content));
                self.group_f64(50, v.rotation);
                self.group_str(7, &escape_unicode(&v.style));
            }
            DxfEntity::Solid(v) => {
                self.entity_header("SOLID", &v.layer, v.color, &v.line_type, owner_handle);
                self.group_f64(10, v.x1);
                self.group_f64(20, v.y1);
                self.group_f64(30, 0.0);
                self.group_f64(11, v.x2);
                self.group_f64(21, v.y2);
                self.group_f64(31, 0.0);
                self.group_f64(12, v.x3);
                self.group_f64(22, v.y3);
                self.group_f64(32, 0.0);
                self.group_f64(13, v.x4);
                self.group_f64(23, v.y4);
                self.group_f64(33, 0.0);
            }
            DxfEntity::FilledPolygon(v) => {
                self.write_filled_polygon(v, owner_handle);
            }
            DxfEntity::Insert(v) => {
                self.entity_header("INSERT", &v.layer, v.color, &v.line_type, owner_handle);
                self.group_str(2, &escape_unicode(&v.block_name));
                self.group_f64(10, v.x);
                self.group_f64(20, v.y);
                self.group_f64(30, 0.0);
                self.group_f64(41, v.scale_x);
                self.group_f64(42, v.scale_y);
                self.group_f64(43, 1.0);
                self.group_f64(50, v.rotation);
            }
        }
    }

    fn write_filled_polygon(&mut self, polygon: &DxfFilledPolygon, owner_handle: Option<&str>) {
        let points = polygon
            .points
            .iter()
            .copied()
            .filter(|p| p.x.is_finite() && p.y.is_finite())
            .collect::<Vec<_>>();
        if points.len() < 3 {
            return;
        }

        let anchor = points[0];
        for pair in points[1..].windows(2) {
            let p2 = pair[0];
            let p3 = pair[1];
            self.entity_header(
                "SOLID",
                &polygon.layer,
                polygon.color,
                &polygon.line_type,
                owner_handle,
            );
            self.group_f64(10, anchor.x);
            self.group_f64(20, anchor.y);
            self.group_f64(30, 0.0);
            self.group_f64(11, p2.x);
            self.group_f64(21, p2.y);
            self.group_f64(31, 0.0);
            self.group_f64(12, p3.x);
            self.group_f64(22, p3.y);
            self.group_f64(32, 0.0);
            self.group_f64(13, p3.x);
            self.group_f64(23, p3.y);
            self.group_f64(33, 0.0);
        }
    }

    fn entity_header(
        &mut self,
        entity_type: &str,
        layer: &str,
        color: i32,
        line_type: &str,
        owner_handle: Option<&str>,
    ) {
        self.group_str(0, entity_type);
        self.write_handle();
        if let Some(owner) = owner_handle {
            self.group_str(330, owner);
        }
        self.group_str(8, &escape_unicode(layer));
        self.group_i32(62, color);
        self.group_str(6, line_type);
    }

    fn section_start(&mut self, name: &str) {
        self.group_str(0, "SECTION");
        self.group_str(2, name);
    }

    fn section_end(&mut self) {
        self.group_str(0, "ENDSEC");
    }

    fn group_str(&mut self, code: i32, value: &str) {
        let _ = write!(self.out, "{code:>3}\n{value}\n");
    }

    fn group_i32(&mut self, code: i32, value: i32) {
        let _ = write!(self.out, "{code:>3}\n{value}\n");
    }

    fn group_f64(&mut self, code: i32, value: f64) {
        let _ = write!(self.out, "{code:>3}\n{value:.12}\n");
    }

    fn write_handle(&mut self) {
        let handle = self.alloc_handle();
        self.group_str(5, &handle);
    }

    fn alloc_handle(&mut self) -> String {
        let handle = format!("{:X}", self.next_handle);
        self.next_handle += 1;
        handle
    }
}

fn collect_line_types(doc: &DxfDocument) -> BTreeSet<String> {
    let mut out = BTreeSet::<String>::new();
    for layer in &doc.layers {
        out.insert(layer.line_type.clone());
    }
    for entity in &doc.entities {
        out.insert(entity_line_type(entity).to_string());
    }
    for block in &doc.blocks {
        for entity in &block.entities {
            out.insert(entity_line_type(entity).to_string());
        }
    }
    out
}

fn entity_line_type(entity: &DxfEntity) -> &str {
    match entity {
        DxfEntity::Line(v) => &v.line_type,
        DxfEntity::Circle(v) => &v.line_type,
        DxfEntity::Arc(v) => &v.line_type,
        DxfEntity::Ellipse(v) => &v.line_type,
        DxfEntity::Point(v) => &v.line_type,
        DxfEntity::Text(v) => &v.line_type,
        DxfEntity::Solid(v) => &v.line_type,
        DxfEntity::FilledPolygon(v) => &v.line_type,
        DxfEntity::Insert(v) => &v.line_type,
    }
}

fn escape_unicode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\r' => {}
            '\n' => out.push_str("\\P"),
            '\\' => out.push_str("\\\\"),
            _ if ch.is_ascii() && !ch.is_ascii_control() => out.push(ch),
            _ => {
                let _ = write!(out, "\\U+{:04X}", ch as u32);
            }
        }
    }
    out
}

fn block_defs_by_number(block_defs: &[BlockDef]) -> HashMap<u32, &BlockDef> {
    let mut map = HashMap::<u32, &BlockDef>::with_capacity(block_defs.len());
    for block_def in block_defs {
        map.insert(block_def.number, block_def);
    }
    map
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Transform2D {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
}

impl Transform2D {
    fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    fn from_insert(block: &Block) -> Self {
        let cos = block.rotation.cos();
        let sin = block.rotation.sin();
        Self {
            a: cos * block.scale_x,
            b: sin * block.scale_x,
            c: -sin * block.scale_y,
            d: cos * block.scale_y,
            tx: block.ref_x,
            ty: block.ref_y,
        }
    }

    fn compose(&self, rhs: &Self) -> Self {
        Self {
            a: self.a * rhs.a + self.c * rhs.b,
            b: self.b * rhs.a + self.d * rhs.b,
            c: self.a * rhs.c + self.c * rhs.d,
            d: self.b * rhs.c + self.d * rhs.d,
            tx: self.a * rhs.tx + self.c * rhs.ty + self.tx,
            ty: self.b * rhs.tx + self.d * rhs.ty + self.ty,
        }
    }

    fn apply_point(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.tx,
            self.b * x + self.d * y + self.ty,
        )
    }

    fn apply_vector(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y, self.b * x + self.d * y)
    }

    fn average_scale(&self) -> f64 {
        let sx = (self.a * self.a + self.b * self.b).sqrt();
        let sy = (self.c * self.c + self.d * self.d).sqrt();
        (sx + sy) / 2.0
    }

    fn rotation_deg(&self) -> f64 {
        self.b.atan2(self.a) * 180.0 / PI
    }
}

struct ExplodeContext<'a> {
    doc: &'a JwwDocument,
    block_name_map: &'a HashMap<u32, String>,
    block_defs: &'a HashMap<u32, &'a BlockDef>,
    unsupported_entities: &'a mut Vec<String>,
    options: ConvertOptions,
}

fn convert_entities_exploded(
    context: &mut ExplodeContext<'_>,
    entities: &[Entity],
    transform: &Transform2D,
    expanding_stack: &mut Vec<u32>,
) -> Vec<DxfEntity> {
    let mut out = Vec::<DxfEntity>::new();
    for entity in entities {
        match entity {
            Entity::Block(block) => {
                if expanding_stack.len() >= context.options.max_block_nesting {
                    context
                        .unsupported_entities
                        .push(format!("BLOCK_DEPTH_LIMIT({})", block.def_number));
                    continue;
                }
                if expanding_stack.contains(&block.def_number) {
                    context
                        .unsupported_entities
                        .push(format!("BLOCK_CYCLE({})", block.def_number));
                    continue;
                }

                let Some(block_def) = context.block_defs.get(&block.def_number).copied() else {
                    context
                        .unsupported_entities
                        .push(format!("UNRESOLVED_BLOCK({})", block.def_number));
                    continue;
                };

                expanding_stack.push(block.def_number);
                let child_transform = transform.compose(&Transform2D::from_insert(block));
                let expanded = convert_entities_exploded(
                    context,
                    &block_def.entities,
                    &child_transform,
                    expanding_stack,
                );
                expanding_stack.pop();
                out.extend(expanded);
            }
            _ => match convert_entity(context.doc, entity, context.block_name_map) {
                Some(converted) => {
                    for dxf_entity in converted {
                        out.extend(transform_entity_for_explode(&dxf_entity, transform));
                    }
                }
                None => context
                    .unsupported_entities
                    .push(entity.entity_type().to_string()),
            },
        }
    }
    out
}

fn transform_entity_for_explode(entity: &DxfEntity, transform: &Transform2D) -> Vec<DxfEntity> {
    match entity {
        DxfEntity::Line(v) => {
            let (x1, y1) = transform.apply_point(v.x1, v.y1);
            let (x2, y2) = transform.apply_point(v.x2, v.y2);
            vec![DxfEntity::Line(DxfLine {
                layer: v.layer.clone(),
                color: v.color,
                line_type: v.line_type.clone(),
                x1,
                y1,
                x2,
                y2,
            })]
        }
        DxfEntity::Circle(v) => transform_circle_for_explode(v, transform),
        DxfEntity::Arc(v) => transform_arc_for_explode(v, transform),
        DxfEntity::Ellipse(v) => transform_ellipse_for_explode(v, transform),
        DxfEntity::Point(v) => {
            let (x, y) = transform.apply_point(v.x, v.y);
            vec![DxfEntity::Point(DxfPoint {
                layer: v.layer.clone(),
                color: v.color,
                line_type: v.line_type.clone(),
                x,
                y,
            })]
        }
        DxfEntity::Text(v) => {
            let (x, y) = transform.apply_point(v.x, v.y);
            let height = (v.height * transform.average_scale().abs()).max(0.1);
            vec![DxfEntity::Text(DxfText {
                layer: v.layer.clone(),
                color: v.color,
                line_type: v.line_type.clone(),
                x,
                y,
                height,
                rotation: v.rotation + transform.rotation_deg(),
                content: v.content.clone(),
                style: v.style.clone(),
            })]
        }
        DxfEntity::Solid(v) => {
            let (x1, y1) = transform.apply_point(v.x1, v.y1);
            let (x2, y2) = transform.apply_point(v.x2, v.y2);
            let (x3, y3) = transform.apply_point(v.x3, v.y3);
            let (x4, y4) = transform.apply_point(v.x4, v.y4);
            vec![DxfEntity::Solid(DxfSolid {
                layer: v.layer.clone(),
                color: v.color,
                line_type: v.line_type.clone(),
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
                x4,
                y4,
            })]
        }
        DxfEntity::FilledPolygon(v) => vec![DxfEntity::FilledPolygon(DxfFilledPolygon {
            layer: v.layer.clone(),
            color: v.color,
            line_type: v.line_type.clone(),
            points: v
                .points
                .iter()
                .map(|p| {
                    let (x, y) = transform.apply_point(p.x, p.y);
                    DxfVertex { x, y }
                })
                .collect(),
        })],
        DxfEntity::Insert(v) => {
            let (x, y) = transform.apply_point(v.x, v.y);
            vec![DxfEntity::Insert(DxfInsert {
                layer: v.layer.clone(),
                color: v.color,
                line_type: v.line_type.clone(),
                block_name: v.block_name.clone(),
                x,
                y,
                scale_x: v.scale_x,
                scale_y: v.scale_y,
                rotation: v.rotation + transform.rotation_deg(),
            })]
        }
    }
}

fn transform_circle_for_explode(circle: &DxfCircle, transform: &Transform2D) -> Vec<DxfEntity> {
    let (center_x, center_y) = transform.apply_point(circle.center_x, circle.center_y);
    let (ux, uy) = transform.apply_vector(circle.radius, 0.0);
    let (vx, vy) = transform.apply_vector(0.0, circle.radius);

    let lu = (ux * ux + uy * uy).sqrt();
    let lv = (vx * vx + vy * vy).sqrt();
    if lu <= 1e-12 && lv <= 1e-12 {
        return vec![DxfEntity::Point(DxfPoint {
            layer: circle.layer.clone(),
            color: circle.color,
            line_type: circle.line_type.clone(),
            x: center_x,
            y: center_y,
        })];
    }

    let denom = lu * lv;
    let dot = if denom <= 1e-12 {
        0.0
    } else {
        (ux * vx + uy * vy) / denom
    };
    if nearly_equal(lu, lv) && dot.abs() < 1e-6 {
        return vec![DxfEntity::Circle(DxfCircle {
            layer: circle.layer.clone(),
            color: circle.color,
            line_type: circle.line_type.clone(),
            center_x,
            center_y,
            radius: (lu + lv) / 2.0,
        })];
    }

    let (major_x, major_y, minor_ratio) = if lu >= lv {
        (ux, uy, if lu <= 1e-12 { 1.0 } else { lv / lu })
    } else {
        (vx, vy, if lv <= 1e-12 { 1.0 } else { lu / lv })
    };

    vec![DxfEntity::Ellipse(DxfEllipse {
        layer: circle.layer.clone(),
        color: circle.color,
        line_type: circle.line_type.clone(),
        center_x,
        center_y,
        major_axis_x: major_x,
        major_axis_y: major_y,
        minor_ratio,
        start_param: 0.0,
        end_param: 2.0 * PI,
    })]
}

fn transform_arc_for_explode(arc: &DxfArc, transform: &Transform2D) -> Vec<DxfEntity> {
    let mut end = arc.end_angle;
    let start = arc.start_angle;
    if end < start {
        end += 360.0;
    }
    let sweep = (end - start).abs();
    let segments = ((sweep / 360.0) * 96.0).ceil() as usize;
    let segments = segments.clamp(8, 192);

    let mut points = Vec::<(f64, f64)>::with_capacity(segments + 1);
    for i in 0..=segments {
        let t = start + (end - start) * (i as f64) / (segments as f64);
        let rad = t * PI / 180.0;
        let x = arc.center_x + arc.radius * rad.cos();
        let y = arc.center_y + arc.radius * rad.sin();
        points.push(transform.apply_point(x, y));
    }

    points_to_lines(points, arc.layer.clone(), arc.color, arc.line_type.clone())
}

fn transform_ellipse_for_explode(ellipse: &DxfEllipse, transform: &Transform2D) -> Vec<DxfEntity> {
    let start = ellipse.start_param;
    let mut end = ellipse.end_param;
    if end <= start {
        end += 2.0 * PI;
    }
    let span = (end - start).abs();
    let segments = ((span / (2.0 * PI)) * 128.0).ceil() as usize;
    let segments = segments.clamp(12, 256);

    let major_x = ellipse.major_axis_x;
    let major_y = ellipse.major_axis_y;
    let minor_x = -major_y * ellipse.minor_ratio;
    let minor_y = major_x * ellipse.minor_ratio;

    let mut points = Vec::<(f64, f64)>::with_capacity(segments + 1);
    for i in 0..=segments {
        let t = start + (end - start) * (i as f64) / (segments as f64);
        let x = ellipse.center_x + major_x * t.cos() + minor_x * t.sin();
        let y = ellipse.center_y + major_y * t.cos() + minor_y * t.sin();
        points.push(transform.apply_point(x, y));
    }

    points_to_lines(
        points,
        ellipse.layer.clone(),
        ellipse.color,
        ellipse.line_type.clone(),
    )
}

fn points_to_lines(
    points: Vec<(f64, f64)>,
    layer: String,
    color: i32,
    line_type: String,
) -> Vec<DxfEntity> {
    if points.len() < 2 {
        return Vec::new();
    }
    let mut out = Vec::<DxfEntity>::with_capacity(points.len().saturating_sub(1));
    for w in points.windows(2) {
        let (x1, y1) = w[0];
        let (x2, y2) = w[1];
        out.push(DxfEntity::Line(DxfLine {
            layer: layer.clone(),
            color,
            line_type: line_type.clone(),
            x1,
            y1,
            x2,
            y2,
        }));
    }
    out
}

fn nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

fn convert_layers(doc: &JwwDocument) -> Vec<DxfLayer> {
    let mut layers = Vec::<DxfLayer>::with_capacity(16 * 16);
    for g in 0..16 {
        for l in 0..16 {
            let layer = &doc.header.layer_groups[g].layers[l];
            let name = if layer.name.is_empty() {
                format!("{:X}-{:X}", g, l)
            } else {
                layer.name.clone()
            };
            layers.push(DxfLayer {
                name,
                color: ((g * 16 + l) % 255 + 1) as i32,
                line_type: "CONTINUOUS".to_string(),
                frozen: layer.state == 0,
                locked: layer.protect != 0,
            });
        }
    }
    layers
}

fn convert_blocks(
    doc: &JwwDocument,
    block_name_map: &HashMap<u32, String>,
    unsupported_entities: &mut Vec<String>,
) -> Vec<DxfBlock> {
    let mut blocks = Vec::<DxfBlock>::with_capacity(doc.block_defs.len());
    for block_def in &doc.block_defs {
        let name = block_def_name(block_def.number, &block_def.name);
        let entities = convert_entities(
            doc,
            &block_def.entities,
            block_name_map,
            unsupported_entities,
        );
        blocks.push(DxfBlock {
            name,
            base_x: 0.0,
            base_y: 0.0,
            entities,
        });
    }
    blocks
}

fn convert_entities(
    doc: &JwwDocument,
    entities: &[Entity],
    block_name_map: &HashMap<u32, String>,
    unsupported_entities: &mut Vec<String>,
) -> Vec<DxfEntity> {
    let mut out = Vec::<DxfEntity>::new();
    for entity in entities {
        match convert_entity(doc, entity, block_name_map) {
            Some(converted) => {
                for e in converted {
                    out.push(e);
                }
            }
            None => unsupported_entities.push(entity.entity_type().to_string()),
        }
    }
    out
}

fn convert_entity(
    doc: &JwwDocument,
    entity: &Entity,
    block_name_map: &HashMap<u32, String>,
) -> Option<Vec<DxfEntity>> {
    let base = entity.base();
    let layer = layer_name(doc, base.layer_group, base.layer);
    let color = map_color(base.pen_color);
    let line_type = map_line_type(base.pen_style).to_string();

    match entity {
        Entity::Line(v) => Some(vec![DxfEntity::Line(DxfLine {
            layer,
            color,
            line_type,
            x1: v.start_x,
            y1: v.start_y,
            x2: v.end_x,
            y2: v.end_y,
        })]),
        Entity::Arc(v) => Some(convert_arc(v, layer, color, line_type)),
        Entity::Point(v) => {
            if v.is_temporary {
                Some(Vec::new())
            } else {
                Some(vec![DxfEntity::Point(DxfPoint {
                    layer,
                    color,
                    line_type,
                    x: v.x,
                    y: v.y,
                })])
            }
        }
        Entity::Text(v) => Some(vec![DxfEntity::Text(convert_text(
            v, layer, color, line_type,
        ))]),
        Entity::Solid(v) => Some(vec![DxfEntity::Solid(convert_solid(
            v, layer, color, line_type,
        ))]),
        Entity::CircleSolid(v) => Some(convert_circle_solid(v, layer, color, line_type)),
        Entity::Block(v) => {
            let block_name = block_name_map
                .get(&v.def_number)
                .cloned()
                .unwrap_or_else(|| format!("BLOCK_{}", v.def_number));
            Some(vec![DxfEntity::Insert(DxfInsert {
                layer,
                color,
                line_type,
                block_name,
                x: v.ref_x,
                y: v.ref_y,
                scale_x: v.scale_x,
                scale_y: v.scale_y,
                rotation: rad_to_deg(v.rotation),
            })])
        }
        Entity::Dimension(v) => Some(vec![
            DxfEntity::Line(DxfLine {
                layer: layer.clone(),
                color,
                line_type: line_type.clone(),
                x1: v.line.start_x,
                y1: v.line.start_y,
                x2: v.line.end_x,
                y2: v.line.end_y,
            }),
            DxfEntity::Text(convert_text(&v.text, layer, color, line_type)),
        ]),
    }
}

fn convert_solid(solid: &Solid, layer: String, color: i32, line_type: String) -> DxfSolid {
    let points = order_solid_vertices([
        DxfVertex {
            x: solid.point1_x,
            y: solid.point1_y,
        },
        DxfVertex {
            x: solid.point2_x,
            y: solid.point2_y,
        },
        DxfVertex {
            x: solid.point3_x,
            y: solid.point3_y,
        },
        DxfVertex {
            x: solid.point4_x,
            y: solid.point4_y,
        },
    ]);

    DxfSolid {
        layer,
        color,
        line_type,
        x1: points[0].x,
        y1: points[0].y,
        x2: points[1].x,
        y2: points[1].y,
        x3: points[2].x,
        y3: points[2].y,
        x4: points[3].x,
        y4: points[3].y,
    }
}

fn order_solid_vertices(points: [DxfVertex; 4]) -> [DxfVertex; 4] {
    if !solid_vertices_cross(&points) {
        return points;
    }

    let center_x = points.iter().map(|p| p.x).sum::<f64>() / points.len() as f64;
    let center_y = points.iter().map(|p| p.y).sum::<f64>() / points.len() as f64;
    let mut ordered = points;
    ordered.sort_by(|a, b| {
        let angle_a = (a.y - center_y).atan2(a.x - center_x);
        let angle_b = (b.y - center_y).atan2(b.x - center_x);
        angle_a
            .partial_cmp(&angle_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if let Some(index) = ordered
        .iter()
        .position(|point| same_vertex(*point, points[0]))
    {
        ordered.rotate_left(index);
    }
    ordered
}

fn solid_vertices_cross(points: &[DxfVertex; 4]) -> bool {
    segments_intersect(points[0], points[1], points[2], points[3])
        || segments_intersect(points[1], points[2], points[3], points[0])
}

fn segments_intersect(a: DxfVertex, b: DxfVertex, c: DxfVertex, d: DxfVertex) -> bool {
    let ab_c = orientation(a, b, c);
    let ab_d = orientation(a, b, d);
    let cd_a = orientation(c, d, a);
    let cd_b = orientation(c, d, b);

    ab_c * ab_d < 0.0 && cd_a * cd_b < 0.0
}

fn orientation(a: DxfVertex, b: DxfVertex, c: DxfVertex) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn same_vertex(a: DxfVertex, b: DxfVertex) -> bool {
    (a.x - b.x).abs() <= 1e-9 && (a.y - b.y).abs() <= 1e-9
}

fn convert_circle_solid(
    solid: &CircleSolid,
    layer: String,
    color: i32,
    line_type: String,
) -> Vec<DxfEntity> {
    if solid.radius.abs() <= 1e-12 {
        return Vec::new();
    }

    if matches!(solid.base.pen_style, 105 | 106) {
        return convert_ring_solid(solid, layer, color, line_type);
    }

    let mode = solid.solid_mode.round() as i32;
    let is_full = mode == 100 || (solid.arc_angle.abs() - 2.0 * PI).abs() < 1e-6;
    let boundary = ellipse_arc_points(
        solid,
        solid.radius.abs(),
        if is_full { 0.0 } else { solid.start_angle },
        if is_full { 2.0 * PI } else { solid.arc_angle },
        is_full,
    );

    let points = match mode {
        100 => boundary,
        0 => {
            let mut points = Vec::<DxfVertex>::with_capacity(boundary.len() + 1);
            points.push(DxfVertex {
                x: solid.center_x,
                y: solid.center_y,
            });
            points.extend(boundary);
            points
        }
        -1 | 5 => boundary,
        _ if is_full => boundary,
        _ => {
            let mut points = Vec::<DxfVertex>::with_capacity(boundary.len() + 1);
            points.push(DxfVertex {
                x: solid.center_x,
                y: solid.center_y,
            });
            points.extend(boundary);
            points
        }
    };

    filled_polygon(layer, color, line_type, points)
}

fn convert_ring_solid(
    solid: &CircleSolid,
    layer: String,
    color: i32,
    line_type: String,
) -> Vec<DxfEntity> {
    let outer_radius = solid.radius.abs();
    let inner_radius = solid.solid_mode.abs();
    if inner_radius <= 1e-12 || inner_radius >= outer_radius {
        return filled_polygon(
            layer,
            color,
            line_type,
            ellipse_arc_points(solid, outer_radius, 0.0, 2.0 * PI, true),
        );
    }

    let is_full = (solid.arc_angle.abs() - 2.0 * PI).abs() < 1e-6;
    let start = if is_full { 0.0 } else { solid.start_angle };
    let span = if is_full { 2.0 * PI } else { solid.arc_angle };
    let steps = arc_segment_count(span, is_full);
    let mut out = Vec::<DxfEntity>::with_capacity(steps);

    for idx in 0..steps {
        let t1 = start + span * (idx as f64 / steps as f64);
        let t2 = start + span * ((idx + 1) as f64 / steps as f64);
        let outer1 = ellipse_point(solid, outer_radius, t1);
        let outer2 = ellipse_point(solid, outer_radius, t2);
        let inner2 = ellipse_point(solid, inner_radius, t2);
        let inner1 = ellipse_point(solid, inner_radius, t1);
        out.extend(filled_polygon(
            layer.clone(),
            color,
            line_type.clone(),
            vec![outer1, outer2, inner2, inner1],
        ));
    }

    out
}

fn filled_polygon(
    layer: String,
    color: i32,
    line_type: String,
    points: Vec<DxfVertex>,
) -> Vec<DxfEntity> {
    let points = points
        .into_iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .collect::<Vec<_>>();
    if points.len() < 3 {
        return Vec::new();
    }

    vec![DxfEntity::FilledPolygon(DxfFilledPolygon {
        layer,
        color,
        line_type,
        points,
    })]
}

fn ellipse_arc_points(
    solid: &CircleSolid,
    radius: f64,
    start_angle: f64,
    arc_angle: f64,
    is_full: bool,
) -> Vec<DxfVertex> {
    let steps = arc_segment_count(arc_angle, is_full);
    let end = if is_full { steps } else { steps + 1 };
    (0..end)
        .map(|idx| {
            let t = start_angle + arc_angle * (idx as f64 / steps as f64);
            ellipse_point(solid, radius, t)
        })
        .collect()
}

fn arc_segment_count(arc_angle: f64, is_full: bool) -> usize {
    let span = if is_full {
        2.0 * PI
    } else {
        arc_angle.abs().max(PI / 32.0)
    };
    ((span / (2.0 * PI) * 96.0).ceil() as usize).clamp(8, 128)
}

fn ellipse_point(solid: &CircleSolid, radius: f64, angle: f64) -> DxfVertex {
    let flatness = if solid.flatness.abs() <= 1e-12 {
        1.0
    } else {
        solid.flatness.abs()
    };
    let mut major_radius = radius.abs();
    let mut minor_ratio = flatness;
    let mut tilt = solid.tilt_angle;

    if minor_ratio > 1.0 {
        major_radius *= minor_ratio;
        minor_ratio = 1.0 / minor_ratio;
        tilt += PI / 2.0;
    }

    let minor_radius = major_radius * minor_ratio;
    let cos_tilt = tilt.cos();
    let sin_tilt = tilt.sin();
    let local_x = major_radius * angle.cos();
    let local_y = minor_radius * angle.sin();

    DxfVertex {
        x: solid.center_x + local_x * cos_tilt - local_y * sin_tilt,
        y: solid.center_y + local_x * sin_tilt + local_y * cos_tilt,
    }
}

fn convert_arc(arc: &Arc, layer: String, color: i32, line_type: String) -> Vec<DxfEntity> {
    if arc.is_full_circle && arc.flatness == 1.0 {
        return vec![DxfEntity::Circle(DxfCircle {
            layer,
            color,
            line_type,
            center_x: arc.center_x,
            center_y: arc.center_y,
            radius: arc.radius,
        })];
    }

    if arc.flatness != 1.0 {
        let mut major_radius = arc.radius;
        let mut minor_ratio = arc.flatness;
        let mut tilt_angle = arc.tilt_angle;

        if minor_ratio > 1.0 {
            major_radius = arc.radius * arc.flatness;
            minor_ratio = 1.0 / arc.flatness;
            tilt_angle = arc.tilt_angle + PI / 2.0;
        }

        let major_axis_x = major_radius * tilt_angle.cos();
        let major_axis_y = major_radius * tilt_angle.sin();
        let start_param = if arc.is_full_circle {
            0.0
        } else {
            arc.start_angle
        };
        let end_param = if arc.is_full_circle {
            2.0 * PI
        } else {
            arc.start_angle + arc.arc_angle
        };

        return vec![DxfEntity::Ellipse(DxfEllipse {
            layer,
            color,
            line_type,
            center_x: arc.center_x,
            center_y: arc.center_y,
            major_axis_x,
            major_axis_y,
            minor_ratio,
            start_param,
            end_param,
        })];
    }

    vec![DxfEntity::Arc(DxfArc {
        layer,
        color,
        line_type,
        center_x: arc.center_x,
        center_y: arc.center_y,
        radius: arc.radius,
        start_angle: rad_to_deg(arc.start_angle),
        end_angle: rad_to_deg(arc.start_angle + arc.arc_angle),
    })]
}

fn convert_text(text: &Text, layer: String, color: i32, line_type: String) -> DxfText {
    DxfText {
        layer,
        color,
        line_type,
        x: text.start_x,
        y: text.start_y,
        height: if text.size_y <= 0.0 { 2.5 } else { text.size_y },
        rotation: text.angle,
        content: text.content.clone(),
        style: "STANDARD".to_string(),
    }
}

fn block_name_map(doc: &JwwDocument) -> HashMap<u32, String> {
    let mut map = HashMap::<u32, String>::with_capacity(doc.block_defs.len());
    for block_def in &doc.block_defs {
        map.insert(
            block_def.number,
            block_def_name(block_def.number, &block_def.name),
        );
    }
    map
}

fn block_def_name(number: u32, raw: &str) -> String {
    if raw.is_empty() {
        format!("BLOCK_{number}")
    } else {
        raw.to_string()
    }
}

fn layer_name(doc: &JwwDocument, layer_group: u16, layer: u16) -> String {
    let g = layer_group as usize;
    let l = layer as usize;
    if g < 16 && l < 16 {
        let candidate = doc.header.layer_groups[g].layers[l].name.trim();
        if !candidate.is_empty() {
            return candidate.to_string();
        }
    }
    format!("{:X}-{:X}", layer_group, layer)
}

fn map_color(pen_color: u16) -> i32 {
    match pen_color {
        1 | 8 => 7,
        2 => 5,
        3 => 1,
        4 => 6,
        5 => 3,
        6 => 4,
        7 => 2,
        9 => 8,
        _ => ((pen_color as i32) % 255).max(1),
    }
}

fn map_line_type(pen_style: u8) -> &'static str {
    match pen_style {
        0 | 1 => "CONTINUOUS",
        2 => "DASHED",
        3 => "DASHDOT",
        4 => "CENTER",
        5 => "DOT",
        6 => "DASHED2",
        7 => "DASHDOT2",
        8 => "CENTER2",
        9 => "DOT2",
        _ => "BYLAYER",
    }
}

fn rad_to_deg(rad: f64) -> f64 {
    rad * 180.0 / PI
}

#[cfg(test)]
mod tests {
    use std::array;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::header::{JwwHeader, LayerGroupHeader, LayerHeader};
    use crate::model::{
        Block, BlockDef, CircleSolid, Entity, EntityBase, JwwDocument, Line, Solid, Text,
    };
    use crate::parser::read_document_from_file;

    use super::{
        convert_document, convert_document_with_options, document_to_string, map_line_type,
        solid_vertices_cross, ConvertOptions, DxfDocument, DxfEntity, DxfLayer, DxfText, DxfVertex,
    };

    fn empty_header() -> JwwHeader {
        JwwHeader {
            version: 600,
            memo: String::new(),
            paper_size: 0,
            write_layer_group: 0,
            layer_groups: array::from_fn(|g| LayerGroupHeader {
                state: 0,
                write_layer: 0,
                scale: 1.0,
                protect: 0,
                name: format!("Group{g:X}"),
                layers: array::from_fn(|l| LayerHeader {
                    state: 0,
                    protect: 0,
                    name: format!("{g:X}-{l:X}"),
                }),
            }),
        }
    }

    fn jww_samples_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../jww_samples")
    }

    #[test]
    fn map_line_type_matches_jww_pen_style_numbers() {
        let cases = [
            (0, "CONTINUOUS"),
            (1, "CONTINUOUS"),
            (2, "DASHED"),
            (3, "DASHDOT"),
            (4, "CENTER"),
            (5, "DOT"),
            (6, "DASHED2"),
            (7, "DASHDOT2"),
            (8, "CENTER2"),
            (9, "DOT2"),
            (42, "BYLAYER"),
        ];

        for (pen_style, expected) in cases {
            assert_eq!(map_line_type(pen_style), expected);
        }
    }

    #[test]
    fn convert_document_turns_circle_solid_into_filled_polygon() {
        let base = EntityBase {
            pen_style: 101,
            ..EntityBase::default()
        };
        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![Entity::CircleSolid(CircleSolid {
                base,
                center_x: 10.0,
                center_y: 20.0,
                radius: 3.0,
                flatness: 1.0,
                tilt_angle: 0.0,
                start_angle: 0.0,
                arc_angle: 2.0 * std::f64::consts::PI,
                solid_mode: 100.0,
                color: None,
            })],
            block_defs: vec![],
        };

        let dxf = convert_document(&doc);
        assert_eq!(dxf.entities.len(), 1);
        match &dxf.entities[0] {
            DxfEntity::FilledPolygon(polygon) => {
                assert!(polygon.points.len() >= 24);
                assert!(polygon.points.iter().all(|point| point.x.is_finite()));
                assert!(polygon.points.iter().all(|point| point.y.is_finite()));
            }
            other => panic!("expected FILLED_POLYGON, got {:?}", other),
        }
    }

    #[test]
    fn convert_document_orders_crossed_solid_vertices() {
        let base = EntityBase::default();
        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![Entity::Solid(Solid {
                base,
                point1_x: 0.0,
                point1_y: 10.0,
                point2_x: 10.0,
                point2_y: 0.0,
                point3_x: 10.0,
                point3_y: 10.0,
                point4_x: 0.0,
                point4_y: 0.0,
                color: None,
            })],
            block_defs: vec![],
        };

        let dxf = convert_document(&doc);
        assert_eq!(dxf.entities.len(), 1);
        match &dxf.entities[0] {
            DxfEntity::Solid(solid) => {
                let points = [
                    DxfVertex {
                        x: solid.x1,
                        y: solid.y1,
                    },
                    DxfVertex {
                        x: solid.x2,
                        y: solid.y2,
                    },
                    DxfVertex {
                        x: solid.x3,
                        y: solid.y3,
                    },
                    DxfVertex {
                        x: solid.x4,
                        y: solid.y4,
                    },
                ];
                assert!(!solid_vertices_cross(&points));
                assert_eq!(points[0], DxfVertex { x: 0.0, y: 10.0 });
            }
            other => panic!("expected SOLID, got {:?}", other),
        }
    }

    #[test]
    fn convert_document_handles_line_and_dimension() {
        let base = EntityBase::default();
        let line = Entity::Line(Line {
            base,
            start_x: 0.0,
            start_y: 0.0,
            end_x: 10.0,
            end_y: 0.0,
        });
        let dim = Entity::Dimension(crate::model::Dimension {
            base,
            line: Line {
                base,
                start_x: 0.0,
                start_y: 1.0,
                end_x: 10.0,
                end_y: 1.0,
            },
            text: Text {
                base,
                start_x: 5.0,
                start_y: 2.0,
                end_x: 5.0,
                end_y: 2.0,
                text_type: 0,
                size_x: 1.0,
                size_y: 1.0,
                spacing: 0.0,
                angle: 0.0,
                font_name: String::new(),
                content: "1000".to_string(),
            },
            sxf_mode: Some(0),
            aux_lines: vec![],
            aux_points: vec![],
        });

        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![line, dim],
            block_defs: vec![],
        };

        let dxf = convert_document(&doc);
        let types = dxf
            .entities
            .iter()
            .map(DxfEntity::entity_type)
            .collect::<Vec<_>>();
        assert_eq!(types, vec!["LINE", "LINE", "TEXT"]);
    }

    #[test]
    fn convert_document_resolves_insert_block_name() {
        let base = EntityBase::default();
        let entity = Entity::Block(Block {
            base,
            ref_x: 1.0,
            ref_y: 2.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            def_number: 5,
        });

        let block_def = BlockDef {
            base,
            number: 5,
            is_referenced: true,
            name: "Door".to_string(),
            entities: vec![],
        };

        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![entity],
            block_defs: vec![block_def],
        };

        let dxf = convert_document(&doc);
        match &dxf.entities[0] {
            DxfEntity::Insert(v) => assert_eq!(v.block_name, "Door"),
            other => panic!("expected INSERT, got {:?}", other),
        }
    }

    #[test]
    fn convert_document_explode_inserts_expands_nested_blocks() {
        let base = EntityBase::default();
        let top_insert = Entity::Block(Block {
            base,
            ref_x: 10.0,
            ref_y: 20.0,
            scale_x: 2.0,
            scale_y: 2.0,
            rotation: 0.0,
            def_number: 1,
        });

        let block_2 = BlockDef {
            base,
            number: 2,
            is_referenced: true,
            name: "B2".to_string(),
            entities: vec![Entity::Line(Line {
                base,
                start_x: 0.0,
                start_y: 0.0,
                end_x: 0.0,
                end_y: 1.0,
            })],
        };

        let block_1 = BlockDef {
            base,
            number: 1,
            is_referenced: true,
            name: "B1".to_string(),
            entities: vec![
                Entity::Line(Line {
                    base,
                    start_x: 0.0,
                    start_y: 0.0,
                    end_x: 1.0,
                    end_y: 0.0,
                }),
                Entity::Block(Block {
                    base,
                    ref_x: 0.0,
                    ref_y: 2.0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                    rotation: 0.0,
                    def_number: 2,
                }),
            ],
        };

        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![top_insert],
            block_defs: vec![block_1, block_2],
        };

        let dxf = convert_document_with_options(
            &doc,
            ConvertOptions {
                explode_inserts: true,
                max_block_nesting: 32,
            },
        );

        assert!(dxf.blocks.is_empty());
        assert!(!dxf.entities.is_empty());
        assert!(!dxf
            .entities
            .iter()
            .any(|e| matches!(e, DxfEntity::Insert(_))));

        assert!(contains_line(&dxf.entities, 10.0, 20.0, 12.0, 20.0));
        assert!(contains_line(&dxf.entities, 10.0, 24.0, 10.0, 26.0));
    }

    #[test]
    fn convert_document_explode_inserts_detects_cycle() {
        let base = EntityBase::default();
        let top_insert = Entity::Block(Block {
            base,
            ref_x: 0.0,
            ref_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            def_number: 1,
        });

        let block_1 = BlockDef {
            base,
            number: 1,
            is_referenced: true,
            name: "B1".to_string(),
            entities: vec![Entity::Block(Block {
                base,
                ref_x: 0.0,
                ref_y: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                rotation: 0.0,
                def_number: 2,
            })],
        };
        let block_2 = BlockDef {
            base,
            number: 2,
            is_referenced: true,
            name: "B2".to_string(),
            entities: vec![Entity::Block(Block {
                base,
                ref_x: 0.0,
                ref_y: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                rotation: 0.0,
                def_number: 1,
            })],
        };

        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![top_insert],
            block_defs: vec![block_1, block_2],
        };

        let dxf = convert_document_with_options(
            &doc,
            ConvertOptions {
                explode_inserts: true,
                max_block_nesting: 32,
            },
        );

        assert!(dxf
            .unsupported_entities
            .iter()
            .any(|v| v.starts_with("BLOCK_CYCLE(")));
    }

    #[test]
    fn convert_document_explode_inserts_reports_unresolved_block() {
        let base = EntityBase::default();
        let top_insert = Entity::Block(Block {
            base,
            ref_x: 0.0,
            ref_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            def_number: 999,
        });

        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![top_insert],
            block_defs: vec![],
        };

        let dxf = convert_document_with_options(
            &doc,
            ConvertOptions {
                explode_inserts: true,
                max_block_nesting: 32,
            },
        );

        assert!(dxf.entities.is_empty());
        assert!(dxf.blocks.is_empty());
        assert!(dxf
            .unsupported_entities
            .iter()
            .any(|v| v == "UNRESOLVED_BLOCK(999)"));
    }

    #[test]
    fn convert_document_explode_inserts_enforces_depth_limit() {
        let base = EntityBase::default();
        let top_insert = Entity::Block(Block {
            base,
            ref_x: 0.0,
            ref_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            def_number: 1,
        });

        let block_2 = BlockDef {
            base,
            number: 2,
            is_referenced: true,
            name: "B2".to_string(),
            entities: vec![Entity::Line(Line {
                base,
                start_x: 0.0,
                start_y: 0.0,
                end_x: 1.0,
                end_y: 0.0,
            })],
        };

        let block_1 = BlockDef {
            base,
            number: 1,
            is_referenced: true,
            name: "B1".to_string(),
            entities: vec![Entity::Block(Block {
                base,
                ref_x: 5.0,
                ref_y: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                rotation: 0.0,
                def_number: 2,
            })],
        };

        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![top_insert],
            block_defs: vec![block_1, block_2],
        };

        let dxf = convert_document_with_options(
            &doc,
            ConvertOptions {
                explode_inserts: true,
                max_block_nesting: 1,
            },
        );

        assert!(dxf.entities.is_empty());
        assert!(dxf
            .unsupported_entities
            .iter()
            .any(|v| v == "BLOCK_DEPTH_LIMIT(2)"));
    }

    #[test]
    fn document_to_string_emits_minimum_dxf_sections() {
        let base = EntityBase::default();
        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![Entity::Line(Line {
                base,
                start_x: 0.0,
                start_y: 0.0,
                end_x: 10.0,
                end_y: 0.0,
            })],
            block_defs: vec![],
        };

        let dxf = convert_document(&doc);
        let out = document_to_string(&dxf);

        assert!(out.contains("  0\nSECTION\n  2\nHEADER\n"));
        assert!(out.contains("  2\nTABLES\n"));
        assert!(out.contains("  2\nBLOCKS\n"));
        assert!(out.contains("  2\nENTITIES\n"));
        assert!(out.contains("  0\nLINE\n"));
        assert!(out.ends_with("  0\nEOF\n"));
    }

    #[test]
    fn document_to_string_escapes_unicode_fields() {
        let dxf = DxfDocument {
            layers: vec![DxfLayer {
                name: "図面".to_string(),
                color: 7,
                line_type: "CONTINUOUS".to_string(),
                frozen: false,
                locked: false,
            }],
            entities: vec![DxfEntity::Text(DxfText {
                layer: "図面".to_string(),
                color: 7,
                line_type: "CONTINUOUS".to_string(),
                x: 0.0,
                y: 0.0,
                height: 2.5,
                rotation: 0.0,
                content: "日本語".to_string(),
                style: "STANDARD".to_string(),
            })],
            blocks: vec![],
            unsupported_entities: vec![],
        };

        let out = document_to_string(&dxf);
        assert!(out.contains("\\U+56F3\\U+9762"));
        assert!(out.contains("\\U+65E5\\U+672C\\U+8A9E"));
    }

    #[test]
    fn convert_and_write_all_jww_samples() {
        let dir = jww_samples_dir();
        let mut files = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().map(|ext| ext == "jww").unwrap_or(false))
            .collect::<Vec<_>>();
        files.sort();
        assert!(!files.is_empty(), "no .jww files found in jww_samples");

        for path in files {
            let doc = read_document_from_file(&path)
                .unwrap_or_else(|e| panic!("failed parsing {}: {e}", path.display()));
            let dxf = convert_document(&doc);
            let output = document_to_string(&dxf);
            assert!(output.starts_with("  0\nSECTION\n  2\nHEADER\n"));
            assert!(output.ends_with("  0\nEOF\n"));
            assert!(
                dxf.unsupported_entities.is_empty(),
                "unsupported entities in {}: {:?}",
                path.display(),
                dxf.unsupported_entities
            );
        }
    }

    #[test]
    fn document_to_string_has_objects_section_and_unique_handles() {
        let base = EntityBase::default();
        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![
                Entity::Line(Line {
                    base,
                    start_x: 0.0,
                    start_y: 0.0,
                    end_x: 10.0,
                    end_y: 0.0,
                }),
                Entity::Text(Text {
                    base,
                    start_x: 5.0,
                    start_y: 2.0,
                    end_x: 5.0,
                    end_y: 2.0,
                    text_type: 0,
                    size_x: 1.0,
                    size_y: 1.0,
                    spacing: 0.0,
                    angle: 0.0,
                    font_name: String::new(),
                    content: "abc".to_string(),
                }),
            ],
            block_defs: vec![],
        };

        let dxf = convert_document(&doc);
        let out = document_to_string(&dxf);

        assert!(out.contains("  2\nOBJECTS\n"));
        assert!(out.contains("  2\nBLOCK_RECORD\n"));
        assert!(out.contains("  2\n*Model_Space\n"));
        assert!(out.contains("  2\n*Paper_Space\n"));

        let handles = group_values_by_code(&out, 5);
        assert!(!handles.is_empty());
        let unique = handles.iter().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), handles.len());
        assert!(handles
            .iter()
            .all(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit())));
    }

    fn group_values_by_code(dxf: &str, target_code: i32) -> Vec<String> {
        let mut out = Vec::<String>::new();
        let mut lines = dxf.lines();
        while let Some(code_line) = lines.next() {
            let Some(value_line) = lines.next() else {
                break;
            };
            if code_line.trim().parse::<i32>().ok() == Some(target_code) {
                out.push(value_line.to_string());
            }
        }
        out
    }

    fn contains_line(entities: &[DxfEntity], x1: f64, y1: f64, x2: f64, y2: f64) -> bool {
        entities.iter().any(|entity| {
            if let DxfEntity::Line(line) = entity {
                nearly_eq(line.x1, x1)
                    && nearly_eq(line.y1, y1)
                    && nearly_eq(line.x2, x2)
                    && nearly_eq(line.y2, y2)
            } else {
                false
            }
        })
    }

    fn nearly_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }
}
