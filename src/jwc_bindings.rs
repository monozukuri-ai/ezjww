use ezjww_core::jwc::{
    convert_jwc_document, JwcConversionError, JwcConvertOptions, JwcDxfConversion,
};
use ezjww_core::schema::{jwc_document_to_dto, jwc_dxf_document_to_dto};
use ezjww_core::{CadFormat, ConvertOptions, DxfTargetVersion, JwcError};
use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde::Serialize;
use serde_json::Value;

pub(crate) fn to_python<T: Serialize>(py: Python<'_>, value: &T) -> PyResult<PyObject> {
    // Value widens source f32 to f64, matching the WASM serializer. Avoid a
    // JSON text round trip that would shorten f32 decimals differently.
    let value = serde_json::to_value(value).map_err(|e| PyValueError::new_err(e.to_string()))?;
    value_to_python(py, &value)
}

fn value_to_python(py: Python<'_>, value: &Value) -> PyResult<PyObject> {
    Ok(match value {
        Value::Null => py.None(),
        Value::Bool(v) => v.into_py(py),
        Value::String(v) => v.into_py(py),
        Value::Number(v) => {
            if let Some(n) = v.as_i64() {
                n.into_py(py)
            } else if let Some(n) = v.as_u64() {
                n.into_py(py)
            } else {
                v.as_f64()
                    .ok_or_else(|| PyValueError::new_err("invalid number"))?
                    .into_py(py)
            }
        }
        Value::Array(values) => {
            let out = PyList::empty_bound(py);
            for v in values {
                out.append(value_to_python(py, v)?)?;
            }
            out.unbind().into()
        }
        Value::Object(values) => {
            let out = PyDict::new_bound(py);
            for (k, v) in values {
                out.set_item(k, value_to_python(py, v)?)?;
            }
            out.unbind().into()
        }
    })
}

fn jwc_error(error: JwcError) -> PyErr {
    match error {
        JwcError::Io(e) => PyIOError::new_err(e.to_string()),
        e => PyValueError::new_err(e.to_string()),
    }
}

fn conversion_error(error: JwcConversionError) -> PyErr {
    match error {
        JwcConversionError::Input(e) => jwc_error(e),
        e => PyValueError::new_err(e.to_string()),
    }
}

#[pyfunction]
pub(crate) fn detect_file_format(path: &str) -> PyResult<Option<&'static str>> {
    Ok(ezjww_core::detect_file_format(path)
        .map_err(|e| PyIOError::new_err(e.to_string()))?
        .map(|format| match format {
            CadFormat::Jww => "jww",
            CadFormat::Jwc => "jwc",
        }))
}

#[pyfunction]
pub(crate) fn is_jwc_file(path: &str) -> PyResult<bool> {
    Ok(detect_file_format(path)? == Some("jwc"))
}

#[pyfunction]
fn read_jwc_header(py: Python<'_>, path: &str) -> PyResult<PyObject> {
    to_python(
        py,
        &ezjww_core::read_jwc_header_from_file(path).map_err(jwc_error)?,
    )
}

#[pyfunction]
fn read_jwc_document(py: Python<'_>, path: &str) -> PyResult<PyObject> {
    let doc = ezjww_core::read_jwc_document_from_file(path).map_err(jwc_error)?;
    to_python(py, &jwc_document_to_dto(&doc))
}

#[pyfunction]
fn read_cad_document(py: Python<'_>, path: &str) -> PyResult<PyObject> {
    let format = detect_file_format(path)?
        .ok_or_else(|| PyValueError::new_err("unrecognized CAD format"))?;
    let document = if format == "jwc" {
        read_jwc_document(py, path)?
    } else {
        super::read_document(py, path)?
    };
    let out = PyDict::new_bound(py);
    out.set_item("format", format)?;
    out.set_item("document", document)?;
    Ok(out.unbind().into())
}

pub(crate) fn convert(
    path: &str,
    options: ConvertOptions,
    coordinates: &str,
) -> PyResult<JwcDxfConversion> {
    let options = JwcConvertOptions {
        coordinates: coordinates.parse().map_err(conversion_error)?,
        dxf: options,
    };
    ezjww_core::jwc::read_jwc_dxf_from_file(path, options).map_err(conversion_error)
}

pub(crate) fn dxf_document(
    py: Python<'_>,
    path: &str,
    options: ConvertOptions,
    coordinates: &str,
) -> PyResult<PyObject> {
    to_python(
        py,
        &jwc_dxf_document_to_dto(&convert(path, options, coordinates)?),
    )
}

pub(crate) fn write_report(
    py: Python<'_>,
    path: &str,
    output: &str,
    options: ConvertOptions,
    coordinates: &str,
    target: DxfTargetVersion,
) -> PyResult<PyObject> {
    let source = ezjww_core::read_jwc_document_from_file(path).map_err(jwc_error)?;
    let converted = convert_jwc_document(
        &source,
        JwcConvertOptions {
            coordinates: coordinates.parse().map_err(conversion_error)?,
            dxf: options,
        },
    )
    .map_err(conversion_error)?;
    converted
        .write_to_file(output, target)
        .map_err(|e| PyIOError::new_err(e.to_string()))?;
    let out = PyDict::new_bound(py);
    out.set_item("source_format", "jwc")?;
    out.set_item("source_profile", to_python(py, &source.profile_id)?)?;
    out.set_item("source_version", py.None())?;
    out.set_item("target_version", target.acad_version())?;
    out.set_item("source_entities", source.entities.len())?;
    out.set_item(
        "source_entity_counts",
        to_python(py, &source.entity_counts())?,
    )?;
    out.set_item(
        "all_source_entity_counts",
        to_python(py, &source.entity_counts())?,
    )?;
    out.set_item("source_block_definitions", 0)?;
    out.set_item("source_block_entities", 0)?;
    out.set_item("converted_entities", converted.document.entities.len())?;
    out.set_item("converted_block_entities", 0)?;
    out.set_item(
        "converted_entity_counts",
        super::dxf_entity_counts_to_pydict(py, &converted.document)?,
    )?;
    out.set_item("unsupported_entity_counts", PyDict::new_bound(py))?;
    let normalized_names = source
        .header
        .layer_groups
        .iter()
        .flat_map(|g| &g.layers)
        .zip(&converted.document.layers)
        .filter(|(src, dst)| !src.name.text.trim().is_empty() && src.name.text != dst.name)
        .count();
    out.set_item("normalized_layer_names", normalized_names)?;
    out.set_item("normalized_block_names", 0)?;
    out.set_item("diagnostics", to_python(py, &source.diagnostics)?)?;
    out.set_item(
        "validation",
        to_python(
            py,
            &ezjww_core::BlockReferenceValidationDto {
                total_references: 0,
                resolved_references: 0,
                unresolved_def_numbers: vec![],
                has_unresolved: false,
            },
        )?,
    )?;
    out.set_item("jwc_conversion_report", to_python(py, &converted.report)?)?;
    Ok(out.unbind().into())
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(detect_file_format, m)?)?;
    m.add_function(wrap_pyfunction!(is_jwc_file, m)?)?;
    m.add_function(wrap_pyfunction!(read_jwc_header, m)?)?;
    m.add_function(wrap_pyfunction!(read_jwc_document, m)?)?;
    m.add_function(wrap_pyfunction!(read_cad_document, m)?)?;
    Ok(())
}
