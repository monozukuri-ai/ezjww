//! Strict JS input for the new-document writer. Keep the shape aligned with
//! the Python writer; reader documents are deliberately not writable inputs.
use std::collections::BTreeMap;

use ezjww_core::{
    Arc, Entity, EntityBase, JwwWriteDocument, JwwWriteOptions, LayerGroupHeader, LayerHeader,
    Line, Point, Text,
};
use js_sys::{Array, Reflect};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = newJwwDocument)]
pub fn new_jww_document() -> Result<JsValue, JsValue> {
    // Serialize the actual Rust defaults, including blank names. Header readers
    // synthesize display names which must not become the writer's defaults.
    #[derive(Serialize)]
    struct Options<'a> {
        version: u32,
        memo: &'a str,
        paper_size: u32,
        write_layer_group: u32,
        layer_groups: &'a [LayerGroupHeader; 16],
    }
    #[derive(Serialize)]
    struct Document<'a> {
        options: Options<'a>,
        entities: &'a [Entity],
    }
    let doc = JwwWriteDocument::default();
    super::to_js_value(&Document {
        options: Options {
            version: doc.options.version,
            memo: &doc.options.memo,
            paper_size: doc.options.paper_size,
            write_layer_group: doc.options.write_layer_group,
            layer_groups: &doc.options.layer_groups,
        },
        entities: &doc.entities,
    })
}

#[wasm_bindgen(js_name = toJwwBytes)]
pub fn to_jww_bytes(document: JsValue) -> Result<Vec<u8>, JsValue> {
    let input = Input::new(document, "document", &["options", "entities"])?;
    let options = options(input.get("options")?)?;
    let entities = array(input.get("entities")?, "entities")?
        .iter()
        .enumerate()
        .map(|(i, value)| entity(value, &format!("entities[{i}]")))
        .collect::<Result<_, _>>()?;
    ezjww_core::to_jww_bytes(&JwwWriteDocument { options, entities })
        .map_err(|e| super::js_error(&e.to_string()))
}

fn invalid(path: &str, message: &str) -> JsValue {
    super::js_error(&format!("{path}: {message}"))
}

// Read own keys explicitly: serde-wasm-bindgen's struct deserializer only visits
// declared fields, so deny_unknown_fields alone would silently discard extras.
struct Input {
    fields: BTreeMap<String, JsValue>,
    path: String,
}

impl Input {
    fn new(value: JsValue, path: &str, allowed: &[&str]) -> Result<Self, JsValue> {
        if !value.is_object() || Array::is_array(&value) {
            return Err(invalid(path, "expected an object"));
        }
        let mut fields = BTreeMap::new();
        for key in Reflect::own_keys(&value)? {
            let name = key
                .as_string()
                .ok_or_else(|| invalid(path, "expected string keys"))?;
            if !allowed.contains(&name.as_str()) {
                return Err(invalid(&format!("{path}.{name}"), "unsupported field"));
            }
            fields.insert(name, Reflect::get(&value, &key)?);
        }
        for key in allowed {
            if !fields.contains_key(*key) {
                return Err(invalid(&format!("{path}.{key}"), "missing field"));
            }
        }
        Ok(Self {
            fields,
            path: path.into(),
        })
    }

    fn get(&self, key: &str) -> Result<JsValue, JsValue> {
        self.fields
            .get(key)
            .cloned()
            .ok_or_else(|| self.error(key, "missing field"))
    }

    fn error(&self, key: &str, message: &str) -> JsValue {
        invalid(&format!("{}.{key}", self.path), message)
    }

    fn number(&self, key: &str) -> Result<f64, JsValue> {
        self.get(key)?
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or_else(|| self.error(key, "expected a finite number"))
    }

    fn integer<T: TryFrom<u32>>(&self, key: &str) -> Result<T, JsValue> {
        let value = self.number(key)?;
        if value.fract() != 0.0 || value < 0.0 || value > u32::MAX as f64 {
            return Err(self.error(key, "expected an unsigned integer in range"));
        }
        T::try_from(value as u32).map_err(|_| self.error(key, "integer exceeds field range"))
    }

    fn boolean(&self, key: &str) -> Result<bool, JsValue> {
        self.get(key)?
            .as_bool()
            .ok_or_else(|| self.error(key, "expected a boolean"))
    }

    fn string(&self, key: &str) -> Result<String, JsValue> {
        let value = self.get(key)?;
        let text = value
            .as_string()
            .ok_or_else(|| self.error(key, "expected a string"))?;
        // JS can contain unpaired UTF-16 surrogates. Do not silently replace
        // them when crossing into Rust's Unicode scalar string representation.
        if JsValue::from_str(&text) != value {
            return Err(self.error(key, "unpaired UTF-16 surrogate"));
        }
        Ok(text)
    }
}

fn array(value: JsValue, path: &str) -> Result<Array, JsValue> {
    if !Array::is_array(&value) {
        return Err(invalid(path, "expected an array"));
    }
    Ok(value.unchecked_into())
}

fn options(value: JsValue) -> Result<JwwWriteOptions, JsValue> {
    let input = Input::new(
        value,
        "options",
        &[
            "version",
            "memo",
            "paper_size",
            "write_layer_group",
            "layer_groups",
        ],
    )?;
    let groups = array(input.get("layer_groups")?, "options.layer_groups")?;
    if groups.length() != 16 {
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
        if layers.length() != 16 {
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
    Ok(out)
}

fn entity(value: JsValue, path: &str) -> Result<Entity, JsValue> {
    if !value.is_object() || Array::is_array(&value) {
        return Err(invalid(path, "expected an object"));
    }
    let kind = Reflect::get(&value, &JsValue::from_str("type"))?
        .as_string()
        .ok_or_else(|| invalid(&format!("{path}.type"), "expected an entity type"))?;
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
        _ => return Err(invalid(&format!("{path}.type"), "unsupported entity type")),
    };
    let mut allowed = vec!["type", "base"];
    allowed.extend_from_slice(fields);
    let input = Input::new(value, path, &allowed)?;
    // A getter must not change the discriminator between inspection and decode.
    if input.string("type")? != kind {
        return Err(input.error("type", "entity type changed during validation"));
    }
    let b = Input::new(
        input.get("base")?,
        &format!("{path}.base"),
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
    let base = EntityBase {
        group: b.integer("group")?,
        pen_style: b.integer("pen_style")?,
        pen_color: b.integer("pen_color")?,
        pen_width: b.integer("pen_width")?,
        layer: b.integer("layer")?,
        layer_group: b.integer("layer_group")?,
        flag: b.integer("flag")?,
    };
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
        _ => unreachable!(),
    })
}
