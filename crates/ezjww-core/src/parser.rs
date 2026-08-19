use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::diagnostics::DecodeDiagnostic;
use crate::error::JwwError;
use crate::header::parse_header_with_diagnostics;
use crate::model::{
    Arc, Block, BlockDef, CircleSolid, Dimension, Entity, EntityBase, JwwDocument, Line, Point,
    Solid, Text,
};
use crate::reader::Reader;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedJwwDocument {
    pub document: JwwDocument,
    pub diagnostics: Vec<DecodeDiagnostic>,
}

pub fn parse_document(data: &[u8]) -> Result<JwwDocument, JwwError> {
    parse_document_with_diagnostics(data).map(|parsed| parsed.document)
}

pub fn parse_document_with_diagnostics(data: &[u8]) -> Result<ParsedJwwDocument, JwwError> {
    let (header, mut diagnostics) = parse_header_with_diagnostics(data)?;
    let entity_list_offset =
        find_entity_list_offset(data, header.version).ok_or(JwwError::EntityListNotFound)?;
    let mut reader = Reader::with_base_offset(&data[entity_list_offset..], entity_list_offset);
    let mut table = ArchiveTable::new();
    let outcome = parse_entity_list_lenient(&mut reader, header.version, &mut table);
    let stop_offset = entity_list_offset + reader.bytes_read();
    let entity_diagnostics = reader.into_decode_diagnostics();
    diagnostics.extend(entity_diagnostics);

    let entities = outcome.entities;
    let block_defs = match outcome.truncation {
        None => {
            let (block_defs, block_diagnostics) = if stop_offset < data.len() {
                parse_block_def_list(
                    &data[stop_offset..],
                    header.version,
                    stop_offset,
                    &mut table,
                )
            } else {
                (Vec::new(), Vec::new())
            };
            diagnostics.extend(block_diagnostics);
            block_defs
        }
        Some(truncation) => {
            // Best effort: keep what was read when at least one entity survived,
            // and report the truncation instead of failing the whole file. Block
            // definitions cannot be located behind a truncated list.
            if entities.is_empty() {
                return Err(truncation.error);
            }
            diagnostics.push(DecodeDiagnostic::entity_list_truncated(
                stop_offset,
                truncation.expected_entities,
                entities.len(),
                truncation.error.to_string(),
            ));
            Vec::new()
        }
    };

    Ok(ParsedJwwDocument {
        document: JwwDocument {
            header,
            entities,
            block_defs,
        },
        diagnostics,
    })
}

pub fn read_document_from_file(path: impl AsRef<Path>) -> Result<JwwDocument, JwwError> {
    let data = fs::read(path)?;
    parse_document(&data)
}

pub fn read_document_from_file_with_diagnostics(
    path: impl AsRef<Path>,
) -> Result<ParsedJwwDocument, JwwError> {
    let data = fs::read(path)?;
    parse_document_with_diagnostics(&data)
}

fn find_entity_list_offset(data: &[u8], version: u32) -> Option<usize> {
    let expected_schema = version as u16;
    let mut fallback_offset = None;

    if data.len() < 128 {
        return None;
    }

    for i in 100..data.len().saturating_sub(20) {
        if data[i] != 0xFF || data[i + 1] != 0xFF {
            continue;
        }

        let schema = u16::from_le_bytes([data[i + 2], data[i + 3]]);
        let name_len = u16::from_le_bytes([data[i + 4], data[i + 5]]) as usize;
        if !(8..=32).contains(&name_len) || i + 6 + name_len > data.len() {
            continue;
        }

        let class_name = &data[i + 6..i + 6 + name_len];
        if !class_name.starts_with(b"CData") || i < 2 {
            continue;
        }

        let offset = entity_list_offset_before_class_tag(data, i);
        if schema == expected_schema {
            return Some(offset);
        }
        if fallback_offset.is_none() {
            fallback_offset = Some(offset);
        }
    }

    fallback_offset
}

/// Offset of the entity list count preceding the first class tag at `class_tag`.
///
/// `CObList::Serialize` writes the count with `CArchive::WriteCount`: a WORD, or the
/// escape WORD `0xFFFF` followed by a DWORD when the list holds 65535+ entities. In
/// the escaped layout the two bytes right before the class tag are the high word of
/// the DWORD count, so the plain "count is the WORD before the tag" rule would read a
/// tiny count and silently drop almost the whole drawing.
fn entity_list_offset_before_class_tag(data: &[u8], class_tag: usize) -> usize {
    if class_tag >= 6 && data[class_tag - 6] == 0xFF && data[class_tag - 5] == 0xFF {
        let dword = u32::from_le_bytes([
            data[class_tag - 4],
            data[class_tag - 3],
            data[class_tag - 2],
            data[class_tag - 1],
        ]) as usize;
        // Every entity is at least ~20 bytes; reject implausible escaped counts so a
        // header field that happens to end in FF FF cannot hijack a small list.
        if dword >= 0xFFFF && dword.saturating_mul(20) <= data.len() {
            return class_tag - 6;
        }
    }
    class_tag - 2
}

/// `CArchive::ReadCount`: a WORD count, escaped to a DWORD by `0xFFFF`.
fn read_count(reader: &mut Reader<'_>) -> Result<usize, JwwError> {
    let word = reader.read_u16()?;
    if word != 0xFFFF {
        return Ok(word as usize);
    }
    Ok(reader.read_u32()? as usize)
}

/// Serialized object tag as written by `CArchive::WriteObject`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectTag {
    /// `0xFFFF`: a `CRuntimeClass` description follows, then the object.
    NewClass,
    /// `0x8000 | pid` (or the big-tag form): an object of an already-loaded class.
    ClassRef(u32),
    /// `0` (`wNullTag`), also `0x8000` as written by some third-party writers.
    Null,
    /// A reference to an already-loaded object (no class bit).
    ObjectRef(u32),
}

fn read_object_tag(reader: &mut Reader<'_>) -> Result<ObjectTag, JwwError> {
    let word = reader.read_u16()?;
    if word == 0xFFFF {
        return Ok(ObjectTag::NewClass);
    }
    // wBigObjectTag (0x7FFF): the real tag follows as a DWORD (class bit 0x8000_0000).
    let tag: u32 = if word == 0x7FFF {
        reader.read_u32()?
    } else {
        ((u32::from(word) & 0x8000) << 16) | (u32::from(word) & 0x7FFF)
    };
    if tag & 0x8000_0000 != 0 {
        let pid = tag & 0x7FFF_FFFF;
        if pid == 0 {
            return Ok(ObjectTag::Null);
        }
        return Ok(ObjectTag::ClassRef(pid));
    }
    if tag == 0 {
        return Ok(ObjectTag::Null);
    }
    Ok(ObjectTag::ObjectRef(tag))
}

/// Result of reading an entity list as far as the data allows.
struct EntityListOutcome {
    entities: Vec<Entity>,
    truncation: Option<EntityListTruncation>,
}

struct EntityListTruncation {
    expected_entities: usize,
    error: JwwError,
}

/// A single, archive-wide table of class-schema and object back-references.
///
/// The on-disk format is one continuous MFC `CArchive` stream: the main
/// entity list, the block-definition list, and every block's own nested
/// entity list all share the *same* running index of "classes and objects
/// seen so far". A class or object registered while reading the main list
/// can legitimately be referenced again from inside a block, and a class
/// registered inside one block can be referenced again from a later block.
/// This table must therefore be threaded through the whole document parse
/// instead of being recreated per list.
struct ArchiveTable {
    class_names: HashMap<u32, String>,
    objects: HashMap<u32, Entity>,
    next_pid: u32,
}

impl ArchiveTable {
    fn new() -> Self {
        Self {
            class_names: HashMap::new(),
            objects: HashMap::new(),
            next_pid: 1,
        }
    }

    /// Registers a newly-seen class name and returns the pid assigned to it.
    fn register_class(&mut self, name: String) -> u32 {
        let pid = self.next_pid;
        self.class_names.insert(pid, name);
        self.next_pid += 1;
        pid
    }

    fn resolve_class(&self, pid: u32) -> Option<&str> {
        self.class_names.get(&pid).map(String::as_str)
    }

    /// Reserves the pid slot an object occupies once fully read, regardless
    /// of whether it was reached via a new class or a class reference.
    fn reserve_object_pid(&mut self) -> u32 {
        let pid = self.next_pid;
        self.next_pid += 1;
        pid
    }

    fn store_object(&mut self, pid: u32, entity: Entity) {
        self.objects.insert(pid, entity);
    }

    fn resolve_object(&self, pid: u32) -> Option<&Entity> {
        self.objects.get(&pid)
    }
}

fn parse_entity_list_lenient(
    reader: &mut Reader<'_>,
    version: u32,
    table: &mut ArchiveTable,
) -> EntityListOutcome {
    let count = match read_count(reader) {
        Ok(count) => count,
        Err(error) => {
            return EntityListOutcome {
                entities: Vec::new(),
                truncation: Some(EntityListTruncation {
                    expected_entities: 0,
                    error,
                }),
            }
        }
    };
    // A corrupt count must not drive a huge allocation up front.
    let mut entities = Vec::with_capacity(count.min(1 << 16));

    for _ in 0..count {
        match parse_entity_with_pid_tracking(reader, version, table) {
            Ok(Some(entity)) => entities.push(entity),
            Ok(None) => {}
            Err(error) => {
                return EntityListOutcome {
                    entities,
                    truncation: Some(EntityListTruncation {
                        expected_entities: count,
                        error,
                    }),
                }
            }
        }
    }

    EntityListOutcome {
        entities,
        truncation: None,
    }
}

fn parse_entity_with_pid_tracking(
    reader: &mut Reader<'_>,
    version: u32,
    table: &mut ArchiveTable,
) -> Result<Option<Entity>, JwwError> {
    let class_name = match read_object_tag(reader)? {
        ObjectTag::NewClass => {
            let _schema_version = reader.read_u16()?;
            let name_len = reader.read_u16()? as usize;
            let name = String::from_utf8_lossy(&reader.read_bytes(name_len)?).to_string();
            table.register_class(name.clone());
            name
        }
        ObjectTag::Null => return Ok(None),
        ObjectTag::ClassRef(class_pid) => table
            .resolve_class(class_pid)
            .map(str::to_string)
            .ok_or(JwwError::UnknownClassPid(class_pid))?,
        ObjectTag::ObjectRef(tag) => {
            return table
                .resolve_object(tag)
                .cloned()
                .map(Some)
                .ok_or(JwwError::UnsupportedObjectReference(tag));
        }
    };

    let entity = match class_name.as_str() {
        "CDataSen" => Some(Entity::Line(parse_line(reader, version)?)),
        "CDataEnko" => Some(Entity::Arc(parse_arc(reader, version)?)),
        "CDataTen" => Some(Entity::Point(parse_point(reader, version)?)),
        "CDataMoji" => Some(Entity::Text(parse_text(reader, version)?)),
        "CDataSolid" => Some(parse_solid(reader, version)?),
        "CDataBlock" => Some(Entity::Block(parse_block(reader, version)?)),
        "CDataSunpou" => Some(Entity::Dimension(parse_dimension(reader, version)?)),
        _ => return Err(JwwError::UnknownEntityClass(class_name)),
    };

    let pid = table.reserve_object_pid();
    if let Some(entity) = &entity {
        table.store_object(pid, entity.clone());
    }
    Ok(entity)
}

fn parse_entity_base(reader: &mut Reader<'_>, version: u32) -> Result<EntityBase, JwwError> {
    let group = reader.read_u32()?;
    let pen_style = reader.read_u8()?;
    let pen_color = reader.read_u16()?;
    let pen_width = if version >= 351 {
        reader.read_u16()?
    } else {
        0
    };
    let layer = reader.read_u16()?;
    let layer_group = reader.read_u16()?;
    let flag = reader.read_u16()?;

    Ok(EntityBase {
        group,
        pen_style,
        pen_color,
        pen_width,
        layer,
        layer_group,
        flag,
    })
}

fn parse_line(reader: &mut Reader<'_>, version: u32) -> Result<Line, JwwError> {
    let base = parse_entity_base(reader, version)?;
    Ok(Line {
        base,
        start_x: reader.read_f64()?,
        start_y: reader.read_f64()?,
        end_x: reader.read_f64()?,
        end_y: reader.read_f64()?,
    })
}

fn parse_arc(reader: &mut Reader<'_>, version: u32) -> Result<Arc, JwwError> {
    let base = parse_entity_base(reader, version)?;
    Ok(Arc {
        base,
        center_x: reader.read_f64()?,
        center_y: reader.read_f64()?,
        radius: reader.read_f64()?,
        start_angle: reader.read_f64()?,
        arc_angle: reader.read_f64()?,
        tilt_angle: reader.read_f64()?,
        flatness: reader.read_f64()?,
        is_full_circle: reader.read_u32()? != 0,
    })
}

fn parse_point(reader: &mut Reader<'_>, version: u32) -> Result<Point, JwwError> {
    let base = parse_entity_base(reader, version)?;
    let x = reader.read_f64()?;
    let y = reader.read_f64()?;
    let is_temporary = reader.read_u32()? != 0;

    let (code, angle, scale) = if base.pen_style == 100 {
        (reader.read_u32()?, reader.read_f64()?, reader.read_f64()?)
    } else {
        (0, 0.0, 0.0)
    };

    Ok(Point {
        base,
        x,
        y,
        is_temporary,
        code,
        angle,
        scale,
    })
}

fn parse_text(reader: &mut Reader<'_>, version: u32) -> Result<Text, JwwError> {
    let base = parse_entity_base(reader, version)?;
    Ok(Text {
        base,
        start_x: reader.read_f64()?,
        start_y: reader.read_f64()?,
        end_x: reader.read_f64()?,
        end_y: reader.read_f64()?,
        text_type: reader.read_u32()?,
        size_x: reader.read_f64()?,
        size_y: reader.read_f64()?,
        spacing: reader.read_f64()?,
        angle: reader.read_f64()?,
        font_name: reader.read_cstring_with_context("entity.text.font_name")?,
        content: reader.read_cstring_with_context("entity.text.content")?,
    })
}

fn parse_solid(reader: &mut Reader<'_>, version: u32) -> Result<Entity, JwwError> {
    let base = parse_entity_base(reader, version)?;
    let start_x = reader.read_f64()?;
    let start_y = reader.read_f64()?;
    let end_x = reader.read_f64()?;
    let end_y = reader.read_f64()?;
    let dpoint2_x = reader.read_f64()?;
    let dpoint2_y = reader.read_f64()?;
    let dpoint3_x = reader.read_f64()?;
    let dpoint3_y = reader.read_f64()?;
    let color = if base.pen_color == 10 {
        Some(reader.read_u32()?)
    } else {
        None
    };

    if base.pen_style >= 101 {
        return Ok(Entity::CircleSolid(CircleSolid {
            base,
            center_x: start_x,
            center_y: start_y,
            radius: end_x,
            flatness: end_y,
            tilt_angle: dpoint2_x,
            start_angle: dpoint2_y,
            arc_angle: dpoint3_x,
            solid_mode: dpoint3_y,
            color,
        }));
    }

    Ok(Entity::Solid(Solid {
        base,
        point1_x: start_x,
        point1_y: start_y,
        point2_x: dpoint2_x,
        point2_y: dpoint2_y,
        point3_x: dpoint3_x,
        point3_y: dpoint3_y,
        point4_x: end_x,
        point4_y: end_y,
        color,
    }))
}

fn parse_block(reader: &mut Reader<'_>, version: u32) -> Result<Block, JwwError> {
    let base = parse_entity_base(reader, version)?;
    Ok(Block {
        base,
        ref_x: reader.read_f64()?,
        ref_y: reader.read_f64()?,
        scale_x: reader.read_f64()?,
        scale_y: reader.read_f64()?,
        rotation: reader.read_f64()?,
        def_number: reader.read_u32()?,
    })
}

fn parse_dimension(reader: &mut Reader<'_>, version: u32) -> Result<Dimension, JwwError> {
    let base = parse_entity_base(reader, version)?;
    let line = parse_line(reader, version)?;
    let text = parse_text(reader, version)?;

    let mut sxf_mode = None;
    let mut aux_lines = Vec::new();
    let mut aux_points = Vec::new();
    if version >= 420 {
        sxf_mode = Some(reader.read_u16()?);
        for _ in 0..2 {
            aux_lines.push(parse_line(reader, version)?);
        }
        for _ in 0..4 {
            aux_points.push(parse_point(reader, version)?);
        }
    }

    Ok(Dimension {
        base,
        line,
        text,
        sxf_mode,
        aux_lines,
        aux_points,
    })
}

fn parse_block_def_list(
    data: &[u8],
    version: u32,
    base_offset: usize,
    table: &mut ArchiveTable,
) -> (Vec<BlockDef>, Vec<DecodeDiagnostic>) {
    let mut reader = Reader::with_base_offset(data, base_offset);
    // The block definition list is a CObList too, so its count follows
    // CArchive::ReadCount (WORD, 0xFFFF escape). Older writers of this crate's
    // fixtures used a plain DWORD; accept that layout when the two bytes after a
    // WORD count are zero and a class tag follows.
    let count = match read_count(&mut reader) {
        Ok(v) => v,
        Err(_) => return (Vec::new(), reader.into_decode_diagnostics()),
    };
    let plain_word_count = data.len() >= 6 && !(data[0] == 0xFF && data[1] == 0xFF);
    if count > 0
        && plain_word_count
        && data[2] == 0
        && data[3] == 0
        && data[4] == 0xFF
        && data[5] == 0xFF
    {
        // Legacy DWORD count layout: consume its high word.
        if reader.skip(2).is_err() {
            return (Vec::new(), reader.into_decode_diagnostics());
        }
    }

    if count > 10_000 {
        return (Vec::new(), reader.into_decode_diagnostics());
    }

    let mut block_defs = Vec::<BlockDef>::with_capacity(count);

    for _ in 0..count {
        match parse_block_def_with_tracking(&mut reader, version, table) {
            Ok(Some(block_def)) => block_defs.push(block_def),
            Ok(None) => {}
            Err(_) => break,
        }
    }

    (block_defs, reader.into_decode_diagnostics())
}

fn parse_block_def_with_tracking(
    reader: &mut Reader<'_>,
    version: u32,
    table: &mut ArchiveTable,
) -> Result<Option<BlockDef>, JwwError> {
    match read_object_tag(reader)? {
        ObjectTag::NewClass => {
            let _schema = reader.read_u16()?;
            let name_len = reader.read_u16()? as usize;
            let class_name = String::from_utf8_lossy(&reader.read_bytes(name_len)?).to_string();
            table.register_class(class_name);
        }
        ObjectTag::Null => return Ok(None),
        // The wrapper class name (e.g. "CDataList") is never actually needed
        // to know which fields follow, so a reference to it is accepted
        // without a strict lookup, same as before this table was unified.
        ObjectTag::ClassRef(_) => {}
        ObjectTag::ObjectRef(tag) => return Err(JwwError::UnsupportedObjectReference(tag)),
    }

    // The block-definition object's own identity is established here, the
    // moment its class is known -- *before* its nested entity list (which
    // can itself register further classes/objects) is read. This mirrors
    // real CArchive::ReadObject, which reserves the new object's slot
    // before calling into its Serialize body.
    table.reserve_object_pid();

    let base = parse_entity_base(reader, version)?;
    let number = reader.read_u32()?;
    let is_referenced = reader.read_u32()? != 0;
    reader.skip(4)?; // CTime
    let name = reader.read_cstring_with_context("block_definition.name")?;

    // Keep whatever part of a damaged nested list could be read; the outer block
    // definition loop stops at the next tag mismatch anyway.
    let entities = parse_entity_list_lenient(reader, version, table).entities;

    Ok(Some(BlockDef {
        base,
        number,
        is_referenced,
        name,
        entities,
    }))
}

pub fn entity_counts(entities: &[Entity]) -> HashMap<&'static str, usize> {
    let mut counts = HashMap::<&'static str, usize>::new();
    for entity in entities {
        *counts.entry(entity.entity_type()).or_insert(0) += 1;
    }
    counts
}

pub fn block_def_name_map(block_defs: &[BlockDef]) -> HashMap<u32, String> {
    let mut map = HashMap::<u32, String>::with_capacity(block_defs.len());
    for block_def in block_defs {
        map.insert(block_def.number, block_def.name.clone());
    }
    map
}

pub fn resolve_block_name(def_number: u32, block_defs: &[BlockDef]) -> Option<&str> {
    block_defs
        .iter()
        .find(|def| def.number == def_number)
        .map(|def| def.name.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlockReferenceValidation {
    pub total_references: usize,
    pub resolved_references: usize,
    pub unresolved_def_numbers: Vec<u32>,
}

impl BlockReferenceValidation {
    pub fn has_unresolved(&self) -> bool {
        !self.unresolved_def_numbers.is_empty()
    }
}

pub fn validate_block_references(document: &JwwDocument) -> BlockReferenceValidation {
    let mut ref_numbers = Vec::<u32>::new();
    collect_block_ref_numbers(&document.entities, &mut ref_numbers);
    for block_def in &document.block_defs {
        collect_block_ref_numbers(&block_def.entities, &mut ref_numbers);
    }

    let total_references = ref_numbers.len();
    let name_map = block_def_name_map(&document.block_defs);

    let mut resolved_references = 0usize;
    let mut unresolved = BTreeSet::<u32>::new();
    for def_number in ref_numbers {
        if name_map.contains_key(&def_number) {
            resolved_references += 1;
        } else {
            unresolved.insert(def_number);
        }
    }

    BlockReferenceValidation {
        total_references,
        resolved_references,
        unresolved_def_numbers: unresolved.into_iter().collect(),
    }
}

fn collect_block_ref_numbers(entities: &[Entity], out: &mut Vec<u32>) {
    for entity in entities {
        if let Entity::Block(block) = entity {
            out.push(block.def_number);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use crate::model::{BlockDef, Entity, EntityBase};

    use super::{
        block_def_name_map, entity_counts, parse_document_with_diagnostics,
        read_document_from_file, resolve_block_name, validate_block_references, JwwError,
    };

    fn jww_samples_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../jww_samples")
    }

    fn block_regression_samples_dir() -> PathBuf {
        jww_samples_dir().join("block_regressions")
    }

    #[test]
    fn parse_all_jww_samples() {
        let dir = jww_samples_dir();
        let mut files = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().map(|ext| ext == "jww").unwrap_or(false))
            .collect::<Vec<_>>();
        files.sort();

        for path in files {
            let doc = read_document_from_file(&path)
                .unwrap_or_else(|e| panic!("failed parsing {}: {e}", path.display()));
            assert_eq!(doc.header.version, 600);
            assert!(
                !doc.entities.is_empty(),
                "no entities in {}",
                path.display()
            );
        }
    }

    #[test]
    fn real_data_scan_nested_dimensions_in_block_defs() {
        let dir = jww_samples_dir();
        let mut files = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().map(|ext| ext == "jww").unwrap_or(false))
            .collect::<Vec<_>>();
        files.sort();

        let mut files_scanned = 0usize;
        let mut files_with_block_defs = 0usize;
        let mut files_with_nested_dimension = 0usize;

        for path in files {
            let doc = read_document_from_file(&path)
                .unwrap_or_else(|e| panic!("failed parsing {}: {e}", path.display()));
            files_scanned += 1;

            if !doc.block_defs.is_empty() {
                files_with_block_defs += 1;
            }

            let mut nested_dim_count = 0usize;
            for block_def in &doc.block_defs {
                for entity in &block_def.entities {
                    if let Entity::Dimension(dim) = entity {
                        nested_dim_count += 1;
                        assert!(
                            dim.text.size_x >= 0.0 && dim.text.size_y >= 0.0,
                            "invalid dimension text size in {}",
                            path.display()
                        );
                    }
                }
            }
            if nested_dim_count > 0 {
                files_with_nested_dimension += 1;
            }
        }

        assert!(files_scanned > 0, "no files scanned");
        eprintln!(
            "real_data_scan: files={}, with_block_defs={}, with_nested_dimension={}",
            files_scanned, files_with_block_defs, files_with_nested_dimension
        );
    }

    #[test]
    fn parse_shikichizu_expected_counts() {
        let path = jww_samples_dir().join("敷地図.jww");
        let doc = read_document_from_file(&path).unwrap();
        let counts = entity_counts(&doc.entities);

        assert_eq!(counts.get("LINE").copied().unwrap_or(0), 9);
        assert_eq!(counts.get("ARC").copied().unwrap_or(0), 0);
        assert_eq!(counts.get("POINT").copied().unwrap_or(0), 0);
        assert_eq!(counts.get("TEXT").copied().unwrap_or(0), 0);
    }

    #[test]
    fn invalid_signature_returns_error() {
        let err = super::parse_document(b"NotJwwData").unwrap_err();
        assert!(matches!(
            err,
            JwwError::InvalidSignature | JwwError::InvalidSignatureFound(_)
        ));
    }

    #[test]
    fn parse_minimal_with_block_def() {
        let data = build_minimal_jww_with_block_def();
        let doc = super::parse_document(&data).unwrap();
        assert_eq!(doc.entities.len(), 1);
        assert_eq!(doc.block_defs.len(), 1);

        match &doc.entities[0] {
            Entity::Block(block) => assert_eq!(block.def_number, 1),
            other => panic!("expected BLOCK entity, got {:?}", other),
        }

        let def = &doc.block_defs[0];
        assert_eq!(def.number, 1);
        assert!(!def.is_referenced);
        assert_eq!(def.name, "BLK");
        assert_eq!(resolve_block_name(1, &doc.block_defs), Some("BLK"));

        let validation = validate_block_references(&doc);
        assert_eq!(validation.total_references, 1);
        assert_eq!(validation.resolved_references, 1);
        assert!(validation.unresolved_def_numbers.is_empty());
        assert!(!validation.has_unresolved());
    }

    // Regression coverage for the archive-wide class/object table.
    //
    // Before that table was shared across the main entity list, the
    // block-definition list, and each block's own nested entity list, a
    // class registered in one of those scopes could not be resolved when
    // referenced again from another scope. In real Jw_cad files this
    // showed up as blocks (furniture, door/window symbols, ...) silently
    // losing all but their first entity, with no error surfaced to the
    // caller. These three fixtures were produced directly in Jw_cad (not
    // hand-built) to isolate each scope-crossing pattern individually.

    #[test]
    fn parse_block_def_reuses_class_registered_in_main_entity_list() {
        // out1_in2.jww: one LINE drawn outside any block, then a block
        // containing two more LINEs. The block's first LINE must resolve
        // "CDataSen" via a class reference into the main list's table
        // instead of failing as an unknown class pid.
        let path = block_regression_samples_dir().join("out1_in2.jww");
        let doc = read_document_from_file(&path).unwrap();

        assert_eq!(doc.block_defs.len(), 1);
        assert_eq!(doc.block_defs[0].entities.len(), 2);
        for entity in &doc.block_defs[0].entities {
            assert!(matches!(entity, Entity::Line(_)));
        }
    }

    #[test]
    fn parse_block_def_resolves_repeated_entity_classes_within_one_block() {
        // sqr-circle-blocks.jww: a single block containing a 4-line square
        // plus a circle drawn separately at the top level. Every one of the
        // square's later edges references the same "CDataSen" class as its
        // first edge; all five entities inside the block must be recovered.
        let path = block_regression_samples_dir().join("sqr-circle-blocks.jww");
        let doc = read_document_from_file(&path).unwrap();

        assert_eq!(doc.block_defs.len(), 1);
        assert_eq!(doc.block_defs[0].entities.len(), 5);

        let line_count = doc.block_defs[0]
            .entities
            .iter()
            .filter(|e| matches!(e, Entity::Line(_)))
            .count();
        assert_eq!(line_count, 4);
    }

    #[test]
    fn parse_multiple_block_defs_each_with_multiple_entities() {
        // 2blocks.jww: two separate blocks, one with two LINEs and one with
        // two CIRCLEs. Before the fix, a failure while reading the first
        // block's second entity aborted the whole block-definition list, so
        // the second block was never even attempted.
        let path = block_regression_samples_dir().join("2blocks.jww");
        let doc = read_document_from_file(&path).unwrap();

        assert_eq!(doc.block_defs.len(), 2);
        assert_eq!(doc.block_defs[0].entities.len(), 2);
        assert_eq!(doc.block_defs[1].entities.len(), 2);
    }

    #[test]
    fn parse_minimal_when_class_schema_differs_from_header_version() {
        let data = build_minimal_jww_420_with_schema_1_line();
        let doc = super::parse_document(&data).unwrap();

        assert_eq!(doc.header.version, 420);
        assert_eq!(doc.entities.len(), 1);

        match &doc.entities[0] {
            Entity::Line(line) => {
                assert_eq!(line.start_x, 1.0);
                assert_eq!(line.end_x, 3.0);
            }
            other => panic!("expected LINE entity, got {:?}", other),
        }
    }

    #[test]
    fn block_def_map_works() {
        let defs = vec![
            BlockDef {
                base: EntityBase::default(),
                number: 3,
                is_referenced: false,
                name: "A".to_string(),
                entities: vec![],
            },
            BlockDef {
                base: EntityBase::default(),
                number: 7,
                is_referenced: true,
                name: "B".to_string(),
                entities: vec![],
            },
        ];

        let map = block_def_name_map(&defs);
        assert_eq!(map.get(&3).map(String::as_str), Some("A"));
        assert_eq!(map.get(&7).map(String::as_str), Some("B"));
        assert_eq!(resolve_block_name(10, &defs), None);
    }

    #[test]
    fn parse_minimal_with_dimension_entity() {
        let data = build_minimal_jww_with_dimension();
        let doc = super::parse_document(&data).unwrap();
        assert_eq!(doc.entities.len(), 1);

        match &doc.entities[0] {
            Entity::Dimension(dim) => {
                assert_eq!(dim.line.start_x, 0.0);
                assert_eq!(dim.line.end_x, 10.0);
                assert_eq!(dim.text.content, "1000");
            }
            other => panic!("expected DIMENSION entity, got {:?}", other),
        }
    }

    #[test]
    fn cp932_decode_replacement_is_structured_parser_diagnostic() {
        let mut data = build_minimal_jww_with_dimension();
        let content_offset = data
            .windows(4)
            .position(|window| window == b"1000")
            .unwrap();
        data[content_offset] = 0x81;

        let parsed = parse_document_with_diagnostics(&data).unwrap();
        assert_eq!(parsed.diagnostics.len(), 1);

        let diagnostic = &parsed.diagnostics[0];
        assert_eq!(diagnostic.code, "CP932_DECODE_REPLACED");
        assert_eq!(diagnostic.severity, "warning");
        assert_eq!(diagnostic.action, "normalized");
        let details = diagnostic.decode_details().expect("decode details");
        assert_eq!(details.encoding, "cp932");
        assert_eq!(details.field, "entity.text.content");
        assert_eq!(details.byte_offset, content_offset);
        assert_eq!(details.byte_length, 4);
        assert_eq!(details.replacement_characters, 1);
        assert!(details.had_errors);

        match &parsed.document.entities[0] {
            Entity::Dimension(dim) => {
                assert!(dim.text.content.contains(char::REPLACEMENT_CHARACTER));
            }
            other => panic!("expected DIMENSION entity, got {:?}", other),
        }
    }

    #[test]
    fn parse_circle_solid_from_cdata_solid_pen_style_101() {
        let data = build_minimal_jww_with_circle_solid();
        let doc = super::parse_document(&data).unwrap();
        assert_eq!(doc.entities.len(), 1);

        match &doc.entities[0] {
            Entity::CircleSolid(solid) => {
                assert_eq!(solid.center_x, 10.0);
                assert_eq!(solid.center_y, 20.0);
                assert_eq!(solid.radius, 3.0);
                assert_eq!(solid.flatness, 1.0);
                assert_eq!(solid.arc_angle, std::f64::consts::PI * 2.0);
                assert_eq!(solid.solid_mode, 100.0);
            }
            other => panic!("expected CIRCLE_SOLID entity, got {:?}", other),
        }
    }

    #[test]
    fn validate_unresolved_block_reference() {
        let data = build_minimal_jww_with_unresolved_block_ref();
        let doc = super::parse_document(&data).unwrap();
        let validation = validate_block_references(&doc);

        assert_eq!(validation.total_references, 1);
        assert_eq!(validation.resolved_references, 0);
        assert_eq!(validation.unresolved_def_numbers, vec![99]);
        assert!(validation.has_unresolved());
    }

    /// Header for a version-600 file with an empty memo and default layer tables.
    fn minimal_header_600() -> Vec<u8> {
        let mut data = Vec::<u8>::new();
        data.extend_from_slice(b"JwwData.");
        data.extend_from_slice(&600u32.to_le_bytes());
        data.push(0); // memo
        data.extend_from_slice(&0u32.to_le_bytes()); // paper size
        data.extend_from_slice(&0u32.to_le_bytes()); // write layer group
        for _ in 0..16 {
            data.extend_from_slice(&0u32.to_le_bytes()); // state
            data.extend_from_slice(&0u32.to_le_bytes()); // write layer
            data.extend_from_slice(&1.0f64.to_le_bytes()); // scale
            data.extend_from_slice(&0u32.to_le_bytes()); // protect
            for _ in 0..16 {
                data.extend_from_slice(&0u32.to_le_bytes()); // layer state
                data.extend_from_slice(&0u32.to_le_bytes()); // layer protect
            }
        }
        data
    }

    fn append_line_payload(data: &mut Vec<u8>, index: f64) {
        append_entity_base(data);
        data.extend_from_slice(&index.to_le_bytes()); // start_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // start_y
        data.extend_from_slice(&(index + 1.0).to_le_bytes()); // end_x
        data.extend_from_slice(&1.0f64.to_le_bytes()); // end_y
    }

    fn append_new_class(data: &mut Vec<u8>, name: &[u8]) {
        data.extend_from_slice(&0xFFFFu16.to_le_bytes());
        data.extend_from_slice(&600u16.to_le_bytes());
        data.extend_from_slice(&(name.len() as u16).to_le_bytes());
        data.extend_from_slice(name);
    }

    #[test]
    fn parse_entity_list_with_escaped_dword_count() {
        // CArchive::WriteCount escapes counts >= 0xFFFF as WORD 0xFFFF + DWORD.
        let count = 70_000usize;
        let mut data = minimal_header_600();
        data.extend_from_slice(&0xFFFFu16.to_le_bytes());
        data.extend_from_slice(&(count as u32).to_le_bytes());
        append_new_class(&mut data, b"CDataSen");
        append_line_payload(&mut data, 0.0);
        for index in 1..count {
            data.extend_from_slice(&(0x8000u16 | 1).to_le_bytes()); // class ref PID 1
            append_line_payload(&mut data, index as f64);
        }
        data.extend_from_slice(&0u32.to_le_bytes()); // block def count

        let parsed = parse_document_with_diagnostics(&data).unwrap();
        // The synthetic header has no layer-name section, so the heuristic header
        // reader may emit CP932 notes; only the list itself matters here.
        assert!(parsed
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "ENTITY_LIST_TRUNCATED"));
        assert_eq!(parsed.document.entities.len(), count);
        match &parsed.document.entities[count - 1] {
            Entity::Line(line) => assert_eq!(line.start_x, (count - 1) as f64),
            other => panic!("expected LINE entity, got {:?}", other),
        }
    }

    #[test]
    fn parse_entity_list_skips_null_tags_and_reads_big_object_tags() {
        let mut data = minimal_header_600();
        data.extend_from_slice(&3u16.to_le_bytes()); // count
        append_new_class(&mut data, b"CDataSen"); // class PID 1, object PID 2
        append_line_payload(&mut data, 0.0);
        data.extend_from_slice(&0u16.to_le_bytes()); // wNullTag: NULL object
        data.extend_from_slice(&0x7FFFu16.to_le_bytes()); // wBigObjectTag
        data.extend_from_slice(&(0x8000_0000u32 | 1).to_le_bytes()); // class ref PID 1
        append_line_payload(&mut data, 5.0);
        data.extend_from_slice(&0u32.to_le_bytes()); // block def count

        let parsed = parse_document_with_diagnostics(&data).unwrap();
        assert_eq!(parsed.document.entities.len(), 2);
        assert!(parsed.diagnostics.is_empty());
        match &parsed.document.entities[1] {
            Entity::Line(line) => assert_eq!(line.start_x, 5.0),
            other => panic!("expected LINE entity, got {:?}", other),
        }
    }

    #[test]
    fn truncated_entity_list_keeps_parsed_entities_with_diagnostic() {
        let mut data = minimal_header_600();
        data.extend_from_slice(&3u16.to_le_bytes()); // announces 3 entities
        append_new_class(&mut data, b"CDataSen");
        append_line_payload(&mut data, 0.0);
        data.extend_from_slice(&(0x8000u16 | 1).to_le_bytes());
        append_line_payload(&mut data, 1.0);
        data.extend_from_slice(&(0x8000u16 | 1).to_le_bytes());
        append_entity_base(&mut data);
        data.extend_from_slice(&2.0f64.to_le_bytes()); // file ends mid-entity

        let parsed = parse_document_with_diagnostics(&data).unwrap();
        assert_eq!(parsed.document.entities.len(), 2);
        assert!(parsed.document.block_defs.is_empty());
        assert_eq!(parsed.diagnostics.len(), 1);
        let diagnostic = &parsed.diagnostics[0];
        assert_eq!(diagnostic.code, "ENTITY_LIST_TRUNCATED");
        assert_eq!(diagnostic.severity, "error");
        let details = diagnostic.truncation_details().expect("truncation details");
        assert_eq!(details.expected_entities, 3);
        assert_eq!(details.parsed_entities, 2);
        assert_eq!(details.byte_offset, data.len());
        assert!(
            details.error.contains("unexpected EOF"),
            "{}",
            details.error
        );
    }

    #[test]
    fn truncated_entity_list_without_any_entity_is_an_error() {
        let mut data = minimal_header_600();
        data.extend_from_slice(&2u16.to_le_bytes());
        append_new_class(&mut data, b"CDataSen");
        append_entity_base(&mut data); // no coordinates at all

        let err = parse_document_with_diagnostics(&data).unwrap_err();
        assert!(matches!(err, JwwError::UnexpectedEof(_)), "{err}");
    }

    #[test]
    fn unknown_class_reference_after_valid_entities_is_reported_as_truncation() {
        let mut data = minimal_header_600();
        data.extend_from_slice(&2u16.to_le_bytes());
        append_new_class(&mut data, b"CDataSen");
        append_line_payload(&mut data, 0.0);
        data.extend_from_slice(&(0x8000u16 | 0x1000).to_le_bytes()); // desync: PID 4096
        data.extend_from_slice(&[0u8; 64]);

        let parsed = parse_document_with_diagnostics(&data).unwrap();
        assert_eq!(parsed.document.entities.len(), 1);
        let details = parsed.diagnostics[0]
            .truncation_details()
            .expect("truncation details");
        assert!(
            details.error.contains("unknown class PID: 4096"),
            "{}",
            details.error
        );
    }

    #[test]
    fn unicode_strings_and_word_block_count_as_written_by_jw_cad_8() {
        // JWW version 700 written by a Unicode build: memo, text and block names use
        // the FF FE FF marker; the block definition list count is a WORD.
        let mut data = Vec::<u8>::new();
        data.extend_from_slice(b"JwwData.");
        data.extend_from_slice(&700u32.to_le_bytes());
        data.extend_from_slice(&[0xFF, 0xFE, 0xFF, 2]); // memo "\r\n" as UTF-16LE
        data.extend_from_slice(&[0x0D, 0x00, 0x0A, 0x00]);
        data.extend_from_slice(&0u32.to_le_bytes()); // paper size
        data.extend_from_slice(&0u32.to_le_bytes()); // write layer group
        for _ in 0..16 {
            data.extend_from_slice(&0u32.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
            data.extend_from_slice(&1.0f64.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
            for _ in 0..16 {
                data.extend_from_slice(&0u32.to_le_bytes());
                data.extend_from_slice(&0u32.to_le_bytes());
            }
        }
        // entity list: 1 text + 1 block reference
        data.extend_from_slice(&2u16.to_le_bytes());
        append_new_class(&mut data, b"CDataMoji");
        append_entity_base(&mut data);
        for value in [0.0f64, 0.0, 10.0, 0.0] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        data.extend_from_slice(&0u32.to_le_bytes()); // text_type
        for value in [2.5f64, 2.5, 0.0, 0.0] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        data.extend_from_slice(&[0xFF, 0xFE, 0xFF, 0]); // font name: empty unicode string
        data.extend_from_slice(&[0xFF, 0xFE, 0xFF, 2]); // content "図面"
        for unit in "図面".encode_utf16() {
            data.extend_from_slice(&unit.to_le_bytes());
        }
        append_new_class(&mut data, b"CDataBlock");
        append_entity_base(&mut data);
        for value in [0.0f64, 0.0, 1.0, 1.0, 0.0] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        data.extend_from_slice(&7u32.to_le_bytes()); // def_number
                                                     // block definition list: WORD count, one CDataList with a unicode name
        data.extend_from_slice(&1u16.to_le_bytes());
        append_new_class(&mut data, b"CDataList");
        append_entity_base(&mut data);
        data.extend_from_slice(&7u32.to_le_bytes()); // number
        data.extend_from_slice(&1u32.to_le_bytes()); // is_referenced
        data.extend_from_slice(&0u32.to_le_bytes()); // ctime
        data.extend_from_slice(&[0xFF, 0xFE, 0xFF, 3]);
        for unit in "BLK".encode_utf16() {
            data.extend_from_slice(&unit.to_le_bytes());
        }
        data.extend_from_slice(&1u16.to_le_bytes()); // nested list: 1 line
        append_new_class(&mut data, b"CDataSen");
        append_line_payload(&mut data, 3.0);

        let parsed = parse_document_with_diagnostics(&data).unwrap();
        assert_eq!(parsed.document.header.memo, "\r\n");
        assert_eq!(parsed.document.entities.len(), 2);
        match &parsed.document.entities[0] {
            Entity::Text(text) => assert_eq!(text.content, "図面"),
            other => panic!("expected TEXT entity, got {:?}", other),
        }
        assert_eq!(parsed.document.block_defs.len(), 1);
        assert_eq!(parsed.document.block_defs[0].name, "BLK");
        assert_eq!(parsed.document.block_defs[0].entities.len(), 1);
        let validation = validate_block_references(&parsed.document);
        assert_eq!(validation.unresolved_def_numbers, Vec::<u32>::new());
    }

    #[test]
    fn foreign_signature_is_named_in_the_error() {
        let data = b"ho_cad(c) something".to_vec();
        let err = parse_document_with_diagnostics(&data).unwrap_err();
        assert!(err.to_string().contains("ho_cad(c"), "{err}");
    }

    fn build_minimal_jww_with_block_def() -> Vec<u8> {
        let mut data = Vec::<u8>::new();
        data.extend_from_slice(b"JwwData.");
        data.extend_from_slice(&600u32.to_le_bytes());

        // memo CString: empty
        data.push(0);

        data.extend_from_slice(&0u32.to_le_bytes()); // paper size
        data.extend_from_slice(&0u32.to_le_bytes()); // write layer group

        for _ in 0..16 {
            data.extend_from_slice(&0u32.to_le_bytes()); // state
            data.extend_from_slice(&0u32.to_le_bytes()); // write layer
            data.extend_from_slice(&1.0f64.to_le_bytes()); // scale
            data.extend_from_slice(&0u32.to_le_bytes()); // protect
            for _ in 0..16 {
                data.extend_from_slice(&0u32.to_le_bytes()); // layer state
                data.extend_from_slice(&0u32.to_le_bytes()); // layer protect
            }
        }

        // entity list count (WORD)
        data.extend_from_slice(&1u16.to_le_bytes());

        // class definition: CDataBlock
        data.extend_from_slice(&0xFFFFu16.to_le_bytes());
        data.extend_from_slice(&600u16.to_le_bytes());
        let entity_class = b"CDataBlock";
        data.extend_from_slice(&(entity_class.len() as u16).to_le_bytes());
        data.extend_from_slice(entity_class);

        // EntityBase for block insert
        data.extend_from_slice(&0u32.to_le_bytes()); // group
        data.push(1); // pen_style
        data.extend_from_slice(&1u16.to_le_bytes()); // pen_color
        data.extend_from_slice(&1u16.to_le_bytes()); // pen_width
        data.extend_from_slice(&0u16.to_le_bytes()); // layer
        data.extend_from_slice(&0u16.to_le_bytes()); // layer_group
        data.extend_from_slice(&0u16.to_le_bytes()); // flag

        // Block insert payload: ref_x, ref_y, scale_x, scale_y, rotation, def_number
        data.extend_from_slice(&0.0f64.to_le_bytes()); // ref_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // ref_y
        data.extend_from_slice(&1.0f64.to_le_bytes()); // scale_x
        data.extend_from_slice(&1.0f64.to_le_bytes()); // scale_y
        data.extend_from_slice(&0.0f64.to_le_bytes()); // rotation
        data.extend_from_slice(&1u32.to_le_bytes()); // def_number

        // block def count (DWORD)
        data.extend_from_slice(&1u32.to_le_bytes());

        // class definition: CDataList
        data.extend_from_slice(&0xFFFFu16.to_le_bytes());
        data.extend_from_slice(&600u16.to_le_bytes());
        let class_name = b"CDataList";
        data.extend_from_slice(&(class_name.len() as u16).to_le_bytes());
        data.extend_from_slice(class_name);

        // EntityBase
        data.extend_from_slice(&0u32.to_le_bytes()); // group
        data.push(1); // pen_style
        data.extend_from_slice(&1u16.to_le_bytes()); // pen_color
        data.extend_from_slice(&1u16.to_le_bytes()); // pen_width
        data.extend_from_slice(&0u16.to_le_bytes()); // layer
        data.extend_from_slice(&0u16.to_le_bytes()); // layer_group
        data.extend_from_slice(&0u16.to_le_bytes()); // flag

        data.extend_from_slice(&1u32.to_le_bytes()); // number
        data.extend_from_slice(&0u32.to_le_bytes()); // is_referenced
        data.extend_from_slice(&0u32.to_le_bytes()); // ctime

        // CString "BLK"
        data.push(3);
        data.write_all(b"BLK").unwrap();

        // nested entity list count (WORD)
        data.extend_from_slice(&0u16.to_le_bytes());

        data
    }

    fn build_minimal_jww_with_dimension() -> Vec<u8> {
        let mut data = Vec::<u8>::new();
        data.extend_from_slice(b"JwwData.");
        data.extend_from_slice(&600u32.to_le_bytes());
        data.push(0); // memo
        data.extend_from_slice(&0u32.to_le_bytes()); // paper size
        data.extend_from_slice(&0u32.to_le_bytes()); // write layer group

        for _ in 0..16 {
            data.extend_from_slice(&0u32.to_le_bytes()); // group state
            data.extend_from_slice(&0u32.to_le_bytes()); // write layer
            data.extend_from_slice(&1.0f64.to_le_bytes()); // scale
            data.extend_from_slice(&0u32.to_le_bytes()); // protect
            for _ in 0..16 {
                data.extend_from_slice(&0u32.to_le_bytes()); // layer state
                data.extend_from_slice(&0u32.to_le_bytes()); // layer protect
            }
        }

        data.extend_from_slice(&1u16.to_le_bytes()); // entity count
        data.extend_from_slice(&0xFFFFu16.to_le_bytes()); // new class
        data.extend_from_slice(&600u16.to_le_bytes()); // schema
        let class_name = b"CDataSunpou";
        data.extend_from_slice(&(class_name.len() as u16).to_le_bytes());
        data.extend_from_slice(class_name);

        // Dimension base
        append_entity_base(&mut data);
        // Dimension line
        append_entity_base(&mut data);
        data.extend_from_slice(&0.0f64.to_le_bytes()); // start_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // start_y
        data.extend_from_slice(&10.0f64.to_le_bytes()); // end_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // end_y

        // Dimension text
        append_entity_base(&mut data);
        data.extend_from_slice(&0.0f64.to_le_bytes()); // start_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // start_y
        data.extend_from_slice(&0.0f64.to_le_bytes()); // end_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // end_y
        data.extend_from_slice(&0u32.to_le_bytes()); // text_type
        data.extend_from_slice(&1.0f64.to_le_bytes()); // size_x
        data.extend_from_slice(&1.0f64.to_le_bytes()); // size_y
        data.extend_from_slice(&0.0f64.to_le_bytes()); // spacing
        data.extend_from_slice(&0.0f64.to_le_bytes()); // angle
        data.push(0); // font_name cstring
        data.push(4); // content cstring len
        data.write_all(b"1000").unwrap();

        // version >= 420 payload
        data.extend_from_slice(&0u16.to_le_bytes()); // sxf mode
        for _ in 0..2 {
            append_entity_base(&mut data);
            data.extend_from_slice(&0.0f64.to_le_bytes());
            data.extend_from_slice(&0.0f64.to_le_bytes());
            data.extend_from_slice(&0.0f64.to_le_bytes());
            data.extend_from_slice(&0.0f64.to_le_bytes());
        }
        for _ in 0..4 {
            append_entity_base(&mut data);
            data.extend_from_slice(&0.0f64.to_le_bytes()); // x
            data.extend_from_slice(&0.0f64.to_le_bytes()); // y
            data.extend_from_slice(&0u32.to_le_bytes()); // is_temporary
        }

        data.extend_from_slice(&0u32.to_le_bytes()); // block def count
        data
    }

    fn build_minimal_jww_420_with_schema_1_line() -> Vec<u8> {
        let mut data = Vec::<u8>::new();
        data.extend_from_slice(b"JwwData.");
        data.extend_from_slice(&420u32.to_le_bytes());

        data.push(0); // memo
        data.extend_from_slice(&0u32.to_le_bytes()); // paper size
        data.extend_from_slice(&0u32.to_le_bytes()); // write layer group

        for _ in 0..16 {
            data.extend_from_slice(&0u32.to_le_bytes()); // state
            data.extend_from_slice(&0u32.to_le_bytes()); // write layer
            data.extend_from_slice(&1.0f64.to_le_bytes()); // scale
            data.extend_from_slice(&0u32.to_le_bytes()); // protect
            for _ in 0..16 {
                data.extend_from_slice(&0u32.to_le_bytes()); // layer state
                data.extend_from_slice(&0u32.to_le_bytes()); // layer protect
            }
        }

        data.extend_from_slice(&1u16.to_le_bytes()); // entity count
        data.extend_from_slice(&0xFFFFu16.to_le_bytes()); // new class
        data.extend_from_slice(&1u16.to_le_bytes()); // class schema
        let class_name = b"CDataSen";
        data.extend_from_slice(&(class_name.len() as u16).to_le_bytes());
        data.extend_from_slice(class_name);

        append_entity_base(&mut data);
        data.extend_from_slice(&1.0f64.to_le_bytes()); // start_x
        data.extend_from_slice(&2.0f64.to_le_bytes()); // start_y
        data.extend_from_slice(&3.0f64.to_le_bytes()); // end_x
        data.extend_from_slice(&4.0f64.to_le_bytes()); // end_y

        data.extend_from_slice(&0u32.to_le_bytes()); // block def count
        data
    }

    fn build_minimal_jww_with_circle_solid() -> Vec<u8> {
        let mut data = Vec::<u8>::new();
        data.extend_from_slice(b"JwwData.");
        data.extend_from_slice(&600u32.to_le_bytes());
        data.push(0); // memo
        data.extend_from_slice(&0u32.to_le_bytes()); // paper size
        data.extend_from_slice(&0u32.to_le_bytes()); // write layer group

        for _ in 0..16 {
            data.extend_from_slice(&0u32.to_le_bytes()); // state
            data.extend_from_slice(&0u32.to_le_bytes()); // write layer
            data.extend_from_slice(&1.0f64.to_le_bytes()); // scale
            data.extend_from_slice(&0u32.to_le_bytes()); // protect
            for _ in 0..16 {
                data.extend_from_slice(&0u32.to_le_bytes()); // layer state
                data.extend_from_slice(&0u32.to_le_bytes()); // layer protect
            }
        }

        data.extend_from_slice(&1u16.to_le_bytes()); // entity count
        data.extend_from_slice(&0xFFFFu16.to_le_bytes()); // new class
        data.extend_from_slice(&600u16.to_le_bytes()); // schema
        let class_name = b"CDataSolid";
        data.extend_from_slice(&(class_name.len() as u16).to_le_bytes());
        data.extend_from_slice(class_name);

        append_entity_base_with_style_and_color(&mut data, 101, 10);
        data.extend_from_slice(&10.0f64.to_le_bytes()); // center x
        data.extend_from_slice(&20.0f64.to_le_bytes()); // center y
        data.extend_from_slice(&3.0f64.to_le_bytes()); // radius
        data.extend_from_slice(&1.0f64.to_le_bytes()); // flatness
        data.extend_from_slice(&0.0f64.to_le_bytes()); // tilt angle
        data.extend_from_slice(&0.0f64.to_le_bytes()); // start angle
        data.extend_from_slice(&(std::f64::consts::PI * 2.0).to_le_bytes()); // arc angle
        data.extend_from_slice(&100.0f64.to_le_bytes()); // full circle solid
        data.extend_from_slice(&0u32.to_le_bytes()); // solid RGB color

        data.extend_from_slice(&0u32.to_le_bytes()); // block def count
        data
    }

    fn append_entity_base(data: &mut Vec<u8>) {
        append_entity_base_with_style_and_color(data, 1, 1);
    }

    fn append_entity_base_with_style_and_color(data: &mut Vec<u8>, pen_style: u8, pen_color: u16) {
        data.extend_from_slice(&0u32.to_le_bytes()); // group
        data.push(pen_style);
        data.extend_from_slice(&pen_color.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes()); // pen_width
        data.extend_from_slice(&0u16.to_le_bytes()); // layer
        data.extend_from_slice(&0u16.to_le_bytes()); // layer_group
        data.extend_from_slice(&0u16.to_le_bytes()); // flag
    }

    fn build_minimal_jww_with_unresolved_block_ref() -> Vec<u8> {
        let mut data = Vec::<u8>::new();
        data.extend_from_slice(b"JwwData.");
        data.extend_from_slice(&600u32.to_le_bytes());
        data.push(0); // memo
        data.extend_from_slice(&0u32.to_le_bytes()); // paper size
        data.extend_from_slice(&0u32.to_le_bytes()); // write layer group

        for _ in 0..16 {
            data.extend_from_slice(&0u32.to_le_bytes()); // state
            data.extend_from_slice(&0u32.to_le_bytes()); // write layer
            data.extend_from_slice(&1.0f64.to_le_bytes()); // scale
            data.extend_from_slice(&0u32.to_le_bytes()); // protect
            for _ in 0..16 {
                data.extend_from_slice(&0u32.to_le_bytes()); // layer state
                data.extend_from_slice(&0u32.to_le_bytes()); // layer protect
            }
        }

        data.extend_from_slice(&1u16.to_le_bytes()); // entity count
        data.extend_from_slice(&0xFFFFu16.to_le_bytes()); // new class
        data.extend_from_slice(&600u16.to_le_bytes()); // schema
        let class_name = b"CDataBlock";
        data.extend_from_slice(&(class_name.len() as u16).to_le_bytes());
        data.extend_from_slice(class_name);

        append_entity_base(&mut data);
        data.extend_from_slice(&0.0f64.to_le_bytes()); // ref_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // ref_y
        data.extend_from_slice(&1.0f64.to_le_bytes()); // scale_x
        data.extend_from_slice(&1.0f64.to_le_bytes()); // scale_y
        data.extend_from_slice(&0.0f64.to_le_bytes()); // rotation
        data.extend_from_slice(&99u32.to_le_bytes()); // unresolved def_number

        data.extend_from_slice(&0u32.to_le_bytes()); // block def count
        data
    }
}
