//! Strict JS adapter; own keys and Unicode scalars are checked before decoding.
use js_sys::{Array, Reflect};
use serde_json::Value;
use wasm_bindgen::prelude::*;

fn input(value: JsValue, path: &str, depth: usize) -> Result<Value, JsValue> {
    let invalid = |message: &str| super::js_error(&format!("{path}: {message}"));
    if depth > 64 {
        return Err(invalid("input nesting exceeds 64"));
    }
    if value.is_null() {
        return Ok(Value::Null);
    }
    if let Some(v) = value.as_bool() {
        return Ok(Value::Bool(v));
    }
    if let Some(v) = value.as_f64() {
        if v.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&v) {
            return Ok(Value::from(v as u32));
        }
        return serde_json::Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| invalid("expected a finite number"));
    }
    if let Some(v) = value.as_string() {
        if JsValue::from_str(&v) != value {
            return Err(invalid("unpaired UTF-16 surrogate"));
        }
        return Ok(Value::String(v));
    }
    if Array::is_array(&value) {
        let array: Array = value.unchecked_into();
        return array
            .iter()
            .enumerate()
            .map(|(i, v)| input(v, &format!("{path}[{i}]"), depth + 1))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    if value.is_object() {
        let mut out = serde_json::Map::new();
        for key in Reflect::own_keys(&value)? {
            let name = key
                .as_string()
                .ok_or_else(|| invalid("expected string keys"))?;
            out.insert(
                name.clone(),
                input(
                    Reflect::get(&value, &key)?,
                    &format!("{path}.{name}"),
                    depth + 1,
                )?,
            );
        }
        return Ok(Value::Object(out));
    }
    Err(invalid("expected an object, array, string, bool or number"))
}

#[wasm_bindgen(js_name = newJwwDocument)]
pub fn new_jww_document() -> Result<JsValue, JsValue> {
    super::to_js_value(&ezjww_core::writer::input::new_document())
}

#[wasm_bindgen(js_name = toJwwBytes)]
pub fn to_jww_bytes(document: JsValue) -> Result<Vec<u8>, JsValue> {
    let doc = ezjww_core::writer::input::from_value(input(document, "document", 0)?)
        .map_err(|e| super::js_error(&e.to_string()))?;
    ezjww_core::to_jww_bytes(&doc).map_err(|e| super::js_error(&e.to_string()))
}

#[wasm_bindgen(js_name = toWriteDocument)]
pub fn to_write_document(
    document: JsValue,
    skip_unsupported: Option<bool>,
) -> Result<JsValue, JsValue> {
    let report = ezjww_core::writer::to_write_document(
        input(document, "document", 0)?,
        skip_unsupported.unwrap_or(false),
    )
    .map_err(|e| super::js_error(&e.to_string()))?;
    super::to_js_value(&report)
}
