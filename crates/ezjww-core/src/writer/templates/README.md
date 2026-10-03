# Version-700 default header

`header_700.bin` is an original drawing header under the repository MIT license.
It contains settings, not geometry, fonts or application code.

- Size: 16,558 bytes.
- SHA-256: `7d66238bef9a66e3030a39a58936ba6aedc2189a3ebe46d582c908bf43622572`.
- Origin: the native header documented in `jwc_samples/manifest.json`, made by
  importing an original DXF line into Jw_cad and saving JWW. The sixteen group
  scales were set to 1 for the original controlled fixture experiments.
- The memo is UTF-16LE CRLF and the paper size is A1. The writer replaces those
  two fields; its defaults are an empty memo and A3. All other settings are fixed.
- `header_layout.rs` traverses the documented full header including variable-length
  names. Its end must equal the template length.

The original header file was untracked, but the same bytes are recoverable from
the tracked `jwc_samples/inputs/q000.jww`: remove its final eight zero bytes
(empty entity list WORD, empty block list WORD, image count DWORD).
From the repository root:

```sh
python scripts/jww/check_fixtures.py
python scripts/jww/check_fixtures.py --extract-header /tmp/header_700.bin
```

The checker verifies the pinned hash and the copy embedded in the Rust crate.
Recreating a new native header is a separate change: use an original drawing,
record application and output hashes, verify settings, and rerun the native
acceptance cases. Do not silently replace this template with a user's drawing.

References: [Jw_cad format](https://www.jwcad.net/jwdatafmt.txt) and
[MFC archive format](https://learn.microsoft.com/en-us/cpp/mfc/tn002-persistent-object-data-format?view=msvc-170).
