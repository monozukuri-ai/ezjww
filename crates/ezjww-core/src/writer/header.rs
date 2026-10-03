use super::{archive::ArchiveWriter, JwwWriteError, JwwWriteOptions};

// See templates/README.md for provenance and reproducible extraction.
const TEMPLATE: &[u8] = include_bytes!("templates/header_700.bin");
const TEMPLATE_MEMO_END: usize = 20; // pinned UTF-16LE CRLF after signature/version

pub(super) fn write(
    archive: &mut ArchiveWriter,
    options: &JwwWriteOptions,
) -> Result<(), JwwWriteError> {
    archive.bytes(b"JwwData.");
    archive.u32(options.version);
    archive.cstring(&options.memo, "options.memo")?;
    archive.u32(options.paper_size);
    // Copy settings after the paper field. A new memo can have any encoded length.
    archive.bytes(&TEMPLATE[TEMPLATE_MEMO_END + 4..]);
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
    }
}
