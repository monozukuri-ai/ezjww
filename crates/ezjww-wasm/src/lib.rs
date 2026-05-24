use serde::Serialize;
use wasm_bindgen::prelude::*;

use ezjww_core::{
    convert_document_with_options, document_to_string, is_jww_signature, jww_document_to_dto,
    parse_document, parse_header, ConvertOptions, JwwError,
};

#[wasm_bindgen(js_name = isJwwFile)]
pub fn is_jww_file(data: &[u8]) -> bool {
    is_jww_signature(data)
}

#[wasm_bindgen(js_name = readHeader)]
pub fn read_header(data: &[u8]) -> Result<JsValue, JsValue> {
    parse_header(data)
        .map_err(to_js_error)
        .and_then(|header| to_js_value(&header))
}

#[wasm_bindgen(js_name = readDocument)]
pub fn read_document(data: &[u8]) -> Result<JsValue, JsValue> {
    let document = parse_document(data).map_err(to_js_error)?;
    to_js_value(&jww_document_to_dto(&document))
}

#[wasm_bindgen(js_name = readDxfDocument)]
pub fn read_dxf_document(
    data: &[u8],
    explode_inserts: bool,
    max_block_nesting: usize,
) -> Result<JsValue, JsValue> {
    let options = convert_options(explode_inserts, max_block_nesting)?;
    let document = parse_document(data).map_err(to_js_error)?;
    let dxf_document = convert_document_with_options(&document, options);
    to_js_value(&dxf_document)
}

#[wasm_bindgen(js_name = readDxfString)]
pub fn read_dxf_string(
    data: &[u8],
    explode_inserts: bool,
    max_block_nesting: usize,
) -> Result<String, JsValue> {
    let options = convert_options(explode_inserts, max_block_nesting)?;
    let document = parse_document(data).map_err(to_js_error)?;
    let dxf_document = convert_document_with_options(&document, options);
    Ok(document_to_string(&dxf_document))
}

fn convert_options(
    explode_inserts: bool,
    max_block_nesting: usize,
) -> Result<ConvertOptions, JsValue> {
    validate_convert_options(explode_inserts, max_block_nesting).map_err(js_error)
}

fn validate_convert_options(
    explode_inserts: bool,
    max_block_nesting: usize,
) -> Result<ConvertOptions, &'static str> {
    if max_block_nesting == 0 {
        return Err("max_block_nesting must be >= 1");
    }
    Ok(ConvertOptions {
        explode_inserts,
        max_block_nesting,
    })
}

fn to_js_error(err: JwwError) -> JsValue {
    js_error(&err.to_string())
}

fn to_js_value_error(err: serde_wasm_bindgen::Error) -> JsValue {
    js_error(&err.to_string())
}

fn to_js_value<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(to_js_value_error)
}

fn js_error(message: &str) -> JsValue {
    JsValue::from_str(message)
}

#[cfg(test)]
mod tests {
    use super::validate_convert_options;

    #[test]
    fn convert_options_rejects_zero_nesting() {
        assert!(validate_convert_options(false, 0).is_err());
    }

    #[test]
    fn convert_options_accepts_positive_nesting() {
        let options = validate_convert_options(true, 16).unwrap();
        assert!(options.explode_inserts);
        assert_eq!(options.max_block_nesting, 16);
    }
}
