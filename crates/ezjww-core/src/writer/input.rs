//! Shared strict input decoder for Python and JavaScript writer dictionaries.
use super::{JwwWriteDocument, JwwWriteError, JwwWriteOptions};
use crate::base64;
use crate::model::{Block, BlockDef, CircleSolid, Dimension, EmbeddedImage, Solid};
use crate::{Arc, Entity, EntityBase, LayerGroupHeader, LayerHeader, Line, Point, Text};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::collections::HashSet;

fn invalid(path: &str, message: &str) -> JwwWriteError {
    JwwWriteError::invalid(path, message)
}
struct Input<'a> {
    fields: &'a Map<String, Value>,
    path: String,
}
impl<'a> Input<'a> {
    fn new(value: &'a Value, path: &str, allowed: &[&str]) -> Result<Self, JwwWriteError> {
        let fields = value
            .as_object()
            .ok_or_else(|| invalid(path, "expected an object"))?;
        for key in fields.keys() {
            if !allowed.contains(&key.as_str()) {
                return Err(invalid(&format!("{path}.{key}"), "unsupported field"));
            }
        }
        Ok(Self {
            fields,
            path: path.into(),
        })
    }
    fn get(&self, key: &str) -> Result<&'a Value, JwwWriteError> {
        self.fields
            .get(key)
            .ok_or_else(|| self.error(key, "missing field"))
    }
    fn error(&self, key: &str, message: &str) -> JwwWriteError {
        invalid(&format!("{}.{key}", self.path), message)
    }
    fn number(&self, key: &str) -> Result<f64, JwwWriteError> {
        self.get(key)?
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or_else(|| self.error(key, "expected a finite number"))
    }
    fn integer<T: TryFrom<u32>>(&self, key: &str) -> Result<T, JwwWriteError> {
        let v = self.number(key)?;
        if v.fract() != 0.0 || !(0.0..=u32::MAX as f64).contains(&v) {
            return Err(self.error(key, "expected unsigned integer in range"));
        }
        T::try_from(v as u32).map_err(|_| self.error(key, "integer exceeds field range"))
    }
    fn boolean(&self, key: &str) -> Result<bool, JwwWriteError> {
        self.get(key)?
            .as_bool()
            .ok_or_else(|| self.error(key, "expected bool"))
    }
    fn string(&self, key: &str) -> Result<String, JwwWriteError> {
        self.get(key)?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| self.error(key, "expected a string"))
    }
    fn optional<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, JwwWriteError> {
        match self.fields.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => serde_json::from_value(v.clone())
                .map(Some)
                .map_err(|e| self.error(key, &e.to_string())),
        }
    }
}
fn array<'a>(v: &'a Value, path: &str) -> Result<&'a Vec<Value>, JwwWriteError> {
    v.as_array()
        .ok_or_else(|| invalid(path, "expected an array"))
}

/// Template defaults with editable color, line type and character size tables.
pub fn new_document() -> Value {
    let mut doc = JwwWriteDocument::default();
    let header = super::header::default_tables();
    doc.options.palette = header.palette;
    doc.options.line_types = header.line_types;
    doc.options.text_presets = header.text_presets;
    doc.options.pens = header.pens;
    serde_json::to_value(doc).expect("finite defaults")
}

pub fn from_value(mut value: Value) -> Result<JwwWriteDocument, JwwWriteError> {
    if value.get("header").is_some() {
        return Err(invalid(
            "document.header",
            "reader documents require to_write_document()",
        ));
    }
    allocate_colors(&mut value)?;
    let input = Input::new(
        &value,
        "document",
        &["options", "entities", "block_defs", "images"],
    )?;
    let options = options(input.get("options")?)?;
    let entities = array(input.get("entities")?, "entities")?
        .iter()
        .enumerate()
        .map(|(i, v)| entity(v, &format!("entities[{i}]")))
        .collect::<Result<_, _>>()?;
    let mut block_defs = Vec::new();
    if let Some(values) = input.fields.get("block_defs") {
        for (i, value) in array(values, "block_defs")?.iter().enumerate() {
            block_defs.push(block_def(value, i)?);
        }
    }
    let mut images = Vec::new();
    if let Some(values) = input.fields.get("images") {
        for (i, value) in array(values, "images")?.iter().enumerate() {
            images.push(image(value, i)?);
        }
    }
    Ok(JwwWriteDocument {
        options,
        entities,
        block_defs,
        images,
    })
}

/// `to_jww_bytes(&from_value(document))` for a document whose lists arrive one
/// element at a time. Each entity is encoded when it arrives, so neither the
/// input as one `Value` tree nor the decoded entities are ever held: for a
/// drawing of 840,000 entities the peak rose by 1.6 GB and now by 85 MB.
///
/// Push the `block_defs`, then the `entities`, then the `images`, each list in
/// order: the order in which `from_value` allocates the pens of RGB colors. A
/// document that `from_value` and `to_jww_bytes` accept gives the same bytes.
/// Each element is checked when it arrives, so of several errors in a document
/// another one than the one those report may come first.
pub struct DocumentEncoder {
    options: Value,
    /// `options` as decoded at the start, for the checks of each entity; the
    /// header is written in `finish` from the options with the final palette.
    checked: JwwWriteOptions,
    colors: ColorAllocator,
    block_defs: Vec<BlockDef>,
    images: Vec<EmbeddedImage>,
    /// The archive from the first entity on: the entities as they arrive, then
    /// the block definitions and images. The header writes no archive objects,
    /// so the class and object numbers here do not depend on it.
    body: super::archive::ArchiveWriter,
    entities: usize,
    /// `def_number` of each top-level BLOCK entity, for the block checks.
    block_numbers: Vec<u32>,
    /// Texts that place an image, checked against the images in `finish`.
    image_texts: Vec<(String, Entity)>,
    /// 0 = block definitions, 1 = entities, 2 = images: the list being pushed.
    list: u8,
}

impl DocumentEncoder {
    /// `options` is the document's `options`; `used` holds the pens of every
    /// element of the document's lists (`scan_pens`), which allocated pens avoid.
    pub fn new(options: Value, used: HashSet<u16>) -> Result<Self, JwwWriteError> {
        let colors = ColorAllocator::new(used, options.get("palette"))?;
        let checked = self::options(&options)?;
        Ok(Self {
            options,
            checked,
            colors,
            block_defs: Vec::new(),
            images: Vec::new(),
            body: super::archive::ArchiveWriter::new(),
            entities: 0,
            block_numbers: Vec::new(),
            image_texts: Vec::new(),
            list: 0,
        })
    }

    pub fn push_block_def(&mut self, mut value: Value) -> Result<(), JwwWriteError> {
        self.enter(0, "block_defs")?;
        let i = self.block_defs.len();
        let at = At::Key(&At::Root("document"), "block_defs");
        self.colors.apply(&mut value, &At::Index(&at, i))?;
        let definition = block_def(&value, i)?;
        for (j, item) in definition.entities.iter().enumerate() {
            let path = format!("block_defs[{i}].entities[{j}]");
            super::validate::entity(item, &path, &self.checked)?;
            self.keep_image_text(item, path);
        }
        self.block_defs.push(definition);
        Ok(())
    }

    pub fn push_entity(&mut self, mut value: Value) -> Result<(), JwwWriteError> {
        self.enter(1, "entities")?;
        let i = self.entities;
        let at = At::Key(&At::Root("document"), "entities");
        self.colors.apply(&mut value, &At::Index(&at, i))?;
        let path = format!("entities[{i}]");
        let item = entity(&value, &path)?;
        super::validate::entity(&item, &path, &self.checked)?;
        if let Entity::Block(block) = &item {
            self.block_numbers.push(block.def_number);
        }
        super::entities::write(&mut self.body, &item)?;
        self.keep_image_text(&item, path);
        self.entities += 1;
        Ok(())
    }

    pub fn push_image(&mut self, mut value: Value) -> Result<(), JwwWriteError> {
        self.enter(2, "images")?;
        let i = self.images.len();
        let at = At::Key(&At::Root("document"), "images");
        self.colors.apply(&mut value, &At::Index(&at, i))?;
        self.images.push(image(&value, i)?);
        Ok(())
    }

    /// The JWW file: the header and the entity count in front of the encoded
    /// entities, then the block definitions and images.
    pub fn finish(mut self) -> Result<Vec<u8>, JwwWriteError> {
        if let Some(palette) = self.colors.allocated_palette() {
            let options = self
                .options
                .as_object_mut()
                .ok_or_else(|| invalid("options", "expected an object"))?;
            options.insert("palette".into(), palette);
        }
        let options = options(&self.options)?;
        super::validate::write_options(&options)?;
        super::validate::blocks(&self.block_defs, self.block_numbers.iter().copied())?;
        super::validate::images(&self.images)?;
        for (path, text) in &self.image_texts {
            super::validate::image_reference(text, path, &self.images)?;
        }
        super::write_tail(&mut self.body, &self.block_defs, &self.images)?;
        let mut head = super::archive::ArchiveWriter::new();
        super::header::write(&mut head, &options)?;
        head.count(self.entities, "entities")?;
        let mut bytes = self.body.into_bytes();
        bytes.splice(0..0, head.into_bytes());
        Ok(bytes)
    }

    fn keep_image_text(&mut self, item: &Entity, path: String) {
        if let Entity::Text(text) = item {
            if text.image_reference().is_some() {
                self.image_texts.push((path, item.clone()));
            }
        }
    }

    fn enter(&mut self, list: u8, name: &str) -> Result<(), JwwWriteError> {
        if list < self.list {
            return Err(invalid(name, "push block_defs, then entities, then images"));
        }
        self.list = list;
        Ok(())
    }
}

/// `PEN_MARK + k` marks where the pen of the k-th RGB color of an entity is
/// written. The checks allow no pen above 356, so no other base has a mark.
const PEN_MARK: usize = 0x200;

/// The entities of a document encoded as they arrive, before its block
/// definitions, images and final options are known: `finish` takes those.
/// The caller never holds all entities, neither does the stream.
///
/// The pens of RGB colors depend on the whole document (they avoid every pen
/// it references, and the block definitions take theirs first), so the stream
/// writes a placeholder pen and `finish` chooses the pens in the order of
/// `from_value` and writes them over the placeholders. For the accepted
/// entities, `finish` gives the bytes of `to_jww_bytes(&from_value(document))`.
/// An entity that `push_entity` refuses is not written, and the stream goes on.
pub struct EntityStream {
    /// `options` as decoded at the start, for the checks of each entity.
    checked: JwwWriteOptions,
    body: super::archive::ArchiveWriter,
    /// Accepted entities, for the count in front of them.
    entities: usize,
    /// Entities offered, accepted or not: the index of the next one.
    offered: usize,
    /// The pens the accepted entities reference (`scan_pens`).
    used: HashSet<u16>,
    /// Each RGB color of the accepted entities, in the order in which
    /// `from_value` gives them a pen: where its pen goes in `body`, the
    /// COLORREF and the index of its entity.
    pens: Vec<(usize, u32, usize)>,
    /// `def_number` of each accepted BLOCK entity, for the block checks.
    block_numbers: Vec<u32>,
    /// Texts that place an image, checked against the images in `finish`.
    image_texts: Vec<(String, Entity)>,
    /// A failed write left `body` incomplete.
    broken: bool,
    /// `finish` returned the file.
    finished: bool,
}

impl EntityStream {
    /// `options` are the document's options at the start, for the checks of
    /// the entities; `finish` writes the header from the options it is given.
    pub fn new(options: &Value) -> Result<Self, JwwWriteError> {
        Ok(Self {
            checked: self::options(options)?,
            body: super::archive::ArchiveWriter::new(),
            entities: 0,
            offered: 0,
            used: HashSet::new(),
            pens: Vec::new(),
            block_numbers: Vec::new(),
            image_texts: Vec::new(),
            broken: false,
            finished: false,
        })
    }

    /// `Err` with the reason once the stream takes no more entities.
    pub fn check_open(&self) -> Result<(), JwwWriteError> {
        if self.finished {
            Err(invalid("entities", "the stream was already finished"))
        } else if self.broken {
            Err(invalid(
                "entities",
                "an earlier entity could not be written",
            ))
        } else {
            Ok(())
        }
    }

    /// The index of the next entity in error messages (`entities[i]`).
    pub fn next_index(&self) -> usize {
        self.offered
    }

    /// Counts an entity that the caller refused before `push_entity` (for
    /// example one its own conversion rejected), so the indexes stay those of
    /// the offered entities.
    pub fn skip_entity(&mut self) {
        self.offered += 1;
    }

    /// Whether the stream takes no more entities: `finish` returned the file,
    /// or a failed write left it incomplete.
    pub fn is_closed(&self) -> bool {
        self.finished || self.broken
    }

    /// Checks and encodes one entity. On `Err` the entity is not written; the
    /// stream goes on unless `is_closed`.
    pub fn push_entity(&mut self, mut value: Value) -> Result<(), JwwWriteError> {
        self.check_open()?;
        let i = self.offered;
        self.offered += 1;
        let list = At::Key(&At::Root("document"), "entities");
        let at = At::Index(&list, i);
        let mut used = HashSet::new();
        scan_pens(&value, &mut used);
        let mut colors = Vec::new();
        take_colors(&mut value, &at, "", &mut colors)?;
        let path = format!("entities[{i}]");
        let item = entity(&value, &path)?;
        super::validate::entity(&item, &path, &self.checked)?;
        // With RGB colors, the same entity is written with a mark in each
        // color's pen, to find where the pens go (the bases are written in
        // another order than `from_value` gives the colors their pens).
        let mut marks = Vec::with_capacity(colors.len());
        for (k, (pointer, _)) in colors.iter().enumerate() {
            let mark = u16::try_from(PEN_MARK + k)
                .map_err(|_| invalid(&path, "too many RGB colors in one entity"))?;
            let pen = value
                .pointer_mut(&format!("{pointer}/base/pen_color"))
                .expect("take_colors gave the base a placeholder pen");
            *pen = Value::from(mark);
            marks.push(mark);
        }
        let marked = if marks.is_empty() {
            None
        } else {
            Some(entity(&value, &path)?)
        };
        if marked.is_some() {
            self.body.record_pen_colors();
        }
        let written = super::entities::write(&mut self.body, marked.as_ref().unwrap_or(&item));
        let offsets = self.body.take_pen_colors();
        if let Err(e) = written {
            self.broken = true;
            return Err(e);
        }
        for ((_, color), mark) in colors.into_iter().zip(marks) {
            let offset = offsets
                .iter()
                .copied()
                .find(|&o| self.body.u16_at(o) == mark)
                .expect("each RGB color's base is written");
            self.pens.push((offset, color, i));
        }
        if let Entity::Block(block) = &item {
            self.block_numbers.push(block.def_number);
        }
        if let Entity::Text(text) = &item {
            if text.image_reference().is_some() {
                self.image_texts.push((path, item.clone()));
            }
        }
        self.used.extend(used);
        self.entities += 1;
        Ok(())
    }

    /// The JWW file of the accepted entities with these `options`,
    /// `block_defs` and `images`. On `Err` nothing changed, so the caller can
    /// correct the input and call `finish` again.
    pub fn finish(
        &mut self,
        options: Value,
        block_defs: Vec<Value>,
        images: Vec<Value>,
    ) -> Result<Vec<u8>, JwwWriteError> {
        let mut used = HashSet::new();
        for value in block_defs.iter().chain(&images) {
            scan_pens(value, &mut used);
        }
        let mut finisher = self.finisher(options, used)?;
        for value in block_defs {
            finisher.push_block_def(value)?;
        }
        for value in images {
            finisher.push_image(value)?;
        }
        finisher.finish()
    }

    /// `finish` for a caller that converts the block definitions and images
    /// one at a time (a definition as one JSON tree is far larger than as the
    /// writer's model): `used` are the pens they reference (`scan_pens`).
    /// Push the block definitions, then the images, then `finish` the
    /// `StreamFinisher`; on `Err` the stream is unchanged.
    pub fn finisher(
        &mut self,
        options: Value,
        used: HashSet<u16>,
    ) -> Result<StreamFinisher<'_>, JwwWriteError> {
        self.check_open()?;
        // `from_value`: the pens of the whole document, then the colors of the
        // block definitions, the entities and the images in this order.
        let mut all = self.used.clone();
        all.extend(used);
        let colors = ColorAllocator::new(all, options.get("palette"))?;
        Ok(StreamFinisher {
            stream: self,
            options,
            colors,
            definitions: Vec::new(),
            embedded: Vec::new(),
            image_texts: Vec::new(),
            pens: None,
        })
    }
}

/// The end of an `EntityStream` (`EntityStream::finisher`).
pub struct StreamFinisher<'a> {
    stream: &'a mut EntityStream,
    options: Value,
    colors: ColorAllocator,
    definitions: Vec<BlockDef>,
    embedded: Vec<EmbeddedImage>,
    /// Block texts that place an image.
    image_texts: Vec<(String, Entity)>,
    /// Where each entity's RGB pen goes and the pen: chosen after the colors
    /// of the block definitions and before those of the images.
    pens: Option<Vec<(usize, u16)>>,
}

impl StreamFinisher<'_> {
    pub fn push_block_def(&mut self, mut value: Value) -> Result<(), JwwWriteError> {
        if self.pens.is_some() {
            return Err(invalid(
                "block_defs",
                "push the block definitions before the images",
            ));
        }
        let i = self.definitions.len();
        let list = At::Key(&At::Root("document"), "block_defs");
        self.colors.apply(&mut value, &At::Index(&list, i))?;
        let definition = block_def(&value, i)?;
        for (j, item) in definition.entities.iter().enumerate() {
            let path = format!("block_defs[{i}].entities[{j}]");
            super::validate::entity(item, &path, &self.stream.checked)?;
            if let Entity::Text(text) = item {
                if text.image_reference().is_some() {
                    self.image_texts.push((path, item.clone()));
                }
            }
        }
        self.definitions.push(definition);
        Ok(())
    }

    pub fn push_image(&mut self, mut value: Value) -> Result<(), JwwWriteError> {
        self.entity_pens()?;
        let i = self.embedded.len();
        let list = At::Key(&At::Root("document"), "images");
        self.colors.apply(&mut value, &At::Index(&list, i))?;
        self.embedded.push(image(&value, i)?);
        Ok(())
    }

    fn entity_pens(&mut self) -> Result<(), JwwWriteError> {
        if self.pens.is_none() {
            let list = At::Key(&At::Root("document"), "entities");
            let mut pens = Vec::with_capacity(self.stream.pens.len());
            for &(offset, color, i) in &self.stream.pens {
                pens.push((offset, self.colors.pen(color, &At::Index(&list, i))?));
            }
            self.pens = Some(pens);
        }
        Ok(())
    }

    /// The JWW file; the stream then takes no more entities.
    pub fn finish(mut self) -> Result<Vec<u8>, JwwWriteError> {
        self.entity_pens()?;
        if let Some(palette) = self.colors.allocated_palette() {
            self.options
                .as_object_mut()
                .ok_or_else(|| invalid("options", "expected an object"))?
                .insert("palette".into(), palette);
        }
        let options = options(&self.options)?;
        let stream = self.stream;
        super::validate::write_options(&options)?;
        super::validate::blocks(&self.definitions, stream.block_numbers.iter().copied())?;
        super::validate::images(&self.embedded)?;
        for (path, text) in stream.image_texts.iter().chain(&self.image_texts) {
            super::validate::image_reference(text, path, &self.embedded)?;
        }
        let mut head = super::archive::ArchiveWriter::new();
        super::header::write(&mut head, &options)?;
        head.count(stream.entities, "entities")?;
        let checkpoint = stream.body.checkpoint();
        if let Err(e) = super::write_tail(&mut stream.body, &self.definitions, &self.embedded) {
            stream.body.rewind(checkpoint);
            return Err(e);
        }
        for (offset, pen) in self.pens.into_iter().flatten() {
            stream.body.set_u16_at(offset, pen);
        }
        let body = std::mem::replace(&mut stream.body, super::archive::ArchiveWriter::new());
        stream.finished = true;
        let mut bytes = body.into_bytes();
        bytes.splice(0..0, head.into_bytes());
        Ok(bytes)
    }
}

/// What `ColorAllocator::apply` does to `v` before it chooses a pen: each RGB
/// `color` that gets a pen is removed and its base gets pen 1, a valid
/// placeholder for the checks. Appends the base's JSON pointer (relative to
/// `v`) and the COLORREF to `out`, in `apply`'s order.
fn take_colors(
    v: &mut Value,
    at: &At<'_>,
    pointer: &str,
    out: &mut Vec<(String, u32)>,
) -> Result<(), JwwWriteError> {
    if let Some(obj) = v.as_object_mut() {
        if let Some(color) = rgb_color(obj) {
            obj.remove("color");
            let n = color.as_u64().filter(|v| *v <= 0xffffff).ok_or_else(|| {
                invalid(
                    &format!("{}.color", at.render()),
                    "expected COLORREF 0..=0xffffff",
                )
            });
            if is_solid(obj) {
                obj.insert("color".into(), color);
            } else {
                let n = n? as u32;
                let b = obj
                    .get_mut("base")
                    .and_then(Value::as_object_mut)
                    .ok_or_else(|| invalid(&format!("{}.base", at.render()), "RGB needs a base"))?;
                b.insert("pen_color".into(), Value::from(1));
                out.push((pointer.to_owned(), n));
            }
        }
        for (key, v) in obj.iter_mut() {
            if key != "options" {
                let escaped = key.replace('~', "~0").replace('/', "~1");
                take_colors(v, &At::Key(at, key), &format!("{pointer}/{escaped}"), out)?;
            }
        }
    } else if let Some(values) = v.as_array_mut() {
        for (i, v) in values.iter_mut().enumerate() {
            take_colors(v, &At::Index(at, i), &format!("{pointer}/{i}"), out)?;
        }
    }
    Ok(())
}

fn block_def(value: &Value, i: usize) -> Result<BlockDef, JwwWriteError> {
    let path = format!("block_defs[{i}]");
    let b = Input::new(
        value,
        &path,
        &["base", "number", "is_referenced", "name", "entities"],
    )?;
    Ok(BlockDef {
        base: base(b.get("base")?, &format!("{path}.base"))?,
        number: b.integer("number")?,
        is_referenced: b.boolean("is_referenced")?,
        name: b.string("name")?,
        entities: array(b.get("entities")?, &format!("{path}.entities"))?
            .iter()
            .enumerate()
            .map(|(j, v)| entity(v, &format!("{path}.entities[{j}]")))
            .collect::<Result<_, _>>()?,
    })
}

fn image(value: &Value, i: usize) -> Result<EmbeddedImage, JwwWriteError> {
    let path = format!("images[{i}]");
    let image = Input::new(value, &path, &["name", "data", "compressed"])?;
    let name = image.string("name")?;
    let data = base64::decode(&image.string("data")?)
        .map_err(|reason| invalid(&format!("{path}.data"), &reason))?;
    let parsed = EmbeddedImage { name, data };
    if let Some(flag) = image.fields.get("compressed") {
        let flag = flag
            .as_bool()
            .ok_or_else(|| invalid(&format!("{path}.compressed"), "expected bool"))?;
        if flag != parsed.is_compressed() {
            return Err(invalid(
                &format!("{path}.compressed"),
                "must agree with the .gz name suffix",
            ));
        }
    }
    Ok(parsed)
}

fn member(
    value: &Value,
    kind: &str,
    path: &str,
    mut defaults: EntityBase,
) -> Result<Entity, JwwWriteError> {
    let mut v = value.clone();
    let obj = v
        .as_object_mut()
        .ok_or_else(|| invalid(path, "expected an object"))?;
    if obj.contains_key("type") {
        return Err(invalid(path, "dimension members omit type"));
    }
    obj.insert("type".into(), Value::from(kind));
    defaults.pen_style = 1;
    defaults.pen_width = 0;
    defaults.flag = match kind {
        "TEXT" => 0x4010,
        "POINT" => 0x40,
        _ => 0x2000,
    };
    obj.entry("base")
        .or_insert_with(|| serde_json::to_value(defaults).unwrap());
    entity(&v, path)
}

/// A general entity has no RGB payload. Reuse matching pens or allocate unused
/// user slots, never overwriting a color referenced by another entity.
fn allocate_colors(value: &mut Value) -> Result<(), JwwWriteError> {
    let mut used = HashSet::new();
    scan_pens(value, &mut used);
    let mut colors = ColorAllocator::new(used, value.pointer("/options/palette"))?;
    colors.apply(value, &At::Root("document"))?;
    if let Some(palette) = colors.allocated_palette() {
        value["options"]["palette"] = palette;
    }
    Ok(())
}

/// Adds every `base.pen_color` in `value` that is an unsigned integer up to
/// `u16::MAX`, at any depth except under `options` keys, to `used`: the pens
/// that `from_value` and `DocumentEncoder` never give another color.
pub fn scan_pens(value: &Value, used: &mut HashSet<u16>) {
    if let Some(p) = value
        .get("base")
        .and_then(|b| b.get("pen_color"))
        .and_then(Value::as_u64)
    {
        if p <= u16::MAX as u64 {
            used.insert(p as u16);
        }
    }
    if let Some(obj) = value.as_object() {
        for (key, v) in obj {
            if key != "options" {
                scan_pens(v, used);
            }
        }
    }
    if let Some(array) = value.as_array() {
        for v in array {
            scan_pens(v, used);
        }
    }
}

/// Where a value sits, rendered only for an error message (the walk visits
/// every value of the document; formatting each path was most of its time).
#[derive(Clone, Copy)]
enum At<'a> {
    Root(&'a str),
    Index(&'a At<'a>, usize),
    Key(&'a At<'a>, &'a str),
}

impl At<'_> {
    fn render(&self) -> String {
        match self {
            At::Root(name) => (*name).to_owned(),
            At::Index(parent, index) => format!("{}[{index}]", parent.render()),
            At::Key(parent, key) => format!("{}.{key}", parent.render()),
        }
    }
}

/// Pens for the RGB `color` of entities, in the order in which values reach
/// `apply`: a pen whose screen color matches, else the first user slot that no
/// entity references, with its palette color set.
struct ColorAllocator {
    used: HashSet<u16>,
    palette: crate::header::JwwPalette,
    changed: bool,
}

impl ColorAllocator {
    /// `used` are the pens of the document (`scan_pens`); `palette` is its
    /// `options.palette` (absent or null: the template palette).
    fn new(used: HashSet<u16>, palette: Option<&Value>) -> Result<Self, JwwWriteError> {
        let palette = match palette.filter(|v| !v.is_null()) {
            Some(v) => serde_json::from_value(v.clone())
                .map_err(|e| invalid("options.palette", &e.to_string()))?,
            None => super::header::default_tables().palette.unwrap(),
        };
        if palette
            .extended_colors
            .as_ref()
            .is_none_or(|v| v.len() != 257)
        {
            return Err(invalid(
                "options.palette.extended_colors",
                "expected 257 colors",
            ));
        }
        Ok(Self {
            used,
            palette,
            changed: false,
        })
    }

    /// The palette to store in `options.palette` once a slot was allocated.
    fn allocated_palette(&self) -> Option<Value> {
        self.changed
            .then(|| serde_json::to_value(&self.palette).unwrap())
    }

    /// Replaces the RGB colors in `v`, at any depth except under `options` keys.
    fn apply(&mut self, v: &mut Value, at: &At<'_>) -> Result<(), JwwWriteError> {
        if let Some(obj) = v.as_object_mut() {
            if let Some(color) = rgb_color(obj) {
                obj.remove("color");
                let n = color.as_u64().filter(|v| *v <= 0xffffff).ok_or_else(|| {
                    invalid(
                        &format!("{}.color", at.render()),
                        "expected COLORREF 0..=0xffffff",
                    )
                });
                if is_solid(obj) {
                    obj.insert("color".into(), color);
                } else {
                    let pen = self.pen(n? as u32, at)?;
                    let b = obj
                        .get_mut("base")
                        .and_then(Value::as_object_mut)
                        .ok_or_else(|| {
                            invalid(&format!("{}.base", at.render()), "RGB needs a base")
                        })?;
                    b.insert("pen_color".into(), Value::from(pen));
                }
            }
            for (key, v) in obj.iter_mut() {
                if key != "options" {
                    self.apply(v, &At::Key(at, key))?;
                }
            }
        } else if let Some(values) = v.as_array_mut() {
            for (i, v) in values.iter_mut().enumerate() {
                self.apply(v, &At::Index(at, i))?;
            }
        }
        Ok(())
    }

    /// The pen for the RGB `color` of the value at `at`.
    fn pen(&mut self, color: u32, at: &At<'_>) -> Result<u16, JwwWriteError> {
        let existing = (1..=9)
            .chain(101..=356)
            .find(|&p| self.palette.screen_color(p) == Some(color));
        let pen = if let Some(p) = existing {
            p
        } else {
            let p = (117..=356)
                .find(|p| !self.used.contains(p))
                .ok_or_else(|| {
                    invalid(
                        &format!("{}.color", at.render()),
                        "no unused SXF palette slots; cannot approximate RGB",
                    )
                })?;
            self.palette.extended_colors.as_mut().unwrap()[(p - 100) as usize] = color;
            self.changed = true;
            p
        };
        self.used.insert(pen);
        Ok(pen)
    }
}

/// The `color` of an object that `ColorAllocator::apply` replaces or checks:
/// one with a `base`, of an entity type with colors or of no type (a member).
fn rgb_color(obj: &Map<String, Value>) -> Option<Value> {
    obj.get("color").cloned().filter(|_| {
        obj.contains_key("base")
            && matches!(
                obj.get("type").and_then(Value::as_str),
                None | Some(
                    "LINE" | "ARC" | "CIRCLE" | "POINT" | "TEXT" | "SOLID" | "CIRCLE_SOLID"
                )
            )
    })
}

/// Solids keep their RGB `color`; the other entities get a pen for it.
fn is_solid(obj: &Map<String, Value>) -> bool {
    matches!(
        obj.get("type").and_then(Value::as_str),
        Some("SOLID" | "CIRCLE_SOLID")
    )
}

fn options(value: &Value) -> Result<JwwWriteOptions, JwwWriteError> {
    let input = Input::new(
        value,
        "options",
        &[
            "version",
            "memo",
            "paper_size",
            "write_layer_group",
            "layer_groups",
            "palette",
            "line_types",
            "text_presets",
            "pens",
        ],
    )?;
    let groups = array(input.get("layer_groups")?, "options.layer_groups")?;
    if groups.len() != 16 {
        return Err(invalid(
            "options.layer_groups",
            "expected exactly 16 groups",
        ));
    }
    let mut out = JwwWriteOptions {
        version: input.integer("version")?,
        memo: input.string("memo")?,
        paper_size: input.integer("paper_size")?,
        write_layer_group: input.integer("write_layer_group")?,
        ..JwwWriteOptions::default()
    };
    for (g, value) in groups.iter().enumerate() {
        let path = format!("options.layer_groups[{g}]");
        let group = Input::new(
            value,
            &path,
            &["state", "write_layer", "scale", "protect", "name", "layers"],
        )?;
        let layers = array(group.get("layers")?, &format!("{path}.layers"))?;
        if layers.len() != 16 {
            return Err(group.error("layers", "expected exactly 16 layers"));
        }
        let target = &mut out.layer_groups[g];
        *target = LayerGroupHeader {
            state: group.integer("state")?,
            write_layer: group.integer("write_layer")?,
            scale: group.number("scale")?,
            protect: group.integer("protect")?,
            name: group.string("name")?,
            ..LayerGroupHeader::default()
        };
        for (l, value) in layers.iter().enumerate() {
            let layer = Input::new(
                value,
                &format!("{path}.layers[{l}]"),
                &["state", "protect", "name"],
            )?;
            target.layers[l] = LayerHeader {
                state: layer.integer("state")?,
                protect: layer.integer("protect")?,
                name: layer.string("name")?,
            };
        }
    }
    out.palette = input.optional("palette")?;
    if let Some(value) = input.fields.get("line_types").filter(|v| !v.is_null()) {
        let mut value = value.clone();
        for field in ["standard", "double_length"] {
            if let Some(patterns) = value.get_mut(field).and_then(Value::as_array_mut) {
                for pattern in patterns {
                    if let Some(p) = pattern.as_object_mut() {
                        p.remove("runs");
                        p.remove("segments_mm");
                    }
                }
            }
        }
        out.line_types = Some(
            serde_json::from_value(value).map_err(|e| input.error("line_types", &e.to_string()))?,
        );
    }
    out.text_presets = input.optional("text_presets")?;
    out.pens = input.optional("pens")?;
    // Flattened SXF patterns need an explicit key check (serde flatten does not
    // reliably reject unknown keys across a flattened nested structure).
    if let Some(slots) = value.pointer("/line_types/sxf").and_then(Value::as_array) {
        for (i, slot) in slots.iter().enumerate() {
            Input::new(
                slot,
                &format!("options.line_types.sxf[{i}]"),
                &[
                    "number",
                    "pattern",
                    "unit_dots",
                    "pitch",
                    "printer_pitch",
                    "name",
                    "segments_mm",
                ],
            )?;
        }
    }
    Ok(out)
}

pub(super) fn entity(value: &Value, path: &str) -> Result<Entity, JwwWriteError> {
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&format!("{path}.type"), "expected an entity type"))?
        .to_owned();
    let fields: &[&str] = match kind.as_str() {
        "LINE" => &["start_x", "start_y", "end_x", "end_y"],
        "CIRCLE" | "ARC" => &[
            "center_x",
            "center_y",
            "radius",
            "start_angle",
            "arc_angle",
            "tilt_angle",
            "flatness",
            "is_full_circle",
        ],
        "POINT" => &["x", "y", "is_temporary", "code", "angle", "scale"],
        "TEXT" => &[
            "start_x",
            "start_y",
            "end_x",
            "end_y",
            "text_type",
            "size_x",
            "size_y",
            "spacing",
            "angle",
            "font_name",
            "content",
        ],
        "SOLID" => &[
            "point1_x", "point1_y", "point2_x", "point2_y", "point3_x", "point3_y", "point4_x",
            "point4_y", "color",
        ],
        "CIRCLE_SOLID" => &[
            "center_x",
            "center_y",
            "radius",
            "flatness",
            "tilt_angle",
            "start_angle",
            "arc_angle",
            "solid_mode",
            "color",
        ],
        "BLOCK" => &[
            "ref_x",
            "ref_y",
            "scale_x",
            "scale_y",
            "rotation",
            "def_number",
        ],
        "DIMENSION" => &["line", "text", "sxf_mode", "aux_lines", "aux_points"],
        _ => return Err(invalid(&format!("{path}.type"), "unsupported entity type")),
    };
    let mut allowed = vec!["type", "base"];
    allowed.extend_from_slice(fields);
    let input = Input::new(value, path, &allowed)?;
    // A getter must not change the discriminator between inspection and decode.
    if input.string("type")? != kind {
        return Err(input.error("type", "entity type changed during validation"));
    }
    let base = base(input.get("base")?, &format!("{path}.base"))?;
    Ok(match kind.as_str() {
        "LINE" => Entity::Line(Line {
            base,
            start_x: input.number("start_x")?,
            start_y: input.number("start_y")?,
            end_x: input.number("end_x")?,
            end_y: input.number("end_y")?,
        }),
        "CIRCLE" | "ARC" => {
            let full = input.boolean("is_full_circle")?;
            if full != (kind == "CIRCLE") {
                return Err(input.error("is_full_circle", "must agree with entity type"));
            }
            Entity::Arc(Arc {
                base,
                center_x: input.number("center_x")?,
                center_y: input.number("center_y")?,
                radius: input.number("radius")?,
                start_angle: input.number("start_angle")?,
                arc_angle: input.number("arc_angle")?,
                tilt_angle: input.number("tilt_angle")?,
                flatness: input.number("flatness")?,
                is_full_circle: full,
            })
        }
        "POINT" => Entity::Point(Point {
            base,
            x: input.number("x")?,
            y: input.number("y")?,
            is_temporary: input.boolean("is_temporary")?,
            code: input.integer("code")?,
            angle: input.number("angle")?,
            scale: input.number("scale")?,
        }),
        "TEXT" => Entity::Text(Text {
            base,
            start_x: input.number("start_x")?,
            start_y: input.number("start_y")?,
            end_x: input.number("end_x")?,
            end_y: input.number("end_y")?,
            text_type: input.integer("text_type")?,
            size_x: input.number("size_x")?,
            size_y: input.number("size_y")?,
            spacing: input.number("spacing")?,
            angle: input.number("angle")?,
            font_name: input.string("font_name")?,
            content: input.string("content")?,
        }),
        "SOLID" => Entity::Solid(Solid {
            base,
            point1_x: input.number("point1_x")?,
            point1_y: input.number("point1_y")?,
            point2_x: input.number("point2_x")?,
            point2_y: input.number("point2_y")?,
            point3_x: input.number("point3_x")?,
            point3_y: input.number("point3_y")?,
            point4_x: input.number("point4_x")?,
            point4_y: input.number("point4_y")?,
            color: input.optional("color")?,
        }),
        "CIRCLE_SOLID" => Entity::CircleSolid(CircleSolid {
            base,
            center_x: input.number("center_x")?,
            center_y: input.number("center_y")?,
            radius: input.number("radius")?,
            flatness: input.number("flatness")?,
            tilt_angle: input.number("tilt_angle")?,
            start_angle: input.number("start_angle")?,
            arc_angle: input.number("arc_angle")?,
            solid_mode: input.number("solid_mode")?,
            color: input.optional("color")?,
        }),
        "BLOCK" => Entity::Block(Block {
            base,
            ref_x: input.number("ref_x")?,
            ref_y: input.number("ref_y")?,
            scale_x: input.number("scale_x")?,
            scale_y: input.number("scale_y")?,
            rotation: input.number("rotation")?,
            def_number: input.integer("def_number")?,
        }),
        "DIMENSION" => {
            let line = member(input.get("line")?, "LINE", &format!("{path}.line"), base)?;
            let text = member(input.get("text")?, "TEXT", &format!("{path}.text"), base)?;
            let aux_lines = array(input.get("aux_lines")?, &format!("{path}.aux_lines"))?
                .iter()
                .enumerate()
                .map(
                    |(i, v)| match member(v, "LINE", &format!("{path}.aux_lines[{i}]"), base)? {
                        Entity::Line(v) => Ok(v),
                        _ => unreachable!(),
                    },
                )
                .collect::<Result<_, _>>()?;
            let aux_points = array(input.get("aux_points")?, &format!("{path}.aux_points"))?
                .iter()
                .enumerate()
                .map(
                    |(i, v)| match member(v, "POINT", &format!("{path}.aux_points[{i}]"), base)? {
                        Entity::Point(v) => Ok(v),
                        _ => unreachable!(),
                    },
                )
                .collect::<Result<_, _>>()?;
            let Entity::Line(line) = line else {
                unreachable!()
            };
            let Entity::Text(text) = text else {
                unreachable!()
            };
            Entity::Dimension(Dimension {
                base,
                line,
                text,
                sxf_mode: Some(input.integer("sxf_mode")?),
                aux_lines,
                aux_points,
            })
        }
        _ => unreachable!(),
    })
}

fn base(value: &Value, path: &str) -> Result<EntityBase, JwwWriteError> {
    let b = Input::new(
        value,
        path,
        &[
            "group",
            "pen_style",
            "pen_color",
            "pen_width",
            "layer",
            "layer_group",
            "flag",
        ],
    )?;
    Ok(EntityBase {
        group: b.integer("group")?,
        pen_style: b.integer("pen_style")?,
        pen_color: b.integer("pen_color")?,
        pen_width: b.integer("pen_width")?,
        layer: b.integer("layer")?,
        layer_group: b.integer("layer_group")?,
        flag: b.integer("flag")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn line(pen: u16, color: Option<u32>) -> Value {
        let mut line = json!({
            "type": "LINE",
            "base": {
                "group": 0, "pen_style": 1, "pen_color": pen, "pen_width": 0,
                "layer": 0, "layer_group": 0, "flag": 0,
            },
            "start_x": 0.0, "start_y": 0.0, "end_x": 1.0, "end_y": 1.0,
        });
        if let Some(color) = color {
            line["color"] = json!(color);
        }
        line
    }

    #[test]
    fn encoder_writes_what_to_jww_bytes_writes() {
        let mut doc = new_document();
        doc["block_defs"] = json!([{
            "base": line(1, None)["base"], "number": 0, "is_referenced": true,
            "name": "B", "entities": [line(1, Some(0x2A3B4C))],
        }]);
        doc["entities"] = json!([
            line(1, Some(0x1D2E3F)),
            line(118, None),
            {
                "type": "BLOCK", "base": line(1, None)["base"], "ref_x": 5.0, "ref_y": 6.0,
                "scale_x": 1.0, "scale_y": 1.0, "rotation": 0.0, "def_number": 0,
            },
            line(1, Some(0x2A3B4C)),
        ]);
        let expected = from_value(doc.clone()).unwrap();
        // The block definition takes the first free slot; 118 is used further down.
        let pens: Vec<u16> = expected
            .entities
            .iter()
            .filter_map(|e| match e {
                Entity::Line(line) => Some(line.base.pen_color),
                _ => None,
            })
            .collect();
        assert_eq!(pens, [119, 118, 117]);

        let mut used = HashSet::new();
        scan_pens(&doc, &mut used);
        let mut encoder = DocumentEncoder::new(doc["options"].clone(), used).unwrap();
        for value in doc["block_defs"].as_array().unwrap() {
            encoder.push_block_def(value.clone()).unwrap();
        }
        for value in doc["entities"].as_array().unwrap() {
            encoder.push_entity(value.clone()).unwrap();
        }
        assert_eq!(
            encoder.finish().unwrap(),
            crate::to_jww_bytes(&expected).unwrap()
        );
    }

    #[test]
    fn encoder_takes_the_lists_in_from_value_order() {
        let options = new_document()["options"].clone();
        let mut encoder = DocumentEncoder::new(options, HashSet::new()).unwrap();
        encoder.push_entity(line(1, None)).unwrap();
        assert!(encoder.push_block_def(json!({})).is_err());
    }

    /// A dimension whose line, value text and second auxiliary point have RGB
    /// colors: `from_value` gives the points theirs before the line and text.
    fn dimension(colors: [u32; 3]) -> Value {
        let base = |flag: u32| {
            json!({
                "group": 0, "pen_style": 1, "pen_color": 1, "pen_width": 0,
                "layer": 0, "layer_group": 0, "flag": flag,
            })
        };
        let mut line = line(1, Some(colors[0]));
        line.as_object_mut().unwrap().remove("type");
        line["base"]["flag"] = json!(0x2000);
        let point = json!({
            "base": base(0x40), "x": 0.0, "y": 0.0, "is_temporary": false,
            "code": 0, "angle": 0.0, "scale": 0.0,
        });
        let mut points = vec![point.clone(), point.clone(), point.clone(), point];
        points[1]["color"] = json!(colors[2]);
        let mut aux = line.clone();
        aux.as_object_mut().unwrap().remove("color");
        json!({
            "type": "DIMENSION", "base": base(0x2000), "line": line,
            "text": {
                "base": base(0x4010), "start_x": 0.0, "start_y": 0.0, "end_x": 5.0,
                "end_y": 0.0, "text_type": 0, "size_x": 2.5, "size_y": 2.5,
                "spacing": 0.0, "angle": 0.0, "font_name": "MS Gothic", "content": "10",
                "color": colors[1],
            },
            "sxf_mode": 0, "aux_lines": [aux.clone(), aux], "aux_points": points,
        })
    }

    fn stream_bytes(doc: &Value) -> Result<Vec<u8>, JwwWriteError> {
        let mut stream = EntityStream::new(&doc["options"])?;
        for value in doc["entities"].as_array().unwrap() {
            stream.push_entity(value.clone())?;
        }
        stream.finish(
            doc["options"].clone(),
            doc["block_defs"].as_array().cloned().unwrap_or_default(),
            doc["images"].as_array().cloned().unwrap_or_default(),
        )
    }

    #[test]
    fn stream_writes_what_to_jww_bytes_writes() {
        let mut doc = new_document();
        // The block definition is known only at the end, but its color still
        // takes the first free slot; 118 is used by an entity further down.
        doc["block_defs"] = json!([{
            "base": line(1, None)["base"], "number": 0, "is_referenced": true,
            "name": "B", "entities": [line(1, Some(0x2A3B4C)), line(1, Some(0x0A0B0C))],
        }]);
        let mut solid = json!({
            "type": "SOLID", "base": line(10, None)["base"],
            "point1_x": 0.0, "point1_y": 0.0, "point2_x": 1.0, "point2_y": 1.0,
            "point3_x": 0.0, "point3_y": 1.0, "point4_x": 1.0, "point4_y": 0.0,
            "color": 0x445566,
        });
        solid["base"]["pen_style"] = json!(1);
        doc["entities"] = json!([
            line(1, Some(0x1D2E3F)),
            dimension([0x0A0B0C, 0x1D2E3F, 0x3E3F40]),
            line(118, None),
            {
                "type": "BLOCK", "base": line(1, None)["base"], "ref_x": 5.0, "ref_y": 6.0,
                "scale_x": 1.0, "scale_y": 1.0, "rotation": 0.0, "def_number": 0,
            },
            solid,
            line(1, Some(0x2A3B4C)),
            line(3, Some(0x000000)),
        ]);
        let expected = crate::to_jww_bytes(&from_value(doc.clone()).unwrap()).unwrap();
        assert_eq!(stream_bytes(&doc).unwrap(), expected);
    }

    #[test]
    fn stream_skips_what_it_refuses_and_finish_can_be_retried() {
        let mut doc = new_document();
        let mut refused = line(1, Some(0x123456));
        refused["base"]["layer"] = json!(16);
        let mut stream = EntityStream::new(&doc["options"]).unwrap();
        stream.push_entity(line(1, Some(0x1D2E3F))).unwrap();
        let error = stream.push_entity(refused).unwrap_err().to_string();
        assert!(error.contains("entities[1].base.layer"), "{error}");
        assert!(stream.push_entity(json!({"type": "LINE"})).is_err());
        stream.push_entity(line(1, Some(0x2A3B4C))).unwrap();
        assert_eq!(stream.next_index(), 4);

        let mut bad = line(1, Some(0x0A0B0C));
        bad["base"]["pen_color"] = json!(0);
        bad.as_object_mut().unwrap().remove("color");
        let definition = |entities: Value| {
            json!({
                "base": line(1, None)["base"], "number": 0, "is_referenced": false,
                "name": "B", "entities": entities,
            })
        };
        let error = stream
            .finish(
                doc["options"].clone(),
                vec![definition(json!([line(1, Some(0x0A0B0C)), bad]))],
                Vec::new(),
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains("block_defs[0].entities[1]"), "{error}");
        let bytes = stream
            .finish(
                doc["options"].clone(),
                vec![definition(json!([line(1, Some(0x0A0B0C))]))],
                Vec::new(),
            )
            .unwrap();

        doc["entities"] = json!([line(1, Some(0x1D2E3F)), line(1, Some(0x2A3B4C))]);
        doc["block_defs"] = json!([definition(json!([line(1, Some(0x0A0B0C))]))]);
        assert_eq!(
            bytes,
            crate::to_jww_bytes(&from_value(doc.clone()).unwrap()).unwrap()
        );
        assert!(stream.is_closed());
        assert!(stream.push_entity(line(1, None)).is_err());
    }
}
