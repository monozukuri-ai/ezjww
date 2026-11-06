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

/// `from_value` for a document whose lists arrive one element at a time, so the
/// caller never holds the whole input as one `Value` tree (a drawing of 840,000
/// entities took 1.6 GB as a tree).
///
/// Push the `block_defs`, then the `entities`, then the `images`, each list in
/// order: the order in which `from_value` allocates the pens of RGB colors. A
/// document that `from_value` accepts gives the same `JwwWriteDocument`. Each
/// element is checked when it arrives, so of several errors in a document
/// another one than the one `from_value` reports may come first.
pub struct DocumentBuilder {
    options: Value,
    colors: ColorAllocator,
    block_defs: Vec<BlockDef>,
    entities: Vec<Entity>,
    images: Vec<EmbeddedImage>,
    /// 0 = block definitions, 1 = entities, 2 = images: the list being pushed.
    list: u8,
}

impl DocumentBuilder {
    /// `options` is the document's `options`; `used` holds the pens of every
    /// element of the document's lists (`scan_pens`), which allocated pens avoid.
    pub fn new(options: Value, used: HashSet<u16>) -> Result<Self, JwwWriteError> {
        let colors = ColorAllocator::new(used, options.get("palette"))?;
        Ok(Self {
            options,
            colors,
            block_defs: Vec::new(),
            entities: Vec::new(),
            images: Vec::new(),
            list: 0,
        })
    }

    pub fn push_block_def(&mut self, mut value: Value) -> Result<(), JwwWriteError> {
        self.enter(0, "block_defs")?;
        let i = self.block_defs.len();
        let at = At::Key(&At::Root("document"), "block_defs");
        self.colors.apply(&mut value, &At::Index(&at, i))?;
        self.block_defs.push(block_def(&value, i)?);
        Ok(())
    }

    pub fn push_entity(&mut self, mut value: Value) -> Result<(), JwwWriteError> {
        self.enter(1, "entities")?;
        let i = self.entities.len();
        let at = At::Key(&At::Root("document"), "entities");
        self.colors.apply(&mut value, &At::Index(&at, i))?;
        self.entities
            .push(entity(&value, &format!("entities[{i}]"))?);
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

    pub fn finish(mut self) -> Result<JwwWriteDocument, JwwWriteError> {
        if let Some(palette) = self.colors.allocated_palette() {
            let options = self
                .options
                .as_object_mut()
                .ok_or_else(|| invalid("options", "expected an object"))?;
            options.insert("palette".into(), palette);
        }
        Ok(JwwWriteDocument {
            options: options(&self.options)?,
            entities: self.entities,
            block_defs: self.block_defs,
            images: self.images,
        })
    }

    fn enter(&mut self, list: u8, name: &str) -> Result<(), JwwWriteError> {
        if list < self.list {
            return Err(invalid(name, "push block_defs, then entities, then images"));
        }
        self.list = list;
        Ok(())
    }
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
/// that `from_value` and `DocumentBuilder` never give another color.
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
        let used = &mut self.used;
        let palette = &mut self.palette;
        if let Some(obj) = v.as_object_mut() {
            if let Some(color) = obj.get("color").cloned().filter(|_| {
                obj.contains_key("base")
                    && matches!(
                        obj.get("type").and_then(Value::as_str),
                        None | Some(
                            "LINE" | "ARC" | "CIRCLE" | "POINT" | "TEXT" | "SOLID" | "CIRCLE_SOLID"
                        )
                    )
            }) {
                obj.remove("color");
                let n = color.as_u64().filter(|v| *v <= 0xffffff).ok_or_else(|| {
                    invalid(
                        &format!("{}.color", at.render()),
                        "expected COLORREF 0..=0xffffff",
                    )
                });
                let solid = matches!(
                    obj.get("type").and_then(Value::as_str),
                    Some("SOLID" | "CIRCLE_SOLID")
                );
                if solid {
                    obj.insert("color".into(), color);
                } else {
                    let color = n? as u32;
                    let existing = (1..=9)
                        .chain(101..=356)
                        .find(|&p| palette.screen_color(p) == Some(color));
                    let pen = if let Some(p) = existing {
                        p
                    } else {
                        let p = (117..=356).find(|p| !used.contains(p)).ok_or_else(|| {
                            invalid(
                                &format!("{}.color", at.render()),
                                "no unused SXF palette slots; cannot approximate RGB",
                            )
                        })?;
                        palette.extended_colors.as_mut().unwrap()[(p - 100) as usize] = color;
                        self.changed = true;
                        p
                    };
                    used.insert(pen);
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
    fn builder_decodes_like_from_value() {
        let mut doc = new_document();
        doc["block_defs"] = json!([{
            "base": line(1, None)["base"], "number": 0, "is_referenced": false,
            "name": "B", "entities": [line(1, Some(0x2A3B4C))],
        }]);
        doc["entities"] = json!([
            line(1, Some(0x1D2E3F)),
            line(118, None),
            line(1, Some(0x2A3B4C)),
        ]);
        let expected = from_value(doc.clone()).unwrap();

        let mut used = HashSet::new();
        scan_pens(&doc, &mut used);
        let mut builder = DocumentBuilder::new(doc["options"].clone(), used).unwrap();
        for value in doc["block_defs"].as_array().unwrap() {
            builder.push_block_def(value.clone()).unwrap();
        }
        for value in doc["entities"].as_array().unwrap() {
            builder.push_entity(value.clone()).unwrap();
        }
        let built = builder.finish().unwrap();
        assert_eq!(built, expected);
        // The block definition takes the first free slot; 118 is used further down.
        let pens: Vec<u16> = built
            .entities
            .iter()
            .map(|e| match e {
                Entity::Line(line) => line.base.pen_color,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(pens, [119, 118, 117]);
    }

    #[test]
    fn builder_takes_the_lists_in_from_value_order() {
        let options = new_document()["options"].clone();
        let mut builder = DocumentBuilder::new(options, HashSet::new()).unwrap();
        builder.push_entity(line(1, None)).unwrap();
        assert!(builder.push_block_def(json!({})).is_err());
    }
}
