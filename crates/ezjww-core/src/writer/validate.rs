use super::{JwwWriteDocument, JwwWriteError};
use crate::model::{Entity, EntityBase};

pub(super) fn document(document: &JwwWriteDocument) -> Result<(), JwwWriteError> {
    let options = &document.options;
    if options.version != 700 {
        return Err(JwwWriteError::UnsupportedVersion(options.version));
    }
    if !matches!(options.paper_size, 0..=4 | 8..=14) {
        return Err(JwwWriteError::invalid(
            "options.paper_size",
            "expected 0..=4 or 8..=14",
        ));
    }
    if options.memo.contains('\0') {
        return Err(JwwWriteError::invalid(
            "options.memo",
            "NUL is not supported",
        ));
    }
    let default_base = EntityBase {
        pen_style: 1,
        pen_color: 1,
        ..EntityBase::default()
    };
    for (index, entity) in document.entities.iter().enumerate() {
        let Entity::Line(line) = entity else {
            return Err(JwwWriteError::invalid(
                format!("entities[{index}]"),
                "only LINE is supported by this writer",
            ));
        };
        if line.base != default_base {
            return Err(JwwWriteError::invalid(
                format!("entities[{index}].base"),
                "this writer requires the default pen, layer/group 0 and no flags",
            ));
        }
        for (field, value) in [
            ("start_x", line.start_x),
            ("start_y", line.start_y),
            ("end_x", line.end_x),
            ("end_y", line.end_y),
        ] {
            if !value.is_finite() {
                return Err(JwwWriteError::invalid(
                    format!("entities[{index}].{field}"),
                    "expected a finite coordinate",
                ));
            }
        }
    }
    Ok(())
}
