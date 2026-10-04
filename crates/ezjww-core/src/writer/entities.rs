use super::{archive::ArchiveWriter, JwwWriteError};
use crate::model::{BlockDef, Entity, EntityBase, Line, Point, Text};

pub(super) fn write(archive: &mut ArchiveWriter, entity: &Entity) -> Result<(), JwwWriteError> {
    match entity {
        Entity::Line(line) => {
            archive.object("CDataSen", 700)?;
            write_line(archive, line)?;
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
            write_point(archive, point)?;
        }
        Entity::Text(text) => {
            archive.object("CDataMoji", 700)?;
            write_text(archive, text)?;
        }
        Entity::Solid(s) => {
            archive.object("CDataSolid", 700)?;
            base(archive, &s.base);
            doubles(
                archive,
                &[
                    s.point1_x, s.point1_y, s.point4_x, s.point4_y, s.point2_x, s.point2_y,
                    s.point3_x, s.point3_y,
                ],
            );
            if let Some(color) = s.color {
                archive.u32(color);
            }
        }
        Entity::CircleSolid(s) => {
            archive.object("CDataSolid", 700)?;
            base(archive, &s.base);
            doubles(
                archive,
                &[
                    s.center_x,
                    s.center_y,
                    s.radius,
                    s.flatness,
                    s.tilt_angle,
                    s.start_angle,
                    s.arc_angle,
                    s.solid_mode,
                ],
            );
            if let Some(color) = s.color {
                archive.u32(color);
            }
        }
        Entity::Block(b) => {
            archive.object("CDataBlock", 700)?;
            base(archive, &b.base);
            doubles(
                archive,
                &[b.ref_x, b.ref_y, b.scale_x, b.scale_y, b.rotation],
            );
            archive.u32(b.def_number);
        }
        Entity::Dimension(d) => {
            archive.object("CDataSunpou", 700)?;
            base(archive, &d.base);
            write_line(archive, &d.line)?;
            write_text(archive, &d.text)?;
            archive.u16(d.sxf_mode.unwrap());
            for line in &d.aux_lines {
                write_line(archive, line)?;
            }
            for point in &d.aux_points {
                write_point(archive, point)?;
            }
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

fn write_line(archive: &mut ArchiveWriter, line: &Line) -> Result<(), JwwWriteError> {
    base(archive, &line.base);
    doubles(
        archive,
        &[line.start_x, line.start_y, line.end_x, line.end_y],
    );
    Ok(())
}

fn write_point(archive: &mut ArchiveWriter, point: &Point) -> Result<(), JwwWriteError> {
    base(archive, &point.base);
    doubles(archive, &[point.x, point.y]);
    archive.u32(u32::from(point.is_temporary));
    if point.base.pen_style == 100 {
        archive.u32(point.code);
        doubles(archive, &[point.angle, point.scale]);
    }
    Ok(())
}

fn write_text(archive: &mut ArchiveWriter, text: &Text) -> Result<(), JwwWriteError> {
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
    Ok(())
}

pub(super) fn block_definition(
    archive: &mut ArchiveWriter,
    definition: &BlockDef,
) -> Result<(), JwwWriteError> {
    archive.object("CDataList", 700)?;
    base(archive, &definition.base);
    archive.u32(definition.number);
    archive.u32(u32::from(definition.is_referenced));
    archive.u32(0); // deterministic CTime; no runtime clock dependency
    archive.cstring(&definition.name, "block_defs.name")?;
    archive.count(definition.entities.len(), "block_defs.entities")?;
    for entity in &definition.entities {
        write(archive, entity)?;
    }
    Ok(())
}
