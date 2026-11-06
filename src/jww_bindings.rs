//! Strict Python adapter for the shared writer input decoder.
use ezjww_core::writer::input::DocumentBuilder;
use ezjww_core::{to_jww_bytes as encode, JwwDocument, JwwWriteDocument};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyList, PyString};
use serde_json::Value;
use std::collections::HashSet;

/// Where a value sits in the input, rendered only for an error message. Building
/// the `document.entities[i].base.layer` string for every value of a drawing
/// with hundreds of thousands of entities was most of the conversion time.
#[derive(Clone, Copy)]
enum InputPath<'a> {
    Root(&'a str),
    Index(&'a InputPath<'a>, usize),
    Key(&'a InputPath<'a>, &'a str),
}

impl InputPath<'_> {
    fn render(&self) -> String {
        match self {
            InputPath::Root(name) => (*name).to_owned(),
            InputPath::Index(parent, index) => format!("{}[{index}]", parent.render()),
            InputPath::Key(parent, key) => format!("{}.{key}", parent.render()),
        }
    }
}

fn value(input: &Bound<'_, PyAny>, path: &InputPath<'_>, depth: usize) -> PyResult<Value> {
    let invalid = |message: &str| PyValueError::new_err(format!("{}: {message}", path.render()));
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
    if let Ok(bytes) = input.downcast::<PyBytes>() {
        // Image payloads: Python passes bytes, the shared decoder takes base64.
        return Ok(Value::String(ezjww_core::base64::encode(bytes.as_bytes())));
    }
    if let Ok(items) = input.downcast::<PyList>() {
        return items
            .iter()
            .enumerate()
            .map(|(i, v)| value(&v, &InputPath::Index(path, i), depth + 1))
            .collect::<PyResult<Vec<_>>>()
            .map(Value::Array);
    }
    if let Ok(items) = input.downcast::<PyDict>() {
        let mut out = serde_json::Map::new();
        for (key, v) in items.iter() {
            let key = key
                .extract::<String>()
                .map_err(|_| invalid("expected string keys"))?;
            let item = value(&v, &InputPath::Key(path, &key), depth + 1)?;
            out.insert(key, item);
        }
        return Ok(Value::Object(out));
    }
    Err(invalid("expected a dict, list, string, bool or number"))
}

fn bytes(document: &Bound<'_, PyDict>) -> PyResult<Vec<u8>> {
    let parsed = match streamed(document) {
        Some(parsed) => parsed,
        // Every error, and any other shape: the whole-document decoder reports it.
        None => ezjww_core::writer::input::from_value(value(
            document.as_any(),
            &InputPath::Root("document"),
            0,
        )?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?,
    };
    encode(&parsed).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// The write document decoded one list element at a time (`DocumentBuilder`).
/// As one `Value` tree, the input of a drawing of 840,000 entities took 1.6 GB
/// next to the caller's dicts. `None` on any error or unusual shape, which the
/// whole-document decoder then reports as before.
fn streamed(document: &Bound<'_, PyDict>) -> Option<JwwWriteDocument> {
    // block_defs, entities, images: the order in which pens are allocated
    let mut lists: [Option<Bound<'_, PyList>>; 3] = [None, None, None];
    let mut options = None;
    for (key, item) in document.iter() {
        let slot = match key.extract::<String>().ok()?.as_str() {
            "options" => {
                options = Some(item);
                continue;
            }
            "block_defs" => 0,
            "entities" => 1,
            "images" => 2,
            _ => return None,
        };
        lists[slot] = Some(item.downcast_into::<PyList>().ok()?);
    }
    lists[1].as_ref()?;
    let root = InputPath::Root("document");
    let options = value(&options?, &InputPath::Key(&root, "options"), 1).ok()?;
    let mut used = HashSet::new();
    for list in lists.iter().flatten() {
        scan_pens(list.as_any(), &mut used, 1);
    }
    let mut builder = DocumentBuilder::new(options, used).ok()?;
    for (slot, key) in ["block_defs", "entities", "images"].into_iter().enumerate() {
        let Some(list) = &lists[slot] else {
            continue;
        };
        let at = InputPath::Key(&root, key);
        for (i, item) in list.iter().enumerate() {
            let element = value(&item, &InputPath::Index(&at, i), 2).ok()?;
            match slot {
                0 => builder.push_block_def(element),
                1 => builder.push_entity(element),
                _ => builder.push_image(element),
            }
            .ok()?;
        }
    }
    builder.finish().ok()
}

/// `ezjww_core::writer::input::scan_pens` over the Python input: each
/// `base.pen_color` that `value` turns into an unsigned integer up to u16::MAX.
fn scan_pens(input: &Bound<'_, PyAny>, used: &mut HashSet<u16>, depth: usize) {
    if depth > 64 {
        return; // `value` rejects the input
    }
    if let Ok(items) = input.downcast::<PyList>() {
        for item in items.iter() {
            scan_pens(&item, used, depth + 1);
        }
        return;
    }
    let Ok(items) = input.downcast::<PyDict>() else {
        return;
    };
    let py = input.py();
    if let Ok(Some(base)) = items.get_item(pyo3::intern!(py, "base")) {
        if let Ok(base) = base.downcast::<PyDict>() {
            if let Ok(Some(pen)) = base.get_item(pyo3::intern!(py, "pen_color")) {
                if pen.is_instance_of::<PyInt>() && !pen.is_instance_of::<PyBool>() {
                    if let Ok(pen) = pen.extract::<u64>() {
                        if let Ok(pen) = u16::try_from(pen) {
                            used.insert(pen);
                        }
                    }
                }
            }
        }
    }
    for (key, item) in items.iter() {
        // Only lists and dicts hold pens; the key is compared for those alone.
        if (item.is_instance_of::<PyList>() || item.is_instance_of::<PyDict>())
            && !key.eq(pyo3::intern!(py, "options")).unwrap_or(false)
        {
            scan_pens(&item, used, depth + 1);
        }
    }
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
    super::document_to_pyobject(py, &parsed(document)?, &[], ezjww_core::TextEncoding::Cp932)
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
        value(source.as_any(), &InputPath::Root("document"), 0)?,
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
