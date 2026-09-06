use serde::Serialize;
use wasm_bindgen::prelude::*;

use ezjww_core::{
    convert_document_with_options, document_to_string_with_version, is_jww_signature,
    jww_document_to_dto_with_diagnostics, parse_document, parse_document_with_diagnostics,
    parse_header, ConvertOptions, JwwError,
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
    let parsed = parse_document_with_diagnostics(data).map_err(to_js_error)?;
    to_js_value(&jww_document_to_dto_with_diagnostics(
        &parsed.document,
        &parsed.diagnostics,
    ))
}

#[wasm_bindgen(js_name = isJwcFile)]
pub fn is_jwc_file(data: &[u8]) -> bool {
    ezjww_core::is_jwc_signature(data)
}

#[wasm_bindgen(js_name = detectFileFormat)]
pub fn detect_file_format(data: &[u8]) -> Result<JsValue, JsValue> {
    to_js_value(&ezjww_core::detect_format(data))
}

#[wasm_bindgen(js_name = readJwcHeader)]
pub fn read_jwc_header(data: &[u8]) -> Result<JsValue, JsValue> {
    let header = ezjww_core::parse_jwc_header(data).map_err(|e| js_error(&e.to_string()))?;
    to_js_value(&header)
}

#[wasm_bindgen(js_name = readJwcDocument)]
pub fn read_jwc_document(data: &[u8]) -> Result<JsValue, JsValue> {
    let document = ezjww_core::parse_jwc_document(data).map_err(|e| js_error(&e.to_string()))?;
    to_js_value(&ezjww_core::schema::jwc_document_to_dto(&document))
}

#[wasm_bindgen(js_name = readCadDocument)]
pub fn read_cad_document(data: &[u8]) -> Result<JsValue, JsValue> {
    let document = ezjww_core::parse_cad_document(data).map_err(|e| js_error(&e.to_string()))?;
    to_js_value(&ezjww_core::schema::cad_document_to_dto(&document))
}

#[wasm_bindgen(js_name = readDxfDocument)]
pub fn read_dxf_document(
    data: &[u8],
    explode_inserts: bool,
    max_block_nesting: usize,
    jwc_coordinates: Option<String>,
) -> Result<JsValue, JsValue> {
    let options = convert_options(explode_inserts, max_block_nesting)?;
    if is_jwc_file(data) {
        let converted = convert_jwc(data, options, jwc_coordinates.as_deref())?;
        return to_js_value(&ezjww_core::schema::jwc_dxf_document_to_dto(&converted));
    }
    let document = parse_document(data).map_err(to_js_error)?;
    let dxf_document = convert_document_with_options(&document, options);
    to_js_value(&dxf_document)
}

#[wasm_bindgen(js_name = readDxfString)]
pub fn read_dxf_string(
    data: &[u8],
    explode_inserts: bool,
    max_block_nesting: usize,
    jwc_coordinates: Option<String>,
    target_version: Option<String>,
) -> Result<String, JsValue> {
    let options = convert_options(explode_inserts, max_block_nesting)?;
    let target = match target_version
        .as_deref()
        .unwrap_or("AC1015")
        .trim()
        .to_ascii_uppercase()
        .as_str()
    {
        "AC1015" => ezjww_core::DxfTargetVersion::Ac1015,
        "AC1024" => ezjww_core::DxfTargetVersion::Ac1024,
        _ => return Err(js_error("target_version must be AC1015 or AC1024")),
    };
    if is_jwc_file(data) {
        return Ok(convert_jwc(data, options, jwc_coordinates.as_deref())?.to_dxf_string(target));
    }
    let document = parse_document(data).map_err(to_js_error)?;
    let dxf_document = convert_document_with_options(&document, options);
    Ok(document_to_string_with_version(&dxf_document, target))
}

fn convert_jwc(
    data: &[u8],
    options: ConvertOptions,
    coordinates: Option<&str>,
) -> Result<ezjww_core::jwc::JwcDxfConversion, JsValue> {
    let doc = ezjww_core::parse_jwc_document(data).map_err(|e| js_error(&e.to_string()))?;
    let coordinates = coordinates
        .unwrap_or("paper_millimeters")
        .parse::<ezjww_core::jwc::JwcCoordinateSpace>()
        .map_err(|e| js_error(&e.to_string()))?;
    ezjww_core::jwc::convert_jwc_document(
        &doc,
        ezjww_core::jwc::JwcConvertOptions {
            coordinates,
            dxf: options,
        },
    )
    .map_err(|e| js_error(&e.to_string()))
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
