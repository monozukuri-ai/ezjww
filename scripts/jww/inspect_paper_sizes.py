#!/usr/bin/env python3
"""Print Jw_cad 10.02.1's paper-size constants from a local executable."""

import argparse
import hashlib
import json
import struct
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    args = parser.parse_args()
    data = args.executable.read_bytes()
    sha = hashlib.sha256(data).hexdigest()
    if sha != "95e6b11c4ee014e0079f288429ae2c6e5eed41141a963e4b8770d2e8ead87acf":
        raise ValueError("unverified executable; offsets are version-specific")
    result = {
        "application_sha256": sha,
        "width_offset": "0x5d39b8",
        "height_offset": "0x5d3a58",
        "sizes": [],
    }
    for code, label in enumerate(("2A", "3A", "4A", "5A", "10m", "50m", "100m"), 8):
        result["sizes"].append(
            {
                "code": code,
                "label": label,
                "width_mm": struct.unpack_from("<d", data, 0x5D39B8 + code * 8)[0],
                "height_mm": struct.unpack_from("<d", data, 0x5D3A58 + code * 8)[0],
            }
        )
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
