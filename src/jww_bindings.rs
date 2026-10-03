//! Strict Python input for the bounded new-document writer. Parsed documents
//! have a different shape and cannot be silently treated as editable inputs.
use ezjww_core::{
    to_jww_bytes as encode, Arc, Entity, EntityBase, JwwDocument, JwwWriteDocument,
    JwwWriteOptions, LayerGroupHeader, LayerHeader, Line, Point, Text,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyList};

fn invalid(path: &str, message: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(format!("{path}: {message}"))
}

fn keys(dict: &Bound<'_, PyDict>, allowed: &[&str], path: &str) -> PyResult<()> {
    for key in dict.keys().iter() {
        let key: String = key
            .extract()
            .map_err(|_| invalid(path, "expected string keys"))?;
        if !allowed.contains(&key.as_str()) {
            return Err(invalid(&format!("{path}.{key}"), "unsupported field"));
        }
    }
    Ok(())
}

fn required<'py>(dict: &Bound<'py, PyDict>, key: &str, path: &str) -> PyResult<Bound<'py, PyAny>> {
    dict.get_item(key)?
        .ok_or_else(|| invalid(&format!("{path}.{key}"), "missing field"))
}

fn get<'py, T: FromPyObject<'py>>(dict: &Bound<'py, PyDict>, key: &str, path: &str) -> PyResult<T> {
    let value = required(dict, key, path)?;
    if value.is_instance_of::<PyBool>() {
        return Err(invalid(
            &format!("{path}.{key}"),
            "expected a number or string, not bool",
        ));
    }
    value
        .extract()
        .map_err(|error| invalid(&format!("{path}.{key}"), error))
}

fn boolean(dict: &Bound<'_, PyDict>, key: &str, path: &str) -> PyResult<bool> {
    let value = required(dict, key, path)?;
    value
        .downcast::<PyBool>()
        .map_err(|_| invalid(&format!("{path}.{key}"), "expected bool"))?;
    value.extract()
}

fn dictionary<'py>(value: &Bound<'py, PyAny>, path: &str) -> PyResult<Bound<'py, PyDict>> {
    value
        .downcast::<PyDict>()
        .cloned()
        .map_err(|_| invalid(path, "expected a dict"))
}

fn list<'py>(value: &Bound<'py, PyAny>, path: &str) -> PyResult<Bound<'py, PyList>> {
    value
        .downcast::<PyList>()
        .cloned()
        .map_err(|_| invalid(path, "expected a list"))
}

fn options(input: &Bound<'_, PyDict>) -> PyResult<JwwWriteOptions> {
    let path = "options";
    keys(
        input,
        &[
            "version",
            "memo",
            "paper_size",
            "write_layer_group",
            "layer_groups",
        ],
        path,
    )?;
    let groups = list(
        &required(input, "layer_groups", path)?,
        "options.layer_groups",
    )?;
    if groups.len() != 16 {
        return Err(invalid(
            "options.layer_groups",
            "expected exactly 16 groups",
        ));
    }
    let mut out = JwwWriteOptions {
        version: get(input, "version", path)?,
        memo: get(input, "memo", path)?,
        paper_size: get(input, "paper_size", path)?,
        write_layer_group: get(input, "write_layer_group", path)?,
        ..JwwWriteOptions::default()
    };
    for (g, value) in groups.iter().enumerate() {
        let path = format!("options.layer_groups[{g}]");
        let group = dictionary(&value, &path)?;
        keys(
            &group,
            &["state", "write_layer", "scale", "protect", "name", "layers"],
            &path,
        )?;
        let layers = list(
            &required(&group, "layers", &path)?,
            &format!("{path}.layers"),
        )?;
        if layers.len() != 16 {
            return Err(invalid(
                &format!("{path}.layers"),
                "expected exactly 16 layers",
            ));
        }
        let target = &mut out.layer_groups[g];
        *target = LayerGroupHeader {
            state: get(&group, "state", &path)?,
            write_layer: get(&group, "write_layer", &path)?,
            scale: get(&group, "scale", &path)?,
            protect: get(&group, "protect", &path)?,
            name: get(&group, "name", &path)?,
            ..LayerGroupHeader::default()
        };
        for (l, value) in layers.iter().enumerate() {
            let path = format!("{path}.layers[{l}]");
            let layer = dictionary(&value, &path)?;
            keys(&layer, &["state", "protect", "name"], &path)?;
            target.layers[l] = LayerHeader {
                state: get(&layer, "state", &path)?,
                protect: get(&layer, "protect", &path)?,
                name: get(&layer, "name", &path)?,
            };
        }
    }
    Ok(out)
}

fn entity(input: &Bound<'_, PyDict>, path: &str) -> PyResult<Entity> {
    let kind: String = get(input, "type", path)?;
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
    keys(input, &allowed, path)?;
    let base_path = format!("{path}.base");
    let b = dictionary(&required(input, "base", path)?, &base_path)?;
    keys(
        &b,
        &[
            "group",
            "pen_style",
            "pen_color",
            "pen_width",
            "layer",
            "layer_group",
            "flag",
        ],
        &base_path,
    )?;
    let base = EntityBase {
        group: get(&b, "group", &base_path)?,
        pen_style: get(&b, "pen_style", &base_path)?,
        pen_color: get(&b, "pen_color", &base_path)?,
        pen_width: get(&b, "pen_width", &base_path)?,
        layer: get(&b, "layer", &base_path)?,
        layer_group: get(&b, "layer_group", &base_path)?,
        flag: get(&b, "flag", &base_path)?,
    };
    Ok(match kind.as_str() {
        "LINE" => Entity::Line(Line {
            base,
            start_x: get(input, "start_x", path)?,
            start_y: get(input, "start_y", path)?,
            end_x: get(input, "end_x", path)?,
            end_y: get(input, "end_y", path)?,
        }),
        "CIRCLE" | "ARC" => {
            let full = boolean(input, "is_full_circle", path)?;
            if full != (kind == "CIRCLE") {
                return Err(invalid(
                    &format!("{path}.is_full_circle"),
                    "must agree with entity type",
                ));
            }
            Entity::Arc(Arc {
                base,
                center_x: get(input, "center_x", path)?,
                center_y: get(input, "center_y", path)?,
                radius: get(input, "radius", path)?,
                start_angle: get(input, "start_angle", path)?,
                arc_angle: get(input, "arc_angle", path)?,
                tilt_angle: get(input, "tilt_angle", path)?,
                flatness: get(input, "flatness", path)?,
                is_full_circle: full,
            })
        }
        "POINT" => Entity::Point(Point {
            base,
            x: get(input, "x", path)?,
            y: get(input, "y", path)?,
            is_temporary: boolean(input, "is_temporary", path)?,
            code: get(input, "code", path)?,
            angle: get(input, "angle", path)?,
            scale: get(input, "scale", path)?,
        }),
        "TEXT" => Entity::Text(Text {
            base,
            start_x: get(input, "start_x", path)?,
            start_y: get(input, "start_y", path)?,
            end_x: get(input, "end_x", path)?,
            end_y: get(input, "end_y", path)?,
            text_type: get(input, "text_type", path)?,
            size_x: get(input, "size_x", path)?,
            size_y: get(input, "size_y", path)?,
            spacing: get(input, "spacing", path)?,
            angle: get(input, "angle", path)?,
            font_name: get(input, "font_name", path)?,
            content: get(input, "content", path)?,
        }),
        _ => unreachable!(),
    })
}

fn input(document: &Bound<'_, PyDict>) -> PyResult<JwwWriteDocument> {
    keys(document, &["options", "entities"], "document")?;
    let options = options(&dictionary(
        &required(document, "options", "document")?,
        "options",
    )?)?;
    let values = list(&required(document, "entities", "document")?, "entities")?;
    let mut entities = Vec::with_capacity(values.len());
    for (i, value) in values.iter().enumerate() {
        let path = format!("entities[{i}]");
        entities.push(entity(&dictionary(&value, &path)?, &path)?);
    }
    Ok(JwwWriteDocument { options, entities })
}

fn bytes(document: &Bound<'_, PyDict>) -> PyResult<Vec<u8>> {
    encode(&input(document)?).map_err(|e| PyValueError::new_err(e.to_string()))
}

fn parsed(document: &Bound<'_, PyDict>) -> PyResult<JwwDocument> {
    // Use the same full header (including palette/line types) as saved files.
    ezjww_core::parse_document(&bytes(document)?).map_err(super::to_py_err)
}

#[pyfunction]
fn new_jww_document(py: Python<'_>) -> PyResult<PyObject> {
    let defaults = JwwWriteDocument::default();
    let native = ezjww_core::parse_header(
        &encode(&defaults).map_err(|e| PyValueError::new_err(e.to_string()))?,
    )
    .map_err(super::to_py_err)?;
    let options = super::header_to_pydict(py, &native)?;
    options.del_item("palette")?;
    options.del_item("line_types")?;
    // Header readers synthesize display names for blank names. Writer inputs
    // must keep the original blank names, so default bytes match the Rust API.
    let groups = options.get_item("layer_groups")?.unwrap();
    for (g, group) in defaults.options.layer_groups.iter().enumerate() {
        let target = groups.get_item(g)?;
        target.set_item("name", &group.name)?;
        let layers = target.get_item("layers")?;
        for (l, layer) in group.layers.iter().enumerate() {
            layers.get_item(l)?.set_item("name", &layer.name)?;
        }
    }
    let out = PyDict::new_bound(py);
    out.set_item("options", options)?;
    out.set_item("entities", PyList::empty_bound(py))?;
    Ok(out.unbind().into())
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

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(new_jww_document, m)?)?;
    m.add_function(wrap_pyfunction!(to_jww_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(jww_write_document_to_document, m)?)?;
    m.add_function(wrap_pyfunction!(jww_write_document_to_dxf, m)?)?;
    m.add_function(wrap_pyfunction!(jww_write_document_to_dxf_string, m)?)?;
    Ok(())
}
