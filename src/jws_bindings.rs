use crate::jwc_bindings::to_python;
use ezjww_core::{entity_counts, ConvertOptions, DxfTargetVersion, JwsDxfConversion, JwsError};
use pyo3::{
    exceptions::{PyIOError, PyValueError},
    prelude::*,
    types::PyDict,
};

fn error(e: JwsError) -> PyErr {
    match e {
        JwsError::Io(e) => PyIOError::new_err(e.to_string()),
        e => PyValueError::new_err(e.to_string()),
    }
}
#[pyfunction]
pub(crate) fn is_jws_file(path: &str) -> PyResult<bool> {
    Ok(crate::jwc_bindings::detect_file_format(path)? == Some("jws"))
}
#[pyfunction]
fn read_jws_header(py: Python<'_>, path: &str) -> PyResult<PyObject> {
    to_python(
        py,
        &ezjww_core::read_jws_header_from_file(path).map_err(error)?,
    )
}
#[pyfunction]
pub(crate) fn read_jws_document(py: Python<'_>, path: &str) -> PyResult<PyObject> {
    let doc = ezjww_core::read_jws_document_from_file(path).map_err(error)?;
    super::document_parts_to_pyobject(
        py,
        to_python(py, &doc.header)?,
        &doc.entities,
        &doc.block_defs,
        &doc.images,
        &doc.diagnostics,
        &doc.validate_block_references(),
        doc.text_encoding,
    )
}
pub(crate) fn convert(path: &str, options: ConvertOptions) -> PyResult<JwsDxfConversion> {
    let doc = ezjww_core::read_jws_document_from_file(path).map_err(error)?;
    ezjww_core::convert_jws_document(&doc, options).map_err(error)
}
pub(crate) fn write_report(
    py: Python<'_>,
    path: &str,
    output: &str,
    options: ConvertOptions,
    target: DxfTargetVersion,
) -> PyResult<PyObject> {
    let doc = ezjww_core::read_jws_document_from_file(path).map_err(error)?;
    let converted = ezjww_core::convert_jws_document(&doc, options).map_err(error)?;
    converted
        .write_to_file(output, target)
        .map_err(|e| PyIOError::new_err(e.to_string()))?;
    let out = PyDict::new_bound(py);
    out.set_item("source_format", "jws")?;
    out.set_item("source_version", doc.header.version)?;
    out.set_item("target_version", target.acad_version())?;
    out.set_item("source_entities", doc.entities.len())?;
    out.set_item(
        "source_entity_counts",
        to_python(py, &entity_counts(&doc.entities))?,
    )?;
    let mut counts = entity_counts(&doc.entities);
    for b in &doc.block_defs {
        for (k, v) in entity_counts(&b.entities) {
            *counts.entry(k).or_default() += v;
        }
    }
    out.set_item("all_source_entity_counts", to_python(py, &counts)?)?;
    out.set_item("source_block_definitions", doc.block_defs.len())?;
    out.set_item(
        "source_block_entities",
        doc.block_defs
            .iter()
            .map(|b| b.entities.len())
            .sum::<usize>(),
    )?;
    out.set_item("converted_entities", converted.document.entities.len())?;
    out.set_item(
        "converted_block_entities",
        converted
            .document
            .blocks
            .iter()
            .map(|b| b.entities.len())
            .sum::<usize>(),
    )?;
    out.set_item(
        "converted_entity_counts",
        super::dxf_entity_counts_to_pydict(py, &converted.document)?,
    )?;
    out.set_item(
        "unsupported_entity_counts",
        super::unsupported_entity_counts_to_pydict(py, &converted.document.unsupported_entities)?,
    )?;
    out.set_item("normalized_layer_names", 0)?;
    out.set_item(
        "normalized_block_names",
        super::normalized_block_name_count(&doc.block_defs, &converted.document),
    )?;
    out.set_item(
        "validation",
        super::block_reference_validation_to_pydict(py, &doc.validate_block_references())?,
    )?;
    out.set_item("diagnostics", to_python(py, &doc.diagnostics)?)?;
    out.set_item(
        "jws_conversion_report",
        to_python(py, &converted.jws_conversion_report)?,
    )?;
    Ok(out.unbind().into())
}
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(is_jws_file, m)?)?;
    m.add_function(wrap_pyfunction!(read_jws_header, m)?)?;
    m.add_function(wrap_pyfunction!(read_jws_document, m)?)?;
    Ok(())
}
