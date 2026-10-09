//! Minimal standard base64 (RFC 4648 with padding) for embedded image bytes in
//! the JSON-shaped writer input and document DTOs. No external dependency.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode bytes as padded standard base64.
pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(triple >> 18) as usize & 63] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(triple >> 6) as usize & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[triple as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn value(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some((byte - b'A') as u32),
        b'a'..=b'z' => Some((byte - b'a') as u32 + 26),
        b'0'..=b'9' => Some((byte - b'0') as u32 + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Decode padded standard base64; ASCII whitespace between groups is ignored.
pub fn decode(text: &str) -> Result<Vec<u8>, String> {
    let bytes: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if bytes.len() & 3 != 0 {
        return Err("expected base64 text whose length is a multiple of 4".into());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for (index, group) in bytes.chunks(4).enumerate() {
        let last = index + 1 == bytes.len() / 4;
        let padding = group.iter().rev().take_while(|b| **b == b'=').count();
        if padding > 2 || (padding > 0 && !last) {
            return Err("unexpected base64 padding".into());
        }
        let mut acc = 0u32;
        for &b in &group[..4 - padding] {
            acc = (acc << 6)
                | value(b).ok_or_else(|| format!("invalid base64 character {:?}", b as char))?;
        }
        acc <<= 6 * padding as u32;
        out.push((acc >> 16) as u8);
        if padding < 2 {
            out.push((acc >> 8) as u8);
        }
        if padding < 1 {
            out.push(acc as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};

    #[test]
    fn round_trips_every_padding_length() {
        for data in [
            &b""[..],
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"fooba",
            b"foobar",
            &[0xff, 0x00, 0x1f],
        ] {
            let text = encode(data);
            assert_eq!(decode(&text).unwrap(), data, "{text}");
        }
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(decode("Zm9v\nYmFy").unwrap(), b"foobar");
    }

    #[test]
    fn rejects_malformed_text() {
        assert!(decode("Zm9").is_err());
        assert!(decode("Zm9v*mFy").is_err());
        assert!(decode("Zm==Zm9v").is_err());
        assert!(decode("Zm9v===").is_err());
    }
}
