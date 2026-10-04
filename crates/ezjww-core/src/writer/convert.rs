//! Explicit conversion of reader dictionaries. No claim of lossless file editing.
use super::{input, validate, JwwWriteError};
use serde_json::{json, Value};

pub fn to_write_document(source: Value, skip_unsupported: bool) -> Result<Value, JwwWriteError> {
    let header = source
        .get("header")
        .and_then(Value::as_object)
        .ok_or_else(|| JwwWriteError::invalid("header", "expected a parsed JWW header"))?;
    let mut target = input::new_document();
    let mut diagnostics = vec![
        json!({"code":"JWW_TEMPLATE_DEFAULTS", "severity":"warning", "path":"header",
        "message":"Only exposed writer settings are retained. Other header fields, timestamps and embedded files use new-document defaults; this is not a lossless rewrite.", "action":"defaults"}),
    ];
    for field in [
        "memo",
        "paper_size",
        "write_layer_group",
        "layer_groups",
        "palette",
        "line_types",
        "text_presets",
    ] {
        if let Some(value) = header.get(field).filter(|v| !v.is_null()) {
            target["options"][field] = value.clone();
        }
    }
    if header.get("version").and_then(Value::as_u64) != Some(700) {
        diagnostics.push(json!({"code":"JWW_OUTPUT_VERSION", "severity":"info", "path":"header.version", "message":"Output uses version 700", "action":"converted"}));
    }
    // Check header shape before indexing mutably.
    input::from_value(target.clone())?;
    // A hidden reader group can have no current layer. A new drawing always
    // has one current layer per group, selected by write_layer.
    let selected = target["options"]["write_layer_group"].as_u64();
    if let Some(groups) = target["options"]["layer_groups"].as_array_mut() {
        for (g, group) in groups.iter_mut().enumerate() {
            let before = group.clone();
            if selected == Some(g as u64) {
                group["state"] = json!(3);
            } else if group["state"] == 3 {
                group["state"] = json!(2);
            }
            let current = group["write_layer"].as_u64();
            if let Some(layers) = group["layers"].as_array_mut() {
                for (l, layer) in layers.iter_mut().enumerate() {
                    if current == Some(l as u64) {
                        layer["state"] = json!(3);
                    } else if layer["state"] == 3 {
                        layer["state"] = json!(2);
                    }
                }
            }
            if before != *group {
                diagnostics.push(json!({"code":"JWW_CURRENT_LAYER", "severity":"warning", "path":format!("header.layer_groups[{g}]"), "message":"Normalized current group/layer states", "action":"normalized"}));
            }
        }
    }
    let entities = source
        .get("entities")
        .and_then(Value::as_array)
        .ok_or_else(|| JwwWriteError::invalid("entities", "expected a list"))?;
    let definitions = source
        .get("block_defs")
        .and_then(Value::as_array)
        .ok_or_else(|| JwwWriteError::invalid("block_defs", "expected a list"))?;
    let mut fatal = false;
    if let Some(issues) = source.get("diagnostics").and_then(Value::as_array) {
        for (i, issue) in issues.iter().enumerate() {
            let error = issue["severity"] == "error";
            fatal |= error;
            diagnostics.push(json!({"code":"JWW_READER_DIAGNOSTIC", "severity":if error {"error"} else {"warning"},
                "path":format!("diagnostics[{i}]"), "message":issue.to_string(),
                "action":if error {"rejected"} else {"retained"}}));
        }
    }
    let options = input::from_value(target.clone())?.options;
    // Python's reader adds computed dash runs to some tables. Return the same
    // canonical writable settings in both bindings, without those derived fields.
    target["options"] = json!(options);
    let mut convert_entities = |entities: &[Value], path: &str| -> Vec<Value> {
        let mut out = Vec::new();
        for (i, value) in entities.iter().enumerate() {
            let path = format!("{path}[{i}]");
            let mut value = value.clone();
            if let Some(obj) = value.as_object_mut() {
                obj.remove("block_name");
            }
            let checked = dimension_bases(&value, &path).and_then(|()| {
                input::entity(&value, &path).and_then(|e| validate::entity(&e, &path, &options))
            });
            match checked {
                Ok(()) => out.push(value),
                Err(error) => {
                    fatal |= !skip_unsupported;
                    diagnostics.push(json!({"code":"JWW_UNSUPPORTED_ENTITY", "severity":"error", "path":path,
                        "message":error.to_string(), "action":if skip_unsupported {"omitted"} else {"rejected"}}));
                }
            }
        }
        out
    };
    target["entities"] = json!(convert_entities(entities, "entities"));
    let mut blocks = Vec::new();
    for (i, definition) in definitions.iter().enumerate() {
        let mut definition = definition.clone();
        let entities = definition
            .get("entities")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                JwwWriteError::invalid(format!("block_defs[{i}].entities"), "expected a list")
            })?;
        definition["entities"] = json!(convert_entities(
            entities,
            &format!("block_defs[{i}].entities")
        ));
        blocks.push(definition);
    }
    target["block_defs"] = json!(blocks);
    // Recompute reference metadata, while missing/cyclic references still fail.
    let referenced: std::collections::HashSet<u64> = target["entities"]
        .as_array()
        .unwrap()
        .iter()
        .chain(
            target["block_defs"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|b| b["entities"].as_array().unwrap()),
        )
        .filter(|v| v["type"] == "BLOCK")
        .filter_map(|v| v["def_number"].as_u64())
        .collect();
    for definition in target["block_defs"].as_array_mut().unwrap() {
        definition["is_referenced"] = json!(definition["number"]
            .as_u64()
            .is_some_and(|n| referenced.contains(&n)));
    }
    if let Err(error) = input::from_value(target.clone()).and_then(|d| validate::document(&d)) {
        fatal = true;
        diagnostics.push(json!({"code":"JWW_UNSUPPORTED_DOCUMENT", "severity":"error", "path":"document", "message":error.to_string(), "action":"rejected"}));
    }
    Ok(json!({"document":if fatal {Value::Null}else{target}, "diagnostics":diagnostics}))
}

fn dimension_bases(value: &Value, path: &str) -> Result<(), JwwWriteError> {
    if value["type"] == "DIMENSION" {
        let members = [value.get("line"), value.get("text")]
            .into_iter()
            .flatten()
            .chain(
                ["aux_lines", "aux_points"]
                    .into_iter()
                    .flat_map(|key| value[key].as_array().into_iter().flatten()),
            );
        if members.into_iter().any(|v| v.get("base").is_none()) {
            return Err(JwwWriteError::invalid(path,
                "dimension member attributes are missing; read the file again with this version of ezjww"));
        }
    }
    Ok(())
}
