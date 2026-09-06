# JWC test fixtures

Original controlled drawings for parser and conversion regression tests.
The corpus metadata records input values, provenance, and file hashes.

- `cases.json` and `followup_cases.json`: drawing definitions.
- `inputs/`: generated JWW inputs and header templates.
- `generated/`: saved JWC files and reference JWW/DXF outputs.
- `manifest.json`: provenance, usage terms, sizes, and hashes.
- `expected_dxf.json`: archived P4 converter-output hashes for both coordinate
  spaces and DXF versions. These pin the existing converter behavior; they are
  not hashes of native Jw_cad exports. The source report hash is retained and
  the expected values are copied without regenerating outputs.

Drawings and input-generation code follow the repository's [MIT license](../LICENSE).
Third-party drawings, applications, and fonts are not included.

Run the integrity check from the repository root:

```bash
python scripts/jwc/check_corpus.py
```

These files remain in this directory because regression tests read them here.
For supported input behavior, see [JWC support and limits](../docs/JWC_FORMAT.md).
