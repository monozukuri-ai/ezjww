use super::{archive::ArchiveWriter, JwwWriteError};
use crate::model::{Entity, EntityBase};

pub(super) fn write(archive: &mut ArchiveWriter, entity: &Entity) -> Result<(), JwwWriteError> {
    match entity {
        Entity::Line(line) => {
            archive.object("CDataSen", 700)?;
            base(archive, &line.base);
            doubles(
                archive,
                &[line.start_x, line.start_y, line.end_x, line.end_y],
            );
        }
        Entity::Arc(arc) => {
            archive.object("CDataEnko", 700)?;
            base(archive, &arc.base);
            doubles(
                archive,
                &[
                    arc.center_x,
                    arc.center_y,
                    arc.radius,
                    arc.start_angle,
                    arc.arc_angle,
                    arc.tilt_angle,
                    arc.flatness,
                ],
            );
            archive.u32(u32::from(arc.is_full_circle));
        }
        Entity::Point(point) => {
            archive.object("CDataTen", 700)?;
            base(archive, &point.base);
            doubles(archive, &[point.x, point.y]);
            archive.u32(0); // ordinary permanent point; marker fields are absent
        }
        Entity::Text(text) => {
            archive.object("CDataMoji", 700)?;
            base(archive, &text.base);
            doubles(
                archive,
                &[text.start_x, text.start_y, text.end_x, text.end_y],
            );
            archive.u32(text.text_type);
            doubles(
                archive,
                &[text.size_x, text.size_y, text.spacing, text.angle],
            );
            archive.cstring(&text.font_name, "text.font_name")?;
            archive.cstring(&text.content, "text.content")?;
        }
        _ => {
            return Err(JwwWriteError::invalid(
                "entities",
                "unsupported entity type",
            ))
        }
    }
    Ok(())
}

fn base(archive: &mut ArchiveWriter, base: &EntityBase) {
    archive.u32(base.group);
    archive.u8(base.pen_style);
    for value in [
        base.pen_color,
        base.pen_width,
        base.layer,
        base.layer_group,
        base.flag,
    ] {
        archive.u16(value);
    }
}

fn doubles(archive: &mut ArchiveWriter, values: &[f64]) {
    for value in values {
        archive.f64(*value);
    }
}
