use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::f64::consts::PI;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;
use serde::Serializer;

use crate::header::JwwPalette;
use crate::model::{
    metadata_setting_from_text, Arc, Block, BlockDef, CircleSolid, Entity, JwwDocument, Solid, Text,
};

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
    pub line_weight: i32,
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
    pub line_weight: i32,
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfArc {
    pub layer: String,
    pub color: i32,
    pub line_type: String,
    pub line_weight: i32,
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
    pub line_weight: i32,
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
    pub end_x: f64,
    pub end_y: f64,
    pub height: f64,
    pub width_factor: f64,
    pub rotation: f64,
    pub content: String,
    pub style: String,
}

/// A filled quadrilateral.
///
/// The corners are kept in **polygon traversal order** (`1 -> 2 -> 3 -> 4`), *not* in the "Z" order DXF uses on the wire.
/// The ASCII writer is the only place that knows about the wire order: it emits `x4/y4` as group 12 and `x3/y3` as group 13.
///
/// Traversal order is best effort rather than an invariant.
/// `order_solid_vertices` only repairs an ordering it can prove self-crossing,
/// so collinear or degenerate corners reach here in whatever order the JWW file had them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DxfSolid {
    pub layer: String,
    pub color: i32,
    pub line_type: String,
    pub line_weight: i32,
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
    pub line_weight: i32,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum DxfTargetVersion {
    #[default]
    Ac1015,
    Ac1024,
}

impl DxfTargetVersion {
    pub const fn acad_version(self) -> &'static str {
        match self {
            Self::Ac1015 => "AC1015",
            Self::Ac1024 => "AC1024",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ConvertOptions {
    pub explode_inserts: bool,
    pub max_block_nesting: usize,
    /// Em the target renderer draws per unit of DXF text height (group 40).
    ///
    /// `1.0` is the spec's reading and sends 文字高さ out untouched.
    /// Raise it for a renderer that substitutes a TrueType face for a missing SHX font:
    /// SHX reads group 40 as cap height,
    /// so the face is scaled up to match and the em box -- what 文字高さ/文字幅 describe -- inflates in both axes.
    /// The value is that face's em over its cap height, and has to be measured per renderer and font.
    /// Group 41 comes from the JWW file either way.
    pub text_em_scale: f64,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            explode_inserts: false,
            max_block_nesting: 32,
            text_em_scale: 1.0,
        }
    }
}

pub fn convert_document(doc: &JwwDocument) -> DxfDocument {
    convert_document_with_options(doc, ConvertOptions::default())
}

pub fn convert_document_with_options(doc: &JwwDocument, options: ConvertOptions) -> DxfDocument {
    let layer_name_map = layer_name_map(doc);
    let layers = convert_layers(doc, &layer_name_map);
    let block_name_map = block_name_map(doc);
    let block_defs = block_defs_by_number(&doc.block_defs);

    let colors = ColorTable::new(doc.header.palette.as_ref());

    let mut unsupported_entities = Vec::<String>::new();
    let entities = if options.explode_inserts {
        let mut expanding_stack = Vec::new();
        let mut context = ExplodeContext {
            layer_names: &layer_name_map,
            block_name_map: &block_name_map,
            block_defs: &block_defs,
            unsupported_entities: &mut unsupported_entities,
            options,
            colors: &colors,
        };
        convert_entities_exploded(
            &mut context,
            &doc.entities,
            &Transform2D::identity(),
            &mut expanding_stack,
        )
    } else {
        convert_entities(
            &doc.entities,
            &layer_name_map,
            &block_name_map,
            &mut unsupported_entities,
            &colors,
            options,
        )
    };
    let blocks = if options.explode_inserts {
        Vec::new()
    } else {
        convert_blocks(
            doc,
            &layer_name_map,
            &block_name_map,
            &mut unsupported_entities,
            &colors,
            options,
        )
    };

    DxfDocument {
        layers,
        entities,
        blocks,
        unsupported_entities,
    }
}

pub fn document_to_string(doc: &DxfDocument) -> String {
    document_to_string_with_version(doc, DxfTargetVersion::default())
}

pub fn document_to_string_with_version(
    doc: &DxfDocument,
    target_version: DxfTargetVersion,
) -> String {
    let mut writer = AsciiDxfWriter::new(target_version);
    writer.write_document(doc);
    writer.finish()
}

pub fn write_document_to_file(doc: &DxfDocument, path: impl AsRef<Path>) -> io::Result<()> {
    write_document_to_file_with_version(doc, path, DxfTargetVersion::default())
}

pub fn write_document_to_file_with_version(
    doc: &DxfDocument,
    path: impl AsRef<Path>,
    target_version: DxfTargetVersion,
) -> io::Result<()> {
    let data = document_to_string_with_version(doc, target_version);
    fs::write(path, data)
}

struct AsciiDxfWriter {
    out: String,
    target_version: DxfTargetVersion,
    next_handle: u32,
    block_record_order: Vec<String>,
    block_record_handles: BTreeMap<String, String>,
}

impl AsciiDxfWriter {
    fn new(target_version: DxfTargetVersion) -> Self {
        Self {
            out: String::with_capacity(16 * 1024),
            target_version,
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
        self.group_str(1, self.target_version.acad_version());
        // Readers create handles for implicit SEQEND records (for example after
        // INSERT entities). Keep their generated range above every handle this
        // writer can practically allocate so those records never collide.
        self.group_str(9, "$HANDSEED");
        self.group_str(5, "FFFFFFFF");
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
                self.entity_header_with_line_weight(
                    "LINE",
                    &v.layer,
                    v.color,
                    &v.line_type,
                    v.line_weight,
                    owner_handle,
                );
                self.group_f64(10, v.x1);
                self.group_f64(20, v.y1);
                self.group_f64(30, 0.0);
                self.group_f64(11, v.x2);
                self.group_f64(21, v.y2);
                self.group_f64(31, 0.0);
            }
            DxfEntity::Circle(v) => {
                self.entity_header_with_line_weight(
                    "CIRCLE",
                    &v.layer,
                    v.color,
                    &v.line_type,
                    v.line_weight,
                    owner_handle,
                );
                self.group_f64(10, v.center_x);
                self.group_f64(20, v.center_y);
                self.group_f64(30, 0.0);
                self.group_f64(40, v.radius);
            }
            DxfEntity::Arc(v) => {
                self.entity_header_with_line_weight(
                    "ARC",
                    &v.layer,
                    v.color,
                    &v.line_type,
                    v.line_weight,
                    owner_handle,
                );
                self.group_f64(10, v.center_x);
                self.group_f64(20, v.center_y);
                self.group_f64(30, 0.0);
                self.group_f64(40, v.radius);
                self.group_f64(50, v.start_angle);
                self.group_f64(51, v.end_angle);
            }
            DxfEntity::Ellipse(v) => {
                self.entity_header_with_line_weight(
                    "ELLIPSE",
                    &v.layer,
                    v.color,
                    &v.line_type,
                    v.line_weight,
                    owner_handle,
                );
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
                self.group_f64(41, v.width_factor);
                self.group_str(7, &escape_unicode(&v.style));
            }
            DxfEntity::Solid(v) => {
                self.entity_header_with_line_weight(
                    "SOLID",
                    &v.layer,
                    v.color,
                    &v.line_type,
                    v.line_weight,
                    owner_handle,
                );
                self.group_f64(10, v.x1);
                self.group_f64(20, v.y1);
                self.group_f64(30, 0.0);
                self.group_f64(11, v.x2);
                self.group_f64(21, v.y2);
                self.group_f64(31, 0.0);
                // DXF stores SOLID corners in "Z" order: group 12 is the 4th corner, group 13 the 3rd.
                // Writing traversal order verbatim would make a convex quadrilateral render as a bowtie.
                self.group_f64(12, v.x4);
                self.group_f64(22, v.y4);
                self.group_f64(32, 0.0);
                self.group_f64(13, v.x3);
                self.group_f64(23, v.y3);
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
            self.entity_header_with_line_weight(
                "SOLID",
                &polygon.layer,
                polygon.color,
                &polygon.line_type,
                polygon.line_weight,
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
        self.entity_header_with_line_weight(entity_type, layer, color, line_type, -3, owner_handle);
    }

    fn entity_header_with_line_weight(
        &mut self,
        entity_type: &str,
        layer: &str,
        color: i32,
        line_type: &str,
        line_weight: i32,
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
        if line_weight >= 0 {
            self.group_i32(370, line_weight);
        }
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
    layer_names: &'a HashMap<(u16, u16), String>,
    block_name_map: &'a HashMap<u32, String>,
    block_defs: &'a HashMap<u32, &'a BlockDef>,
    unsupported_entities: &'a mut Vec<String>,
    options: ConvertOptions,
    colors: &'a ColorTable,
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
            _ => match convert_entity(
                entity,
                context.layer_names,
                context.block_name_map,
                context.colors,
                context.options,
            ) {
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
                line_weight: v.line_weight,
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
            let (end_x, end_y) = transform.apply_point(v.end_x, v.end_y);
            let height = (v.height * transform.average_scale().abs()).max(0.1);
            let width_factor = exploded_width_factor(v, height, (end_x - x).hypot(end_y - y));
            vec![DxfEntity::Text(DxfText {
                layer: v.layer.clone(),
                color: v.color,
                line_type: v.line_type.clone(),
                x,
                y,
                end_x,
                end_y,
                height,
                width_factor,
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
                line_weight: v.line_weight,
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
            line_weight: v.line_weight,
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
            line_weight: circle.line_weight,
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
        line_weight: circle.line_weight,
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

    points_to_lines(
        points,
        arc.layer.clone(),
        arc.color,
        arc.line_type.clone(),
        arc.line_weight,
    )
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
        ellipse.line_weight,
    )
}

fn points_to_lines(
    points: Vec<(f64, f64)>,
    layer: String,
    color: i32,
    line_type: String,
    line_weight: i32,
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
            line_weight,
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

fn convert_layers(doc: &JwwDocument, layer_names: &HashMap<(u16, u16), String>) -> Vec<DxfLayer> {
    let mut layers = Vec::<DxfLayer>::with_capacity(16 * 16);
    for g in 0..16 {
        for l in 0..16 {
            let layer = &doc.header.layer_groups[g].layers[l];
            let name = layer_names
                .get(&(g as u16, l as u16))
                .cloned()
                .unwrap_or_else(|| format!("{g:X}-{l:X}"));
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
    layer_names: &HashMap<(u16, u16), String>,
    block_name_map: &HashMap<u32, String>,
    unsupported_entities: &mut Vec<String>,
    colors: &ColorTable,
    options: ConvertOptions,
) -> Vec<DxfBlock> {
    let mut blocks = Vec::<DxfBlock>::with_capacity(doc.block_defs.len());
    for block_def in &doc.block_defs {
        let name = block_name_map
            .get(&block_def.number)
            .cloned()
            .unwrap_or_else(|| format!("BLOCK_{}", block_def.number));
        let entities = convert_entities(
            &block_def.entities,
            layer_names,
            block_name_map,
            unsupported_entities,
            colors,
            options,
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
    entities: &[Entity],
    layer_names: &HashMap<(u16, u16), String>,
    block_name_map: &HashMap<u32, String>,
    unsupported_entities: &mut Vec<String>,
    colors: &ColorTable,
    options: ConvertOptions,
) -> Vec<DxfEntity> {
    let mut out = Vec::<DxfEntity>::new();
    for entity in entities {
        match convert_entity(entity, layer_names, block_name_map, colors, options) {
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
    entity: &Entity,
    layer_names: &HashMap<(u16, u16), String>,
    block_name_map: &HashMap<u32, String>,
    colors: &ColorTable,
    options: ConvertOptions,
) -> Option<Vec<DxfEntity>> {
    let base = entity.base();
    let layer = layer_name(layer_names, base.layer_group, base.layer);
    let color = colors.aci(base.pen_color);
    let line_type = map_line_type(base.pen_style).to_string();
    let line_weight = map_line_weight(base.pen_width);

    match entity {
        Entity::Line(v) => Some(vec![DxfEntity::Line(DxfLine {
            layer,
            color,
            line_type,
            line_weight,
            x1: v.start_x,
            y1: v.start_y,
            x2: v.end_x,
            y2: v.end_y,
        })]),
        Entity::Arc(v) => Some(convert_arc(v, layer, color, line_type, line_weight)),
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
        Entity::Text(v) => {
            if metadata_setting_from_text(v).is_some() {
                Some(Vec::new())
            } else {
                Some(vec![DxfEntity::Text(convert_text(
                    v,
                    layer,
                    color,
                    line_type,
                    options.text_em_scale,
                ))])
            }
        }
        // Pen color 10 marks a solid painted in an arbitrary color, and the exact COLORREF is stored on the entity itself.
        // That beats anything the palette could tell us, so it wins over `color` when present.
        Entity::Solid(v) => Some(vec![DxfEntity::Solid(convert_solid(
            v,
            layer,
            v.color.map_or(color, rgb_to_aci),
            line_type,
            line_weight,
        ))]),
        Entity::CircleSolid(v) => Some(convert_circle_solid(
            v,
            layer,
            v.color.map_or(color, rgb_to_aci),
            line_type,
            line_weight,
        )),
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
                line_weight,
                x1: v.line.start_x,
                y1: v.line.start_y,
                x2: v.line.end_x,
                y2: v.line.end_y,
            }),
            DxfEntity::Text(convert_text(
                &v.text,
                layer,
                color,
                line_type,
                options.text_em_scale,
            )),
        ]),
    }
}

fn convert_solid(
    solid: &Solid,
    layer: String,
    color: i32,
    line_type: String,
    line_weight: i32,
) -> DxfSolid {
    // Jw_cad writes the corners as `m_start, m_end, m_DPoint2, m_DPoint3` and that file order is the traversal order.
    // `parse_solid` names them `point1, point4, point2, point3`, so the fields have to be reordered back here.
    let points = order_solid_vertices([
        DxfVertex {
            x: solid.point1_x,
            y: solid.point1_y,
        },
        DxfVertex {
            x: solid.point4_x,
            y: solid.point4_y,
        },
        DxfVertex {
            x: solid.point2_x,
            y: solid.point2_y,
        },
        DxfVertex {
            x: solid.point3_x,
            y: solid.point3_y,
        },
    ]);

    // `points` is in polygon traversal order, which is exactly how `DxfSolid` stores
    // its corners. Translating that to the DXF "Z" order is the writer's job.
    DxfSolid {
        layer,
        color,
        line_type,
        line_weight,
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
    line_weight: i32,
) -> Vec<DxfEntity> {
    if solid.radius.abs() <= 1e-12 {
        return Vec::new();
    }

    if matches!(solid.base.pen_style, 105 | 106) {
        return convert_ring_solid(solid, layer, color, line_type, line_weight);
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

    filled_polygon(layer, color, line_type, line_weight, points)
}

fn convert_ring_solid(
    solid: &CircleSolid,
    layer: String,
    color: i32,
    line_type: String,
    line_weight: i32,
) -> Vec<DxfEntity> {
    let outer_radius = solid.radius.abs();
    let inner_radius = solid.solid_mode.abs();
    if inner_radius <= 1e-12 || inner_radius >= outer_radius {
        return filled_polygon(
            layer,
            color,
            line_type,
            line_weight,
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
            line_weight,
            vec![outer1, outer2, inner2, inner1],
        ));
    }

    out
}

fn filled_polygon(
    layer: String,
    color: i32,
    line_type: String,
    line_weight: i32,
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
        line_weight,
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

fn convert_arc(
    arc: &Arc,
    layer: String,
    color: i32,
    line_type: String,
    line_weight: i32,
) -> Vec<DxfEntity> {
    if arc.is_full_circle && arc.flatness == 1.0 {
        return vec![DxfEntity::Circle(DxfCircle {
            layer,
            color,
            line_type,
            line_weight,
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
        let (span_start, span_end) = jww_arc_to_ccw_span(arc.start_angle, arc.arc_angle);
        let start_param = if arc.is_full_circle { 0.0 } else { span_start };
        let end_param = if arc.is_full_circle {
            2.0 * PI
        } else {
            span_end
        };

        return vec![DxfEntity::Ellipse(DxfEllipse {
            layer,
            color,
            line_type,
            line_weight,
            center_x: arc.center_x,
            center_y: arc.center_y,
            major_axis_x,
            major_axis_y,
            minor_ratio,
            start_param,
            end_param,
        })];
    }

    let (start_angle, end_angle) = jww_arc_to_ccw_span(arc.start_angle, arc.arc_angle);
    vec![DxfEntity::Arc(DxfArc {
        layer,
        color,
        line_type,
        line_weight,
        center_x: arc.center_x,
        center_y: arc.center_y,
        radius: arc.radius,
        start_angle: normalize_degrees(rad_to_deg(start_angle)),
        end_angle: normalize_degrees(rad_to_deg(end_angle)),
    })]
}

/// Guard rails for [`text_box`] against malformed records.
const MIN_TEXT_WIDTH_FACTOR: f64 = 0.05;
const MAX_TEXT_WIDTH_FACTOR: f64 = 20.0;

/// Below this, a width correction is f64 round-off rather than a real difference in the two insert axes.
const UNIFORM_SCALE_EPSILON: f64 = 1e-9;

/// Code points Jw_cad pitches at half a cell: CP932's single-byte set.
const HALF_WIDTH_RANGES: [(u32, u32); 5] = [
    (0x0020, 0x0080), // ASCII printable, plus the 0x80 CP932 still maps single-byte
    (0x00A5, 0x00A5), // ¥ -- the 0x5C position
    (0x203E, 0x203E), // ‾ -- the 0x7E position
    (0xF8F0, 0xF8F3), // CP932's private-use stand-ins for its undefined single bytes
    (0xFF61, 0xFF9F), // half-width katakana
];

/// Cells one character occupies.
fn char_cell_width(c: char) -> f64 {
    let cp = c as u32;
    if HALF_WIDTH_RANGES
        .iter()
        .any(|&(lo, hi)| (lo..=hi).contains(&cp))
    {
        0.5
    } else {
        1.0
    }
}

/// Width of `content` in Jw_cad character cells.
///
/// A half-width character takes half a cell and everything else a full one,
/// which is how Jw_cad itself pitches a string. See [`HALF_WIDTH_RANGES`] for what counts as half-width.
///
/// # Examples
///
/// ```
/// # use ezjww_core::dxf::text_cell_width;
/// assert_eq!(text_cell_width("ＡＢ米米"), 4.0);
/// assert_eq!(text_cell_width("(04)"), 2.0);
/// assert_eq!(text_cell_width("〒100-0000"), 5.0);
/// assert_eq!(text_cell_width("—🙂"), 2.0);
/// ```
pub fn text_cell_width(content: &str) -> f64 {
    content.chars().map(char_cell_width).sum()
}

/// Height fallback for records that carry no usable 文字高さ.
const DEFAULT_TEXT_HEIGHT: f64 = 2.5;

/// The DXF text height (group 40) and width factor (group 41) that make a renderer draw `text` at the size Jw_cad recorded for it.
///
/// The two only work as a pair: group 40 is pre-divided by `em_scale` so the drawn em box lands back on 文字高さ,
/// and group 41 then corrects the pitch of that already-corrected advance.
/// Fixing one axis alone leaves the other stretched.
/// The width factor therefore comes out the same for any `em_scale`.
///
/// The intended width comes from the endpoint Jw_cad stores, not from its `size_x * cells` pitch:
/// the pitch model only holds for the built-in font, and records naming a TrueType font disagree with it by up to 50% across the sample corpus.
fn text_box(text: &Text, em_scale: f64) -> (f64, f64) {
    let size = if text.size_y <= 0.0 {
        DEFAULT_TEXT_HEIGHT
    } else {
        text.size_y
    };
    let height = if em_scale > 0.0 {
        size / em_scale
    } else {
        size
    };

    let cells = text_cell_width(&text.content);
    let intended = (text.end_x - text.start_x).hypot(text.end_y - text.start_y);
    if cells <= 0.0 || intended <= 0.0 {
        return (height, 1.0);
    }

    // The renderer's advance for one cell is `size` whatever `em_scale` was.
    let factor = (intended / (size * cells)).clamp(MIN_TEXT_WIDTH_FACTOR, MAX_TEXT_WIDTH_FACTOR);
    (height, factor)
}

/// Group 41 scales exploded TEXT width to fit between its transformed endpoints.
///
/// Because [`transform_entity_for_explode`] approximates text height (group 40) using an averaged and clamped scale, non-uniform INSERTs introduce a width error.
/// Recalculating group 41 based on actual endpoint movement corrects this width (pitch) without changing the approximated height.
/// This safely acts as a no-op for uniform inserts.
fn exploded_width_factor(text: &DxfText, height: f64, span: f64) -> f64 {
    let original_span = (text.end_x - text.x).hypot(text.end_y - text.y);
    if original_span <= 0.0 || span <= 0.0 || text.height <= 0.0 || height <= 0.0 {
        return text.width_factor;
    }
    let correction = (span / original_span) * (text.height / height);
    // A uniform insert cancels to 1 in exact arithmetic but not always in the last digit of an f64,
    // and re-emitting a factor that only moved there would churn the output for no geometric reason.
    if (correction - 1.0).abs() <= UNIFORM_SCALE_EPSILON {
        return text.width_factor;
    }
    (text.width_factor * correction).clamp(MIN_TEXT_WIDTH_FACTOR, MAX_TEXT_WIDTH_FACTOR)
}

fn convert_text(
    text: &Text,
    layer: String,
    color: i32,
    line_type: String,
    em_scale: f64,
) -> DxfText {
    let (height, width_factor) = text_box(text, em_scale);
    DxfText {
        layer,
        color,
        line_type,
        x: text.start_x,
        y: text.start_y,
        end_x: text.end_x,
        end_y: text.end_y,
        height,
        width_factor,
        rotation: text.angle,
        content: text.content.clone(),
        style: "STANDARD".to_string(),
    }
}

fn block_name_map(doc: &JwwDocument) -> HashMap<u32, String> {
    let mut map = HashMap::<u32, String>::with_capacity(doc.block_defs.len());
    let mut used =
        BTreeSet::<String>::from(["*Model_Space".to_string(), "*Paper_Space".to_string()]);
    for block_def in &doc.block_defs {
        let fallback = format!("BLOCK_{}", block_def.number);
        let candidate = sanitize_dxf_table_name(&block_def.name, &fallback);
        let name = unique_dxf_table_name(candidate, &format!("_{}", block_def.number), &mut used);
        map.insert(block_def.number, name);
    }
    map
}

fn layer_name_map(doc: &JwwDocument) -> HashMap<(u16, u16), String> {
    let mut map = HashMap::<(u16, u16), String>::with_capacity(16 * 16);
    let mut used = BTreeSet::<String>::from(["0".to_string()]);
    for g in 0..16 {
        for l in 0..16 {
            let fallback = format!("{g:X}-{l:X}");
            let raw = &doc.header.layer_groups[g].layers[l].name;
            let candidate = sanitize_dxf_table_name(raw, &fallback);
            let name = unique_dxf_table_name(candidate, &format!("_{fallback}"), &mut used);
            map.insert((g as u16, l as u16), name);
        }
    }
    map
}

fn sanitize_dxf_table_name(raw: &str, fallback: &str) -> String {
    let mut name = raw
        .trim()
        .chars()
        .take(255)
        .map(|character| {
            if character.is_control() || "<>/\\\":;?*|=".contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    if name.trim_matches('_').trim().is_empty() {
        name = fallback.to_string();
    }
    name
}

fn unique_dxf_table_name(candidate: String, suffix: &str, used: &mut BTreeSet<String>) -> String {
    if used.insert(candidate.clone()) {
        return candidate;
    }
    let keep = 255usize.saturating_sub(suffix.chars().count());
    let base = candidate.chars().take(keep).collect::<String>();
    let mut unique = format!("{base}{suffix}");
    let mut collision = 2usize;
    while !used.insert(unique.clone()) {
        let numbered_suffix = format!("{suffix}_{collision}");
        let keep = 255usize.saturating_sub(numbered_suffix.chars().count());
        unique = format!(
            "{}{}",
            candidate.chars().take(keep).collect::<String>(),
            numbered_suffix
        );
        collision += 1;
    }
    unique
}

fn layer_name(layer_names: &HashMap<(u16, u16), String>, layer_group: u16, layer: u16) -> String {
    layer_names
        .get(&(layer_group, layer))
        .cloned()
        .unwrap_or_else(|| format!("{layer_group:X}-{layer:X}"))
}

/// The DXF color number (ACI) of every JWW pen color number a document can use.
///
/// A drawing can reference hundreds of palette slots and contain hundreds of thousands of
/// entities. Resolving one COLORREF scans all 255 ACI values, so the answers are worked out
/// once per document instead of once per entity.
struct ColorTable {
    /// ACI for pen color numbers 0..=9. Number 0 is the screen background, which no entity draws
    /// with and which `JwwPalette::screen_color` does not report, so slot 0 holds the fallback ACI; keeping it makes the lookup a plain index.
    basic: [i32; 10],
    /// ACI for extended pen colors 100..=356, `None` when the file stores no
    /// extended palette.
    extended: Option<Box<[i32]>>,
}

impl ColorTable {
    /// Resolves every pen color number the palette defines.
    ///
    /// When the header palette is available, picks the ACI closest to the RGB value the file actually records.
    /// Falls back to the fixed table only when it is not, which costs nothing extra because `map_color_fallback` is a plain match.
    fn new(palette: Option<&JwwPalette>) -> Self {
        let resolve = |pen_color: u16| match palette.and_then(|p| p.screen_color(pen_color)) {
            Some(rgb) => rgb_to_aci(rgb),
            None => map_color_fallback(pen_color),
        };

        let mut basic = [0; 10];
        for (pen_color, aci) in basic.iter_mut().enumerate() {
            *aci = resolve(pen_color as u16);
        }

        let extended = palette.and_then(|p| p.extended_colors.as_ref()).map(|_| {
            let mut table = vec![0; 257].into_boxed_slice();
            for (offset, aci) in table.iter_mut().enumerate() {
                *aci = resolve(100 + offset as u16);
            }
            table
        });

        Self { basic, extended }
    }

    /// ACI for a pen color number.
    ///
    /// The ranges matched here mirror the ones `JwwPalette::screen_color` reports, and have to be kept in step with it:
    /// a number this match does not know about silently takes the fallback table even when the palette defines it.
    /// `color_table_matches_resolving_on_demand` pins that.
    fn aci(&self, pen_color: u16) -> i32 {
        match pen_color {
            0..=9 => self.basic[pen_color as usize],
            100..=356 => self.extended.as_ref().map_or_else(
                || map_color_fallback(pen_color),
                |table| table[(pen_color - 100) as usize],
            ),
            _ => map_color_fallback(pen_color),
        }
    }
}

/// RGB of a DXF color number (ACI).
///
/// The AutoCAD Color Index is a fixed palette defined outside this crate.
/// 1..=9 are primaries and 250..=255 a gray ramp, both listed here. 10..=249 is 24 hues x 10 shades laid out on a regular grid,
/// so it is generated rather than tabulated: `LEVEL` is the brightness of each shade pair, odd shades are
/// the half saturation variant, and the 24 hues walk the six RGB sectors in quarter steps.
fn aci_rgb(aci: i32) -> (u8, u8, u8) {
    const PRIMARY: [(u8, u8, u8); 9] = [
        (0xFF, 0x00, 0x00),
        (0xFF, 0xFF, 0x00),
        (0x00, 0xFF, 0x00),
        (0x00, 0xFF, 0xFF),
        (0x00, 0x00, 0xFF),
        (0xFF, 0x00, 0xFF),
        (0xFF, 0xFF, 0xFF),
        (0x80, 0x80, 0x80),
        (0xC0, 0xC0, 0xC0),
    ];
    const GRAY: [u8; 6] = [0x54, 0x76, 0x98, 0xBB, 0xDD, 0xFF];
    const LEVEL: [f64; 5] = [1.0, 0.65, 0.5, 0.3, 0.15];

    match aci {
        1..=9 => PRIMARY[aci as usize - 1],
        10..=249 => {
            let (group, shade) = ((aci - 10) / 10, (aci - 10) % 10);
            let hi = LEVEL[shade as usize / 2];
            let lo = if shade % 2 == 1 { hi / 2.0 } else { 0.0 };
            let step = f64::from(group % 4) / 4.0;
            let rise = lo + (hi - lo) * step;
            let fall = hi - (hi - lo) * step;
            let (r, g, b) = match group / 4 {
                0 => (hi, rise, lo),
                1 => (fall, hi, lo),
                2 => (lo, hi, rise),
                3 => (lo, fall, hi),
                4 => (rise, lo, hi),
                _ => (hi, lo, fall),
            };
            let q = |v: f64| (v * 255.0).round() as u8;
            (q(r), q(g), q(b))
        }
        250..=255 => {
            let v = GRAY[aci as usize - 250];
            (v, v, v)
        }
        _ => (0, 0, 0),
    }
}

/// Picks the nearest ACI number for a COLORREF (0x00BBGGRR).
///
/// ACI 7 is listed as white, but it is also the default ink and is drawn black on the white background a DXF is normally viewed on.
/// Jw_cad in turn draws in white on its black screen background, so both ends of the ramp belong on 7.
/// Black has to be pinned there rather than searched for:
/// the palette holds no black — its darkest entry is ACI 250 at (84, 84, 84) — so the nearest ACI to #000000 is 18, a dark red.
fn rgb_to_aci(color: u32) -> i32 {
    let r = (color & 0xFF) as i32;
    let g = ((color >> 8) & 0xFF) as i32;
    let b = ((color >> 16) & 0xFF) as i32;

    // Near-neutral, and at one end or the other. The cutoffs are empirical but boxed in on both sides:
    // grays from 67 up already reach a neutral ACI on their own, and #C0C0C0 has to stay 9.
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    if max - min < 24 && (max < 40 || min > 215) {
        return 7;
    }

    (1..=255)
        .min_by_key(|&aci| {
            let (tr, tg, tb) = aci_rgb(aci);
            let (dr, dg, db) = (r - i32::from(tr), g - i32::from(tg), b - i32::from(tb));
            dr * dr + dg * dg + db * db
        })
        .unwrap_or(7)
}

/// The previous fixed table, kept for files whose palette could not be read.
///
/// Checked against real files its hues rarely match,
/// but it is still better than dropping color entirely when the palette is unavailable.
fn map_color_fallback(pen_color: u16) -> i32 {
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

fn map_line_weight(pen_width: u16) -> i32 {
    if pen_width == 0 {
        -3
    } else {
        i32::from(pen_width).clamp(0, 211)
    }
}

fn rad_to_deg(rad: f64) -> f64 {
    rad * 180.0 / PI
}

fn jww_arc_to_ccw_span(start_angle: f64, arc_angle: f64) -> (f64, f64) {
    if arc_angle < 0.0 {
        (start_angle + arc_angle, start_angle)
    } else {
        (start_angle, start_angle + arc_angle)
    }
}

fn normalize_degrees(degrees: f64) -> f64 {
    degrees.rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use std::array;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::header::{JwwHeader, JwwPalette, LayerGroupHeader, LayerHeader};
    use crate::model::{
        Arc, Block, BlockDef, CircleSolid, Entity, EntityBase, JwwDocument, Line, Solid, Text,
    };
    use crate::parser::read_document_from_file;

    use super::{
        aci_rgb, convert_document, convert_document_with_options, document_to_string,
        document_to_string_with_version, map_color_fallback, map_line_type, rgb_to_aci,
        solid_vertices_cross, text_box, text_cell_width, ColorTable, ConvertOptions, DxfDocument,
        DxfEntity, DxfLayer, DxfSolid, DxfTargetVersion, DxfText, DxfVertex, DEFAULT_TEXT_HEIGHT,
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
            palette: None,
        }
    }

    fn jww_samples_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../jww_samples")
    }

    /// Anchors the generated palette against known AutoCAD Color Index values.
    ///
    /// `rgb_to_aci` only returns numbers, so a wrong RGB here would be invisible to the tests below:
    /// they would happily agree on a number that renders as the wrong color.
    /// Every entry is a published ACI value, independent of how `aci_rgb` computes it.
    ///
    /// Source: <https://www.temblast.com/songview/color3.htm>. Autodesk publishes no official table,
    /// and the variants in circulation disagree: <https://gohtx.com/acadcolors.php> is a different lineage,
    /// and some tables are corrected for AutoCAD's default dark model space background.
    /// Only the table above matches this palette.
    #[test]
    fn aci_rgb_matches_known_palette_values() {
        let cases = [
            (1, (0xFF, 0x00, 0x00)),
            (5, (0x00, 0x00, 0xFF)),
            (7, (0xFF, 0xFF, 0xFF)),
            (8, (0x80, 0x80, 0x80)),
            (9, (0xC0, 0xC0, 0xC0)),
            (10, (0xFF, 0x00, 0x00)),
            (12, (0xA6, 0x00, 0x00)),
            (16, (0x4D, 0x00, 0x00)),
            (17, (0x4D, 0x26, 0x26)),
            (20, (0xFF, 0x40, 0x00)),
            (23, (0xA6, 0x68, 0x53)),
            (30, (0xFF, 0x80, 0x00)),
            (65, (0x70, 0x80, 0x40)),
            (100, (0x00, 0xFF, 0x40)),
            (102, (0x00, 0xA6, 0x29)),
            (108, (0x00, 0x26, 0x0A)),
            (131, (0x80, 0xFF, 0xFF)),
            (134, (0x00, 0x80, 0x80)),
            (150, (0x00, 0x80, 0xFF)),
            (179, (0x13, 0x13, 0x26)),
            (221, (0xFF, 0x80, 0xDF)),
            (230, (0xFF, 0x00, 0x80)),
            (250, (0x54, 0x54, 0x54)),
            (255, (0xFF, 0xFF, 0xFF)),
        ];
        for (aci, expected) in cases {
            assert_eq!(aci_rgb(aci), expected, "wrong RGB for ACI {aci}");
        }
    }

    #[test]
    fn palette_colors_resolve_to_a_near_identical_aci() {
        // (COLORREF, tolerated RGB distance)
        let cases = [
            (0x00FF_FF00, 0),  // #00FFFF cyan
            (0x00C0_C000, 40), // #00C0C0 darker cyan
            (0x0000_C000, 30), // #00C000 green
            (0x0000_FF00, 0),  // #00FF00 green
            (0x0040_FF00, 0),  // #00FF40 green
            (0x0000_FFFF, 0),  // #FFFF00 yellow
            (0x0000_C0C0, 40), // #C0C000 yellow
            (0x00C0_00C0, 40), // #C000C0 magenta
            (0x00FF_0000, 0),  // #0000FF blue
            (0x00FF_2020, 50), // #2020FF blue
            (0x0080_8000, 0),  // #008080 teal
            (0x0080_00FF, 0),  // #FF0080 pink
            (0x0000_00A0, 10), // #A00000 dark red
            (0x0080_8080, 0),  // #808080 gray
            (0x00C0_C0C0, 0),  // #C0C0C0 light gray
        ];
        for (color, tolerance) in cases {
            let palette = JwwPalette {
                pen_colors: [color; 10],
                extended_colors: None,
            };
            let aci = ColorTable::new(Some(&palette)).aci(1);
            let (r, g, b) = aci_rgb(aci);
            let (dr, dg, db) = (
                (color & 0xFF) as i32 - i32::from(r),
                ((color >> 8) & 0xFF) as i32 - i32::from(g),
                ((color >> 16) & 0xFF) as i32 - i32::from(b),
            );
            let distance = ((dr * dr + dg * dg + db * db) as f64).sqrt();
            // this fails if the palette is wrong, not just if the choice changes.
            assert!(
                distance <= f64::from(tolerance),
                "COLORREF {color:#010X} resolved to ACI {aci} at distance {distance:.0}, \
                 which is worse than the tolerated {tolerance}"
            );
        }
    }

    #[test]
    fn white_and_black_collapse_to_default_ink() {
        let ink = |color: u32| {
            let palette = JwwPalette {
                pen_colors: [color; 10],
                extended_colors: None,
            };
            ColorTable::new(Some(&palette)).aci(1)
        };
        assert_eq!(ink(0x0000_0000), 7); // #000000
        assert_eq!(ink(0x00FF_FFFF), 7); // #FFFFFF
                                         // Grays in between stay gray rather than collapsing.
        assert_ne!(ink(0x0080_8080), 7); // #808080
        assert_ne!(ink(0x00C0_C0C0), 7); // #C0C0C0
        assert_ne!(ink(0x0040_4040), 7); // #404040
    }

    #[test]
    fn all_extended_colors_map_through_palette() {
        let mut extended = vec![0u32; 257].into_boxed_slice();
        extended[2] = 0x0000_00FF; // 102 = red
        extended[5] = 0x0000_FFFF; // 105 = yellow
        extended[8] = 0x00FF_FFFF; // 108 = white
        extended[16] = 0x0080_8080; // 116 = darkgray, the last standard color
        extended[17] = 0x0000_00C0; // 117 = user-defined dark red
        extended[157] = 0x00BF_00FF; // 257 = user-defined #FF00BF
        let palette = JwwPalette {
            pen_colors: [0; 10],
            extended_colors: Some(extended),
        };
        let colors = ColorTable::new(Some(&palette));
        assert_eq!(aci_rgb(colors.aci(102)), (0xFF, 0x00, 0x00));
        assert_eq!(aci_rgb(colors.aci(105)), (0xFF, 0xFF, 0x00));
        assert_eq!(colors.aci(108), 7); // white becomes black ink
        assert_eq!(aci_rgb(colors.aci(116)), (0x80, 0x80, 0x80));
        assert_eq!(colors.aci(117), 12);
        assert_eq!(colors.aci(257), 220);
    }

    #[test]
    fn color_table_falls_back_without_palette() {
        let none = ColorTable::new(None);
        assert_eq!(none.aci(1), 7);
        assert_eq!(none.aci(2), 5);
        assert_eq!(none.aci(9), 8);
        assert_eq!(none.aci(108), 108);

        // Same when a palette exists but does not define that number:
        // pen color 0 is the background and 357 is past the extended palette.
        let palette = JwwPalette {
            pen_colors: [0x00FF_FFFF; 10],
            extended_colors: Some(vec![0; 257].into_boxed_slice()),
        };
        let colors = ColorTable::new(Some(&palette));
        assert_eq!(colors.aci(0), 1);
        assert_eq!(colors.aci(357), 102);

        // A file below version 420 carries pen colors but no SXF palette,
        // so the SXF numbers fall back even though a palette was read.
        let no_extended = JwwPalette {
            pen_colors: [0x00FF_FFFF; 10],
            extended_colors: None,
        };
        let colors = ColorTable::new(Some(&no_extended));
        assert_eq!(colors.aci(105), 105);
        assert_eq!(colors.aci(116), 116);
        assert_eq!(colors.aci(257), 2);
    }

    #[test]
    fn color_table_matches_resolving_on_demand() {
        let palettes = [
            None,
            Some(JwwPalette {
                pen_colors: [0; 10],
                extended_colors: None,
            }),
            Some(JwwPalette {
                pen_colors: [0x00FF_FFFF; 10],
                extended_colors: Some(vec![0; 257].into_boxed_slice()),
            }),
            Some(JwwPalette {
                pen_colors: [
                    0,
                    0x00C0_C000,
                    0x0000_00FF,
                    0x0000_C000,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0x00C0_C0C0,
                ],
                extended_colors: Some({
                    let mut colors = vec![0; 257].into_boxed_slice();
                    colors[1..=16].copy_from_slice(&[
                        0x0000_0000,
                        0x0000_00FF,
                        0x0000_8000,
                        0x0000_FF00,
                        0x0000_FFFF,
                        0x00FF_0000,
                        0x00FF_00FF,
                        0x00FF_FFFF,
                        0x0000_0080,
                        0x0000_4080,
                        0x0080_8000,
                        0x0080_0000,
                        0x0080_0080,
                        0x0000_8080,
                        0x00C0_C0C0,
                        0x0080_8080,
                    ]);
                    colors[17] = 0x0000_00C0;
                    colors[157] = 0x00BF_00FF;
                    colors
                }),
            }),
        ];

        for palette in &palettes {
            let colors = ColorTable::new(palette.as_ref());
            for pen_color in 0u16..=1000 {
                let expected = match palette.as_ref().and_then(|p| p.screen_color(pen_color)) {
                    Some(rgb) => rgb_to_aci(rgb),
                    None => map_color_fallback(pen_color),
                };
                assert_eq!(
                    colors.aci(pen_color),
                    expected,
                    "pen color {pen_color} disagrees with an on demand lookup"
                );
            }
        }
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

    /// A `Text` spanning `start_x` to `end_x` on the X axis, as Jw_cad records it.
    fn text_span(content: &str, start_x: f64, end_x: f64, size: f64) -> Text {
        Text {
            base: EntityBase::default(),
            start_x,
            start_y: 0.0,
            end_x,
            end_y: 0.0,
            text_type: 0,
            size_x: size,
            size_y: size,
            spacing: 0.0,
            angle: 0.0,
            font_name: String::new(),
            content: content.to_string(),
        }
    }

    #[test]
    fn text_cell_width_counts_half_width_characters_as_half_a_cell() {
        assert_eq!(text_cell_width("ＡＢ米米"), 4.0);
        assert_eq!(text_cell_width("(04)"), 2.0);
        // Half-width katakana is one CP932 byte,
        // same as ASCII -- and its dakuten is a separate character, so this is six cells' worth of half.
        assert_eq!(text_cell_width("ﾓｼﾞｭｰﾙ"), 3.0);
        assert_eq!(text_cell_width("〒100-0000"), 5.0);
        assert_eq!(text_cell_width(""), 0.0);
    }

    #[test]
    fn text_cell_width_counts_characters_outside_cp932_as_one_cell() {
        for content in ["—", "🙂", "𠮟", "\u{FFFD}"] {
            assert_eq!(text_cell_width(content), 1.0, "{content}");
        }
        // Latin-1 letters are outside CP932 too, so `é` takes a full cell
        // while the ASCII around it takes a half each.
        assert_eq!(text_cell_width("café"), 2.5);
    }

    #[test]
    fn text_cell_width_keeps_the_cp932_single_byte_aliases_half_width() {
        assert_eq!(text_cell_width("¥"), 0.5);
        assert_eq!(text_cell_width("‾"), 0.5);
    }

    /// A renderer that inflates a substituted CJK face, for the tests below.
    const INFLATING_EM_SCALE: f64 = 1.364;

    /// The em box a renderer with the given `em_scale` actually draws from what `text_box` emits.
    fn rendered_em_box(text: &Text, cells: f64, em_scale: f64) -> (f64, f64) {
        let (height, factor) = text_box(text, em_scale);
        let em = em_scale * height;
        (em * factor * cells, em)
    }

    #[test]
    fn text_box_hits_the_size_jww_recorded() {
        // 36 full-width cells at 文字高さ 3, spanning exactly 108 units.
        let text = text_span(&"あ".repeat(36), -56.27, 51.73, 3.0);

        // Both renderers have to land on the same drawn size.
        for em_scale in [1.0, INFLATING_EM_SCALE] {
            let (width, height) = rendered_em_box(&text, 36.0, em_scale);
            assert!(
                (width - 108.0).abs() < 1e-9,
                "expected the corrected width to land on 108, got {width} at {em_scale}"
            );
            assert!(
                (height - 3.0).abs() < 1e-9,
                "expected the em box to match 文字高さ 3, got {height} at {em_scale}"
            );
        }
    }

    #[test]
    fn text_box_keeps_square_cells_square() {
        // A full-width cell is as tall as it is wide in Jw_cad.
        // Correcting only the width would leave the glyphs stretched vertically.
        let text = text_span("日本語表示", 0.0, 20.0, 4.0);
        let (width, height) = rendered_em_box(&text, 5.0, INFLATING_EM_SCALE);
        assert!((width / 5.0 - height).abs() < 1e-9, "cell is not square");
    }

    #[test]
    fn text_box_writes_the_recorded_height_by_default() {
        // Only a caller naming an inflating renderer gets the pre-division.
        let text = text_span("日本語表示", 0.0, 20.0, 4.0);
        assert_eq!(
            text_box(&text, ConvertOptions::default().text_em_scale).0,
            4.0
        );
        assert!(text_box(&text, INFLATING_EM_SCALE).0 < 4.0);
    }

    #[test]
    fn text_box_width_factor_is_the_same_whatever_the_renderer() {
        // group 41 carries the JWW pitch; only group 40 may move.
        let text = text_span("日本語表示", 0.0, 23.0, 4.0);
        let spec = text_box(&text, 1.0).1;
        let inflated = text_box(&text, INFLATING_EM_SCALE).1;
        assert!((spec - inflated).abs() < 1e-12, "{spec} != {inflated}");
    }

    #[test]
    fn text_box_follows_the_stored_endpoint_not_the_pitch_model() {
        // Records naming a TrueType font are laid out with that font's own proportional metrics,
        // so the endpoint disagrees with size_x * cells. The endpoint is what must win.
        let narrow = text_span("▽柱面", 0.0, 10.69, 3.0);
        let pitched = text_span("▽柱面", 0.0, 9.0, 3.0);
        assert!(text_box(&narrow, 1.0).1 > text_box(&pitched, 1.0).1);
    }

    #[test]
    fn text_box_is_neutral_for_degenerate_records() {
        assert_eq!(text_box(&text_span("", 0.0, 10.0, 3.0), 1.0).1, 1.0);
        assert_eq!(text_box(&text_span("abc", 0.0, 0.0, 3.0), 1.0).1, 1.0);

        // A missing 文字高さ still has to produce a drawable height.
        let (height, factor) = text_box(&text_span("abc", 0.0, 10.0, 0.0), 1.0);
        assert_eq!(height, DEFAULT_TEXT_HEIGHT);
        assert!(factor > 0.0);

        // So does a renderer scale that would otherwise divide by zero.
        assert_eq!(text_box(&text_span("abc", 0.0, 10.0, 3.0), 0.0).0, 3.0);
    }

    /// The group pairs of every TEXT record in `rendered`.
    fn text_entity_records(rendered: &str) -> Vec<String> {
        rendered
            .split("  0\n")
            .filter(|record| record.starts_with("TEXT\n"))
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn document_to_string_writes_text_width_factor() {
        // 2 half-width cells at 文字高さ 3 spanning 4 units:
        // a factor of 4/3, which no STYLE table row can be mistaken for.
        let document = JwwDocument {
            header: empty_header(),
            entities: vec![Entity::Text(text_span("AB", 0.0, 4.0, 3.0))],
            block_defs: Vec::new(),
        };
        let dxf = convert_document(&document);
        let factor = match &dxf.entities[0] {
            DxfEntity::Text(v) => v.width_factor,
            other => panic!("expected TEXT, got {other:?}"),
        };
        assert!((factor - 4.0 / 3.0).abs() < 1e-12, "{factor}");

        let rendered = document_to_string(&dxf);
        let records = text_entity_records(&rendered);
        assert_eq!(records.len(), 1, "expected exactly one TEXT record");
        assert!(
            records[0].contains(&format!(" 41\n{factor:.12}\n")),
            "group 41 missing from the emitted TEXT: {}",
            records[0]
        );
    }

    /// A document whose only entity is an INSERT of a block holding `text`, scaled per axis.
    fn text_in_block(text: Text, scale_x: f64, scale_y: f64) -> JwwDocument {
        let base = EntityBase::default();
        JwwDocument {
            header: empty_header(),
            entities: vec![Entity::Block(Block {
                base,
                ref_x: 0.0,
                ref_y: 0.0,
                scale_x,
                scale_y,
                rotation: 0.0,
                def_number: 1,
            })],
            block_defs: vec![BlockDef {
                base,
                number: 1,
                is_referenced: true,
                name: "TEXT_BLOCK".to_string(),
                entities: vec![Entity::Text(text)],
            }],
        }
    }

    fn only_text(entities: &[DxfEntity]) -> &DxfText {
        let mut texts = entities.iter().filter_map(|entity| match entity {
            DxfEntity::Text(v) => Some(v),
            _ => None,
        });
        let text = texts.next().expect("expected a TEXT");
        assert!(texts.next().is_none(), "expected exactly one TEXT");
        text
    }

    /// The advance a renderer with the given `em_scale` draws for `text`.
    fn drawn_advance(text: &DxfText, em_scale: f64) -> f64 {
        em_scale * text.height * text.width_factor * text_cell_width(&text.content)
    }

    /// 3 full-width cells at 文字高さ 4 spanning 15 units: a width factor of 1.25.
    fn block_text() -> Text {
        text_span("日本語", 0.0, 15.0, 4.0)
    }

    #[test]
    fn block_definition_text_carries_the_width_correction() {
        let doc = text_in_block(block_text(), 1.0, 1.0);

        for em_scale in [1.0, INFLATING_EM_SCALE] {
            let dxf = convert_document_with_options(
                &doc,
                ConvertOptions {
                    text_em_scale: em_scale,
                    ..Default::default()
                },
            );
            let text = only_text(&dxf.blocks[0].entities);
            assert!(
                (text.height - 4.0 / em_scale).abs() < 1e-12,
                "group 40 inside the block ignores em_scale {em_scale}: {}",
                text.height
            );
            assert!(
                (drawn_advance(text, em_scale) - 15.0).abs() < 1e-9,
                "block text is drawn {} wide instead of 15 at {em_scale}",
                drawn_advance(text, em_scale)
            );

            let records = text_entity_records(&document_to_string(&dxf));
            assert_eq!(records.len(), 1);
            assert!(
                records[0].contains(&format!(" 41\n{:.12}\n", text.width_factor)),
                "group 41 missing from the block definition TEXT: {}",
                records[0]
            );
        }
    }

    #[test]
    fn exploding_a_uniform_insert_leaves_the_width_factor_alone() {
        let doc = text_in_block(block_text(), 2.0, 2.0);
        let dxf = convert_document_with_options(
            &doc,
            ConvertOptions {
                explode_inserts: true,
                ..Default::default()
            },
        );

        let text = only_text(&dxf.entities);
        assert!((text.height - 8.0).abs() < 1e-12, "{}", text.height);
        assert!(
            (text.width_factor - 1.25).abs() < 1e-12,
            "{}",
            text.width_factor
        );
        assert!((drawn_advance(text, 1.0) - 30.0).abs() < 1e-9);
    }

    #[test]
    fn exploding_a_non_uniform_insert_keeps_the_text_advance() {
        // scale_x 2 / scale_y 1: group 40 takes the average of the two axes (1.5x), so the glyphs come out too tall.
        // Copying group 41 across would make them `scale_x / average_scale` too wide on top of that.
        let doc = text_in_block(block_text(), 2.0, 1.0);

        for em_scale in [1.0, INFLATING_EM_SCALE] {
            let dxf = convert_document_with_options(
                &doc,
                ConvertOptions {
                    explode_inserts: true,
                    text_em_scale: em_scale,
                    ..Default::default()
                },
            );
            let text = only_text(&dxf.entities);
            assert!((text.end_x - text.x - 30.0).abs() < 1e-12);
            assert!(
                (drawn_advance(text, em_scale) - 30.0).abs() < 1e-9,
                "exploded text is drawn {} wide instead of 30 at {em_scale}",
                drawn_advance(text, em_scale)
            );
        }
    }

    #[test]
    fn exploding_a_tiny_insert_absorbs_the_height_floor_into_the_width() {
        // 文字高さ 4 at 1/100 lands under the 0.1 floor group 40 is clamped to.
        // The floor is silent, so group 41 has to take the difference or the string is drawn 25x too wide -- and pre-dividing group 40 by em_scale makes the floor bite sooner.
        let doc = text_in_block(block_text(), 0.01, 0.01);
        let dxf = convert_document_with_options(
            &doc,
            ConvertOptions {
                explode_inserts: true,
                ..Default::default()
            },
        );

        let text = only_text(&dxf.entities);
        assert_eq!(text.height, 0.1);
        assert!(
            (drawn_advance(text, 1.0) - 0.15).abs() < 1e-12,
            "clamped text is drawn {} wide instead of 0.15",
            drawn_advance(text, 1.0)
        );
    }

    #[test]
    fn exploding_a_degenerate_text_keeps_its_neutral_width_factor() {
        // No span to rescale from: the factor `text_box` settled on has to survive.
        let doc = text_in_block(text_span("日本語", 0.0, 0.0, 4.0), 2.0, 1.0);
        let dxf = convert_document_with_options(
            &doc,
            ConvertOptions {
                explode_inserts: true,
                ..Default::default()
            },
        );

        assert_eq!(only_text(&dxf.entities).width_factor, 1.0);
    }

    #[test]
    fn convert_document_excludes_internal_metadata_text() {
        let make_text = |content: &str, y: f64| {
            Entity::Text(Text {
                base: EntityBase::default(),
                start_x: 0.0,
                start_y: y,
                end_x: 0.0,
                end_y: y,
                text_type: 0,
                size_x: 3.0,
                size_y: 3.0,
                spacing: 0.0,
                angle: 0.0,
                font_name: String::new(),
                content: content.to_string(),
            })
        };
        let document = JwwDocument {
            header: empty_header(),
            entities: vec![
                make_text("Printer_PaperSize = 8", -1000.0),
                make_text("visible note", 10.0),
            ],
            block_defs: Vec::new(),
        };

        let converted = convert_document(&document);

        assert_eq!(converted.entities.len(), 1);
        match &converted.entities[0] {
            DxfEntity::Text(text) => assert_eq!(text.content, "visible note"),
            other => panic!("expected TEXT, got {other:?}"),
        }
        assert!(converted.unsupported_entities.is_empty());
    }

    #[test]
    fn convert_document_converts_negative_arc_sweep_to_short_ccw_span() {
        let base = EntityBase::default();
        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![Entity::Arc(Arc {
                base,
                center_x: 0.0,
                center_y: 0.0,
                radius: 10.0,
                start_angle: 1.0_f64.to_radians(),
                arc_angle: (-2.0_f64).to_radians(),
                tilt_angle: 0.0,
                flatness: 1.0,
                is_full_circle: false,
            })],
            block_defs: vec![],
        };

        let dxf = convert_document(&doc);
        match &dxf.entities[0] {
            DxfEntity::Arc(arc) => {
                assert!((arc.start_angle - 359.0).abs() < 1e-9);
                assert!((arc.end_angle - 1.0).abs() < 1e-9);
            }
            other => panic!("expected ARC, got {:?}", other),
        }
    }

    #[test]
    fn convert_document_converts_negative_ellipse_sweep_to_short_ccw_span() {
        let base = EntityBase::default();
        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![Entity::Arc(Arc {
                base,
                center_x: 0.0,
                center_y: 0.0,
                radius: 10.0,
                start_angle: 1.0,
                arc_angle: -0.2,
                tilt_angle: 0.0,
                flatness: 0.5,
                is_full_circle: false,
            })],
            block_defs: vec![],
        };

        let dxf = convert_document(&doc);
        match &dxf.entities[0] {
            DxfEntity::Ellipse(ellipse) => {
                assert!((ellipse.start_param - 0.8).abs() < 1e-9);
                assert!((ellipse.end_param - 1.0).abs() < 1e-9);
            }
            other => panic!("expected ELLIPSE, got {:?}", other),
        }
    }

    #[test]
    fn convert_document_preserves_pen_width_as_line_weight() {
        let base = EntityBase {
            pen_width: 20,
            ..EntityBase::default()
        };
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
        match &dxf.entities[0] {
            DxfEntity::Line(line) => assert_eq!(line.line_weight, 20),
            other => panic!("expected LINE, got {:?}", other),
        }

        let out = document_to_string(&dxf);
        assert!(out.contains("370\n20\n"));
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

    /// `DxfSolid` stores its corners in polygon traversal order, so walking `x1..x4` is the traversal.
    fn solid_traversal(solid: &DxfSolid) -> [DxfVertex; 4] {
        [
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
        ]
    }

    #[test]
    fn convert_document_orders_solid_by_jww_file_order() {
        // A well-formed convex solid whose file order point1 -> point4 -> point2 -> point3 walks the corners clockwise from the top-left.
        //
        // The clockwise winding is what gives this test teeth.
        // Feed the fields in　declaration order instead and the two diagonals become edges,
        // so　`order_solid_vertices` kicks in -- and its angular sort always rebuilds a convex quad counter-clockwise.
        // A counter-clockwise fixture would therefore come out identical either way and pin nothing.
        let base = EntityBase::default();
        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![Entity::Solid(Solid {
                base,
                point1_x: 0.0,
                point1_y: 10.0,
                point2_x: 10.0,
                point2_y: 0.0,
                point3_x: 0.0,
                point3_y: 0.0,
                point4_x: 12.0,
                point4_y: 10.0,
                color: None,
            })],
            block_defs: vec![],
        };

        let dxf = convert_document(&doc);
        assert_eq!(dxf.entities.len(), 1);
        match &dxf.entities[0] {
            DxfEntity::Solid(solid) => {
                let traversal = solid_traversal(solid);
                assert!(!solid_vertices_cross(&traversal));
                assert_eq!(
                    traversal,
                    [
                        DxfVertex { x: 0.0, y: 10.0 },
                        DxfVertex { x: 12.0, y: 10.0 },
                        DxfVertex { x: 10.0, y: 0.0 },
                        DxfVertex { x: 0.0, y: 0.0 },
                    ]
                );
            }
            other => panic!("expected SOLID, got {:?}", other),
        }
    }

    #[test]
    fn convert_document_keeps_concave_solid_traversal() {
        // A concave solid, traversed (0,0) -> (4,0) -> (1,1) -> (0,4) with the third corner denting inwards.
        // Nothing self-crosses in either ordering,
        // so `order_solid_vertices` stays out of the way and the field order is the only thing that decides the shape.
        let base = EntityBase::default();
        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![Entity::Solid(Solid {
                base,
                point1_x: 0.0,
                point1_y: 0.0,
                point2_x: 1.0,
                point2_y: 1.0,
                point3_x: 0.0,
                point3_y: 4.0,
                point4_x: 4.0,
                point4_y: 0.0,
                color: None,
            })],
            block_defs: vec![],
        };

        let dxf = convert_document(&doc);
        assert_eq!(dxf.entities.len(), 1);
        match &dxf.entities[0] {
            DxfEntity::Solid(solid) => {
                assert_eq!(
                    solid_traversal(solid),
                    [
                        DxfVertex { x: 0.0, y: 0.0 },
                        DxfVertex { x: 4.0, y: 0.0 },
                        DxfVertex { x: 1.0, y: 1.0 },
                        DxfVertex { x: 0.0, y: 4.0 },
                    ]
                );
            }
            other => panic!("expected SOLID, got {:?}", other),
        }
    }

    #[test]
    fn convert_document_orders_crossed_solid_vertices() {
        // Corners laid out so that even the file order self-crosses,
        // forcing the `order_solid_vertices` fallback to repair the traversal.
        let base = EntityBase::default();
        let doc = JwwDocument {
            header: empty_header(),
            entities: vec![Entity::Solid(Solid {
                base,
                point1_x: 0.0,
                point1_y: 10.0,
                point2_x: 0.0,
                point2_y: 0.0,
                point3_x: 10.0,
                point3_y: 10.0,
                point4_x: 10.0,
                point4_y: 0.0,
                color: None,
            })],
            block_defs: vec![],
        };

        let dxf = convert_document(&doc);
        assert_eq!(dxf.entities.len(), 1);
        match &dxf.entities[0] {
            DxfEntity::Solid(solid) => {
                let traversal = solid_traversal(solid);
                assert!(!solid_vertices_cross(&traversal));
                assert_eq!(
                    traversal,
                    [
                        DxfVertex { x: 0.0, y: 10.0 },
                        DxfVertex { x: 0.0, y: 0.0 },
                        DxfVertex { x: 10.0, y: 0.0 },
                        DxfVertex { x: 10.0, y: 10.0 },
                    ]
                );
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
                ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
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
        assert!(out.contains("  9\n$HANDSEED\n  5\nFFFFFFFF\n"));
        assert!(out.ends_with("  0\nEOF\n"));
    }

    #[test]
    fn document_to_string_can_emit_ac1024_header() {
        let dxf = DxfDocument {
            layers: vec![],
            entities: vec![],
            blocks: vec![],
            unsupported_entities: vec![],
        };

        let out = document_to_string_with_version(&dxf, DxfTargetVersion::Ac1024);

        assert!(out.contains("  9\n$ACADVER\n  1\nAC1024\n"));
    }

    #[test]
    fn convert_document_sanitizes_and_deduplicates_dxf_table_names() {
        let mut header = empty_header();
        header.layer_groups[0].layers[0].name = "A*B".to_string();
        header.layer_groups[0].layers[1].name = "A?B".to_string();
        let first_base = EntityBase::default();
        let second_base = EntityBase {
            layer: 1,
            ..EntityBase::default()
        };
        let doc = JwwDocument {
            header,
            entities: vec![
                Entity::Line(Line {
                    base: first_base,
                    start_x: 0.0,
                    start_y: 0.0,
                    end_x: 1.0,
                    end_y: 0.0,
                }),
                Entity::Line(Line {
                    base: second_base,
                    start_x: 0.0,
                    start_y: 1.0,
                    end_x: 1.0,
                    end_y: 1.0,
                }),
            ],
            block_defs: vec![
                BlockDef {
                    base: first_base,
                    number: 1,
                    is_referenced: false,
                    name: "B*X".to_string(),
                    entities: vec![],
                },
                BlockDef {
                    base: first_base,
                    number: 2,
                    is_referenced: false,
                    name: "B?X".to_string(),
                    entities: vec![],
                },
            ],
        };

        let dxf = convert_document(&doc);
        let entity_layers = dxf
            .entities
            .iter()
            .map(|entity| match entity {
                DxfEntity::Line(line) => line.layer.as_str(),
                _ => panic!("expected line"),
            })
            .collect::<Vec<_>>();
        let block_names = dxf
            .blocks
            .iter()
            .map(|block| block.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(entity_layers, ["A_B", "A_B_0-1"]);
        assert_eq!(block_names, ["B_X", "B_X_2"]);
        assert!(dxf.layers.iter().all(|layer| !layer
            .name
            .chars()
            .any(|character| "<>/\\\":;?*|=".contains(character))));
    }

    /// Pulls the corner group codes (10..13 / 20..23) of the first SOLID out of an ASCII DXF,
    /// in the order they were emitted.
    fn solid_corner_groups(out: &str) -> Vec<(i32, f64)> {
        let lines: Vec<&str> = out.lines().collect();
        let start = lines
            .iter()
            .position(|line| line.trim() == "SOLID")
            .expect("no SOLID entity in output");
        // Every group is exactly two lines (code, value),
        // so chunking keeps alignment even across the string-valued groups we skip below.
        lines[start + 1..]
            .chunks(2)
            .filter_map(|pair| match pair {
                [code, value] => Some((code.trim().parse::<i32>().ok()?, value.trim())),
                _ => None,
            })
            .take_while(|(code, _)| *code != 0)
            .filter(|(code, _)| matches!(code, 10..=13 | 20..=23))
            .filter_map(|(code, value)| Some((code, value.parse::<f64>().ok()?)))
            .collect()
    }

    #[test]
    fn document_to_string_writes_solid_corners_in_dxf_z_order() {
        // `DxfSolid` holds the corners in traversal order, so the writer -- and only
        // the writer -- has to emit the 4th corner as group 12 and the 3rd as 13.
        let dxf = DxfDocument {
            layers: vec![],
            entities: vec![DxfEntity::Solid(DxfSolid {
                layer: "0".to_string(),
                color: 7,
                line_type: "CONTINUOUS".to_string(),
                line_weight: -3,
                x1: 1.0,
                y1: 1.0,
                x2: 2.0,
                y2: 1.0,
                x3: 2.0,
                y3: 2.0,
                x4: 1.0,
                y4: 2.0,
            })],
            blocks: vec![],
            unsupported_entities: vec![],
        };

        let out = document_to_string(&dxf);

        assert_eq!(
            solid_corner_groups(&out),
            vec![
                (10, 1.0),
                (20, 1.0),
                (11, 2.0),
                (21, 1.0),
                // group 12 is vertex 4, group 13 is vertex 3
                (12, 1.0),
                (22, 2.0),
                (13, 2.0),
                (23, 2.0),
            ]
        );
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
                end_x: 0.0,
                end_y: 0.0,
                height: 2.5,
                width_factor: 1.0,
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
