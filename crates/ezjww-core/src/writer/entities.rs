use super::{archive::ArchiveWriter, JwwWriteError};
use crate::model::Entity;

pub(super) fn write(archive: &mut ArchiveWriter, entity: &Entity) -> Result<(), JwwWriteError> {
    let Entity::Line(line) = entity else {
        return Err(JwwWriteError::invalid(
            "entities",
            "only LINE is supported by this writer",
        ));
    };
    archive.object("CDataSen", 700)?;
    let base = &line.base;
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
    for value in [line.start_x, line.start_y, line.end_x, line.end_y] {
        archive.f64(value);
    }
    Ok(())
}
