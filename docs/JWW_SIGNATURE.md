# JWW file signature

A binary JWW file starts at byte offset `0` with the following exact eight-byte
signature:

| Representation | Value |
|---|---|
| ASCII | `JwwData.` |
| Python bytes | `b"JwwData."` |
| Hex | `4A 77 77 44 61 74 61 2E` |
| Length | 8 bytes |

The comparison is case-sensitive. A BOM, whitespace, or any other preamble is
not allowed before the signature. The filename and extension are not part of
the test.

An independent lightweight detector can implement the check without importing
or initializing `ezjww`:

```python
JWW_SIGNATURE = b"JwwData."


def has_jww_signature(head: bytes) -> bool:
    return len(head) >= len(JWW_SIGNATURE) and head.startswith(JWW_SIGNATURE)
```

The Rust core exposes the same check as `ezjww_core::is_jww_signature`. Python
`ezjww.is_jww_file(path)` reads exactly the first eight bytes and applies this
comparison; TypeScript `isJwwFile(bytes)` applies it to the supplied byte
buffer.

If version sniffing is also required, bytes `8..12` are an unsigned 32-bit
little-endian JWW version number. Read them only after the signature matches
and at least 12 bytes are available.

The signature is a necessary format check, not full-file validation. A matching
prefix can still be followed by a truncated or malformed payload; use
`read_header` or `read_document` when structural validation is required.
