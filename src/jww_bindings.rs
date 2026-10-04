//! Strict Python adapter for the shared writer input decoder.
use ezjww_core::{to_jww_bytes as encode, JwwDocument};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyList, PyString};
use serde_json::Value;

fn value(input: &Bound<'_, PyAny>, path: &str, depth: usize) -> PyResult<Value> {
    let invalid = |message: &str| PyValueError::new_err(format!("{path}: {message}"));
    if depth > 64 {
        return Err(invalid("input nesting exceeds 64"));
    }
    if input.is_none() {
        return Ok(Value::Null);
    }
    if input.is_instance_of::<PyBool>() {
        return Ok(Value::Bool(input.extract()?));
    }
    if input.is_instance_of::<PyInt>() {
        if let Ok(v) = input.extract::<i64>() {
            return Ok(Value::from(v));
        }
        return input
            .extract::<u64>()
            .map(Value::from)
            .map_err(|_| invalid("integer out of range"));
    }
    if input.is_instance_of::<PyFloat>() {
        return serde_json::Number::from_f64(input.extract()?)
            .map(Value::Number)
            .ok_or_else(|| invalid("expected a finite number"));
    }
    if input.is_instance_of::<PyString>() {
        return input
            .extract::<String>()
            .map(Value::from)
            .map_err(|_| invalid("invalid Unicode string"));
    }
    if let Ok(items) = input.downcast::<PyList>() {
        return items
            .iter()
            .enumerate()
            .map(|(i, v)| value(&v, &format!("{path}[{i}]"), depth + 1))
            .collect::<PyResult<Vec<_>>>()
            .map(Value::Array);
    }
    if let Ok(items) = input.downcast::<PyDict>() {
        let mut out = serde_json::Map::new();
        for (key, v) in items.iter() {
            let key = key
                .extract::<String>()
                .map_err(|_| invalid("expected string keys"))?;
            out.insert(key.clone(), value(&v, &format!("{path}.{key}"), depth + 1)?);
        }
        return Ok(Value::Object(out));
    }
    Err(invalid("expected a dict, list, string, bool or number"))
}

fn bytes(document: &Bound<'_, PyDict>) -> PyResult<Vec<u8>> {
    encode(
        &ezjww_core::writer::input::from_value(value(document.as_any(), "document", 0)?)
            .map_err(|e| PyValueError::new_err(e.to_string()))?,
    )
    .map_err(|e| PyValueError::new_err(e.to_string()))
}

fn parsed(document: &Bound<'_, PyDict>) -> PyResult<JwwDocument> {
    // Use the same full header (including palette/line types) as saved files.
    ezjww_core::parse_document(&bytes(document)?).map_err(super::to_py_err)
}

#[pyfunction]
fn new_jww_document(py: Python<'_>) -> PyResult<PyObject> {
    let value = ezjww_core::writer::input::new_document();
    super::jwc_bindings::to_python(py, &value)
}

#[pyfunction]
fn to_jww_bytes<'py>(
    py: Python<'py>,
    document: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyBytes>> {
    Ok(PyBytes::new_bound(py, &bytes(document)?))
}

#[pyfunction]
fn jww_write_document_to_document(
    py: Python<'_>,
    document: &Bound<'_, PyDict>,
) -> PyResult<PyObject> {
    super::document_to_pyobject(py, &parsed(document)?, &[])
}

fn writer_convert_options(
    explode_inserts: bool,
    max_block_nesting: usize,
    text_em_scale: f64,
) -> PyResult<ezjww_core::ConvertOptions> {
    if max_block_nesting == 0 {
        return Err(PyValueError::new_err("max_block_nesting must be positive"));
    }
    super::convert_options(explode_inserts, max_block_nesting, text_em_scale)
}

#[pyfunction(signature = (document, explode_inserts=false, max_block_nesting=32, text_em_scale=1.0))]
fn jww_write_document_to_dxf(
    py: Python<'_>,
    document: &Bound<'_, PyDict>,
    explode_inserts: bool,
    max_block_nesting: usize,
    text_em_scale: f64,
) -> PyResult<PyObject> {
    let options = writer_convert_options(explode_inserts, max_block_nesting, text_em_scale)?;
    let dxf = ezjww_core::convert_document_with_options(&parsed(document)?, options);
    Ok(super::dxf_document_to_pydict(py, &dxf)?.unbind().into())
}

#[pyfunction(signature = (document, explode_inserts=false, max_block_nesting=32, target_version="AC1015", text_em_scale=1.0))]
fn jww_write_document_to_dxf_string(
    document: &Bound<'_, PyDict>,
    explode_inserts: bool,
    max_block_nesting: usize,
    target_version: &str,
    text_em_scale: f64,
) -> PyResult<String> {
    let version = super::parse_dxf_target_version(target_version)?;
    let options = writer_convert_options(explode_inserts, max_block_nesting, text_em_scale)?;
    let dxf = ezjww_core::convert_document_with_options(&parsed(document)?, options);
    Ok(ezjww_core::document_to_string_with_version(&dxf, version))
}

#[pyfunction(signature = (document, skip_unsupported=false))]
fn to_write_document(
    py: Python<'_>,
    document: &Bound<'_, PyDict>,
    skip_unsupported: bool,
) -> PyResult<PyObject> {
    let source = document.copy()?;
    if source.contains("block_def_names")? {
        source.del_item("block_def_names")?;
    }
    let report = ezjww_core::writer::to_write_document(
        value(source.as_any(), "document", 0)?,
        skip_unsupported,
    )
    .map_err(|e| PyValueError::new_err(e.to_string()))?;
    super::jwc_bindings::to_python(py, &report)
}

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(new_jww_document, m)?)?;
    m.add_function(wrap_pyfunction!(to_write_document, m)?)?;
    m.add_function(wrap_pyfunction!(to_jww_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(jww_write_document_to_document, m)?)?;
    m.add_function(wrap_pyfunction!(jww_write_document_to_dxf, m)?)?;
    m.add_function(wrap_pyfunction!(jww_write_document_to_dxf_string, m)?)?;
    Ok(())
}
