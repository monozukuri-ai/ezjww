use std::collections::HashMap;

use super::JwwWriteError;

const MAX_PID: u32 = 0x3fff_fffe;

/// One class/object PID table for the entire CArchive, including future blocks.
pub(super) struct ArchiveWriter {
    bytes: Vec<u8>,
    classes: HashMap<(&'static str, u16), u32>,
    next_pid: u32,
}

impl ArchiveWriter {
    pub(super) fn new() -> Self {
        Self {
            bytes: Vec::new(),
            classes: HashMap::new(),
            next_pid: 1,
        }
    }

    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    pub(super) fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    pub(super) fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }
    pub(super) fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }
    pub(super) fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }
    pub(super) fn f64(&mut self, value: f64) {
        self.bytes(&value.to_le_bytes());
    }

    pub(super) fn count(&mut self, count: usize, field: &str) -> Result<(), JwwWriteError> {
        // DWORD 0xffffffff is reserved for the MFC 64-bit count extension.
        if count >= u32::MAX as usize {
            return Err(JwwWriteError::LimitExceeded {
                field: field.into(),
                maximum: u64::from(u32::MAX - 1),
            });
        }
        if count < 0xffff {
            self.u16(count as u16);
        } else {
            self.u16(0xffff);
            self.u32(count as u32);
        }
        Ok(())
    }

    pub(super) fn cstring(&mut self, value: &str, field: &str) -> Result<(), JwwWriteError> {
        let length = value.encode_utf16().count();
        if length > u32::MAX as usize {
            return Err(JwwWriteError::LimitExceeded {
                field: field.into(),
                maximum: u64::from(u32::MAX),
            });
        }
        self.bytes(&[0xff, 0xfe, 0xff]); // Unicode marker
        if length < 0xff {
            self.u8(length as u8);
        } else if length < 0xfffe {
            self.u8(0xff);
            self.u16(length as u16);
        } else {
            self.u8(0xff);
            self.u16(0xffff);
            self.u32(length as u32);
        }
        for unit in value.encode_utf16() {
            self.u16(unit);
        }
        Ok(())
    }

    /// Reserve the object's identity before its payload/nested lists are written.
    pub(super) fn object(&mut self, class: &'static str, schema: u16) -> Result<(), JwwWriteError> {
        if let Some(&pid) = self.classes.get(&(class, schema)) {
            if pid < 0x7fff {
                self.u16(0x8000 | pid as u16);
            } else {
                self.u16(0x7fff);
                self.u32(0x8000_0000 | pid);
            }
        } else {
            if !class.is_ascii() || class.len() > u16::MAX as usize {
                return Err(JwwWriteError::invalid(
                    "archive.class",
                    "expected a short ASCII class name",
                ));
            }
            let pid = self.reserve_pid()?;
            self.classes.insert((class, schema), pid);
            self.u16(0xffff);
            self.u16(schema);
            self.u16(class.len() as u16);
            self.bytes(class.as_bytes());
        }
        self.reserve_pid()?;
        Ok(())
    }

    fn reserve_pid(&mut self) -> Result<u32, JwwWriteError> {
        if self.next_pid > MAX_PID {
            return Err(JwwWriteError::LimitExceeded {
                field: "archive.pid".into(),
                maximum: u64::from(MAX_PID),
            });
        }
        let pid = self.next_pid;
        self.next_pid += 1;
        Ok(pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::Reader;

    #[test]
    fn numeric_values_are_little_endian() {
        let mut a = ArchiveWriter::new();
        a.u8(7);
        a.u16(0x1234);
        a.u32(0x12345678);
        a.f64(1.0);
        assert_eq!(
            a.into_bytes(),
            [7, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12, 0, 0, 0, 0, 0, 0, 0xf0, 0x3f]
        );
    }

    #[test]
    fn count_escape_and_limit() {
        for (n, expected) in [
            (65534, vec![0xfe, 0xff]),
            (65535, vec![0xff, 0xff, 0xff, 0xff, 0, 0]),
            (65536, vec![0xff, 0xff, 0, 0, 1, 0]),
        ] {
            let mut a = ArchiveWriter::new();
            a.count(n, "entities").unwrap();
            assert_eq!(a.into_bytes(), expected);
        }
        assert!(ArchiveWriter::new()
            .count(u32::MAX as usize, "entities")
            .is_err());
    }

    #[test]
    fn unicode_length_boundaries_and_surrogate_pairs() {
        for (length, prefix) in [
            (0, vec![0]),
            (254, vec![254]),
            (255, vec![255, 255, 0]),
            (65533, vec![255, 253, 255]),
            (65534, vec![255, 255, 255, 254, 255, 0, 0]),
            (65535, vec![255, 255, 255, 255, 255, 0, 0]),
        ] {
            let value = "あ".repeat(length);
            let mut a = ArchiveWriter::new();
            a.cstring(&value, "text").unwrap();
            let bytes = a.into_bytes();
            assert_eq!(&bytes[3..3 + prefix.len()], &prefix);
            assert_eq!(Reader::new(&bytes).read_cstring().unwrap(), value);
        }
        let mut a = ArchiveWriter::new();
        a.cstring("A𠮷", "text").unwrap();
        assert_eq!(
            a.into_bytes(),
            [255, 254, 255, 3, 65, 0, 0x42, 0xd8, 0xb7, 0xdf]
        );
    }

    #[test]
    fn class_and_object_ids_share_the_same_sequence() {
        let mut a = ArchiveWriter::new();
        a.object("CDataSen", 700).unwrap(); // class 1, object 2
        a.object("CDataSen", 700).unwrap(); // object 3
        a.object("CDataList", 700).unwrap(); // class 4, object 5 (before children)
        a.object("CDataSen", 700).unwrap(); // child 6
        a.object("CDataList", 700).unwrap(); // object 7
        let bytes = a.into_bytes();
        assert_eq!(&bytes[14..16], &[1, 128]);
        assert_eq!(&bytes[bytes.len() - 4..], &[1, 128, 4, 128]);
    }

    #[test]
    fn late_class_uses_extended_pid_reference() {
        let mut a = ArchiveWriter::new();
        for _ in 0..32765 {
            a.object("CDataSen", 700).unwrap();
        }
        a.object("CDataList", 700).unwrap(); // class PID 0x7fff
        a.object("CDataList", 700).unwrap();
        let bytes = a.into_bytes();
        assert_eq!(
            &bytes[bytes.len() - 6..],
            &[0xff, 0x7f, 0xff, 0x7f, 0, 0x80]
        );
    }

    #[test]
    fn archive_pid_limit_is_checked() {
        let mut a = ArchiveWriter::new();
        a.next_pid = MAX_PID;
        assert!(matches!(
            a.object("CDataSen", 700),
            Err(JwwWriteError::LimitExceeded { .. })
        ));
    }
}
