use super::{JwwWriteDocument, JwwWriteError};
use crate::model::{metadata_setting_from_text, Entity, EntityBase};
use std::f64::consts::TAU;

type Result = std::result::Result<(), JwwWriteError>;

fn require(valid: bool, field: &str, reason: &str) -> Result {
    if valid {
        Ok(())
    } else {
        Err(JwwWriteError::invalid(field, reason))
    }
}

fn finite(value: f64, field: &str) -> Result {
    require(value.is_finite(), field, "expected a finite number")
}

fn string(value: &str, field: &str) -> Result {
    require(!value.contains('\0'), field, "NUL is not supported")
}

pub(super) fn document(document: &JwwWriteDocument) -> Result {
    let options = &document.options;
    if options.version != 700 {
        return Err(JwwWriteError::UnsupportedVersion(options.version));
    }
    require(
        matches!(options.paper_size, 0..=4 | 8..=14),
        "options.paper_size",
        "expected 0..=4 or 8..=14",
    )?;
    string(&options.memo, "options.memo")?;
    require(
        options.write_layer_group < 16,
        "options.write_layer_group",
        "expected 0..=15",
    )?;
    for (g, group) in options.layer_groups.iter().enumerate() {
        let path = format!("options.layer_groups[{g}]");
        require(
            group.state <= 3 && (group.state == 3) == (g == options.write_layer_group as usize),
            &format!("{path}.state"),
            "expected 0..=3 with only the current group in state 3",
        )?;
        require(
            group.write_layer < 16,
            &format!("{path}.write_layer"),
            "expected 0..=15",
        )?;
        require(
            group.scale.is_finite() && group.scale > 0.0,
            &format!("{path}.scale"),
            "expected a finite positive scale denominator",
        )?;
        require(
            group.protect <= 2,
            &format!("{path}.protect"),
            "expected 0..=2",
        )?;
        string(&group.name, &format!("{path}.name"))?;
        for (l, layer) in group.layers.iter().enumerate() {
            let path = format!("{path}.layers[{l}]");
            require(
                layer.state <= 3 && (layer.state == 3) == (l == group.write_layer as usize),
                &format!("{path}.state"),
                "expected 0..=3 with only the current layer in state 3",
            )?;
            require(
                layer.protect <= 2,
                &format!("{path}.protect"),
                "expected 0..=2",
            )?;
            string(&layer.name, &format!("{path}.name"))?;
        }
    }
    for (index, entity) in document.entities.iter().enumerate() {
        let path = format!("entities[{index}]");
        let check = |field: &str, value: f64| finite(value, &format!("{path}.{field}"));
        let valid =
            |field: &str, ok: bool, reason: &str| require(ok, &format!("{path}.{field}"), reason);
        match entity {
            Entity::Line(line) => {
                base(&line.base, &path, false, false)?;
                for (field, value) in [
                    ("start_x", line.start_x),
                    ("start_y", line.start_y),
                    ("end_x", line.end_x),
                    ("end_y", line.end_y),
                ] {
                    check(field, value)?;
                }
            }
            Entity::Arc(arc) => {
                base(&arc.base, &path, false, false)?;
                for (field, value) in [
                    ("center_x", arc.center_x),
                    ("center_y", arc.center_y),
                    ("radius", arc.radius),
                    ("start_angle", arc.start_angle),
                    ("arc_angle", arc.arc_angle),
                    ("tilt_angle", arc.tilt_angle),
                    ("flatness", arc.flatness),
                ] {
                    check(field, value)?;
                }
                valid("radius", arc.radius > 0.0, "expected a positive radius")?;
                valid(
                    "flatness",
                    arc.flatness == 1.0,
                    "ellipses are not supported",
                )?;
                valid(
                    "tilt_angle",
                    arc.tilt_angle == 0.0,
                    "expected zero for a circular arc",
                )?;
                if arc.is_full_circle {
                    valid(
                        "start_angle",
                        arc.start_angle == 0.0,
                        "full circles must start at zero",
                    )?;
                    valid(
                        "arc_angle",
                        arc.arc_angle == TAU,
                        "full circles must sweep 2*pi radians",
                    )?;
                } else {
                    valid(
                        "start_angle",
                        (0.0..TAU).contains(&arc.start_angle),
                        "expected radians in [0, 2*pi)",
                    )?;
                    valid(
                        "arc_angle",
                        arc.arc_angle > 0.0 && arc.arc_angle < TAU,
                        "expected a counterclockwise sweep in (0, 2*pi) radians",
                    )?;
                }
            }
            Entity::Point(point) => {
                base(&point.base, &path, true, false)?;
                check("x", point.x)?;
                check("y", point.y)?;
                valid(
                    "is_temporary",
                    !point.is_temporary,
                    "temporary points are not supported",
                )?;
                valid("code", point.code == 0, "marker points are not supported")?;
                valid(
                    "angle",
                    point.angle == 0.0,
                    "ordinary points have no stored angle",
                )?;
                valid(
                    "scale",
                    point.scale == 0.0,
                    "ordinary points have no stored scale; use zero",
                )?;
            }
            Entity::Text(text) => {
                base(&text.base, &path, false, true)?;
                for (field, value) in [
                    ("start_x", text.start_x),
                    ("start_y", text.start_y),
                    ("end_x", text.end_x),
                    ("end_y", text.end_y),
                    ("size_x", text.size_x),
                    ("size_y", text.size_y),
                    ("spacing", text.spacing),
                    ("angle", text.angle),
                ] {
                    check(field, value)?;
                }
                valid(
                    "size_x",
                    text.size_x > 0.0,
                    "expected a positive character width",
                )?;
                valid(
                    "size_y",
                    text.size_y > 0.0,
                    "expected a positive character height",
                )?;
                valid(
                    "spacing",
                    text.spacing >= 0.0,
                    "expected nonnegative spacing",
                )?;
                valid(
                    "angle",
                    (0.0..360.0).contains(&text.angle),
                    "expected degrees in [0, 360)",
                )?;
                valid(
                    "text_type",
                    (0..=10).contains(&text.text_type),
                    "only plain text types 0..=10 are supported",
                )?;
                for (field, value) in [("font_name", &text.font_name), ("content", &text.content)] {
                    string(value, &format!("{path}.{field}"))?;
                    valid(
                        field,
                        !value.chars().any(char::is_control),
                        "expected a single line without control characters",
                    )?;
                }
                valid(
                    "font_name",
                    !text.font_name.is_empty(),
                    "expected a font name",
                )?;
                valid(
                    "content",
                    !text.content.starts_with("^@"),
                    "embedded image/control text is not supported",
                )?;
                valid(
                    "content",
                    metadata_setting_from_text(text).is_none(),
                    "native internal setting text is not supported",
                )?;
            }
            _ => return Err(JwwWriteError::invalid(path, "unsupported entity type")),
        }
    }
    Ok(())
}

fn base(base: &EntityBase, path: &str, point: bool, text: bool) -> Result {
    let path = format!("{path}.base");
    require(
        base.group == 0 && base.flag == 0,
        &path,
        "curve groups and attribute flags are not supported",
    )?;
    require(base.layer < 16, &format!("{path}.layer"), "expected 0..=15")?;
    require(
        base.layer_group < 16,
        &format!("{path}.layer_group"),
        "expected 0..=15",
    )?;
    require(
        (1..=9).contains(&base.pen_color),
        &format!("{path}.pen_color"),
        "only basic pen colors 1..=9 are supported",
    )?;
    if text || point {
        require(
            base.pen_style == 1,
            &format!("{path}.pen_style"),
            "only the default text anchor or ordinary point style (1) is supported",
        )?;
    } else {
        require(
            (1..=9).contains(&base.pen_style),
            &format!("{path}.pen_style"),
            "only basic line styles 1..=9 are supported",
        )?;
    }
    if text {
        require(
            base.pen_width == 0,
            &format!("{path}.pen_width"),
            "text uses this field for unsupported dimension flags; expected zero",
        )?;
    } else {
        require(
            base.pen_width <= 500,
            &format!("{path}.pen_width"),
            "expected native width code 0..=500",
        )?;
    }
    Ok(())
}
