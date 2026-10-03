use super::{archive::ArchiveWriter, JwwWriteError, JwwWriteOptions};

// See templates/README.md for provenance and reproducible extraction.
const TEMPLATE: &[u8] = include_bytes!("templates/header_700.bin");
#[cfg(test)]
const TEMPLATE_MEMO_END: usize = 20; // pinned UTF-16LE CRLF after signature/version
const TABLE_END: usize = 28 + 16 * 148;
const NAMES_START: usize = TABLE_END + 84 + 72;
// 272 Unicode CStrings: all empty except "0" and "Defpoints".
const NAMES_END: usize = NAMES_START + 272 * 4 + 2 * (1 + 9);

pub(super) fn write(
    archive: &mut ArchiveWriter,
    options: &JwwWriteOptions,
) -> Result<(), JwwWriteError> {
    archive.bytes(b"JwwData.");
    archive.u32(options.version);
    archive.cstring(&options.memo, "options.memo")?;
    archive.u32(options.paper_size);
    archive.u32(options.write_layer_group);
    for group in &options.layer_groups {
        archive.u32(group.state);
        archive.u32(group.write_layer);
        archive.f64(group.scale);
        archive.u32(group.protect);
        for layer in &group.layers {
            archive.u32(layer.state);
            archive.u32(layer.protect);
        }
    }
    archive.bytes(&TEMPLATE[TABLE_END..NAMES_START]);
    for (g, group) in options.layer_groups.iter().enumerate() {
        for (l, layer) in group.layers.iter().enumerate() {
            archive.cstring(
                &layer.name,
                &format!("options.layer_groups[{g}].layers[{l}].name"),
            )?;
        }
    }
    for (g, group) in options.layer_groups.iter().enumerate() {
        archive.cstring(&group.name, &format!("options.layer_groups[{g}].name"))?;
    }
    archive.bytes(&TEMPLATE[NAMES_END..]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_has_a_complete_header_and_no_geometry() {
        assert_eq!(TEMPLATE.len(), 16558);
        assert_eq!(
            &TEMPLATE[12..TEMPLATE_MEMO_END],
            &[255, 254, 255, 2, 13, 0, 10, 0]
        );
        assert_eq!(
            crate::header_layout::header_end_v700(TEMPLATE).unwrap(),
            TEMPLATE.len()
        );
        let header = crate::parse_header(TEMPLATE).unwrap();
        assert_eq!(header.version, 700);
        assert_eq!(header.memo, "\r\n");
        assert!(header.layer_groups.iter().all(|group| group.scale == 1.0));
        let mut reader = crate::reader::Reader::new(TEMPLATE);
        reader.skip(NAMES_START).unwrap();
        for i in 0..272 {
            assert_eq!(
                reader.read_cstring().unwrap(),
                match i {
                    0 => "0",
                    1 => "Defpoints",
                    _ => "",
                }
            );
        }
        assert_eq!(reader.bytes_read(), NAMES_END);
    }
}
