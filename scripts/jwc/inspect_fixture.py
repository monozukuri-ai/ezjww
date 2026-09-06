#!/usr/bin/env python3
"""Inspect the P0 layout hypothesis, without declaring a supported JWC profile.

This research utility is not connected to ezjww's product parser. Unknown header
and attribute fields stay raw. Complete byte consumption is only a boundary check.
"""
import argparse
import hashlib
import json
import math
import struct
from pathlib import Path

SIGNATURE = b'jw_cad(c)data.......a.f.m...............'
BODY = 2421
TAIL = 2304
PAPERS = {0: (1189., 841.), 1: (841., 594.), 2: (594., 420.),
          3: (420., 297.), 4: (297., 210.)}


def inspect(data):
    if len(data) < BODY + TAIL or data[:40] != SIGNATURE:
        raise ValueError('Not a complete file of the observed layout')
    for end in (199, 399, 599, 799):
        if data[end] != 10:
            raise ValueError(f'Missing observed header newline at {end}')
    fields = data[200:399].split(b'\0')[0].rstrip().decode('ascii').split(',')
    counts = list(map(int, fields[:5]))
    if len(fields) not in (31, 32) or any(n < 0 or n > 100000 for n in counts):
        raise ValueError('Unexpected header fields or counts')
    if counts[4] > 100:
        raise ValueError('Temporary point count outside investigated array hypothesis')
    width, height = PAPERS[int(fields[11])]
    factor = width / 518.
    scales = struct.unpack_from('<16f', data, 1797)
    if any(not math.isfinite(s) or s <= 0 for s in scales):
        raise ValueError('Invalid scale')
    records = []
    cursor = BODY

    def floats(offset, count):
        values = struct.unpack_from('<' + 'f' * count, data, offset)
        if not all(math.isfinite(x) for x in values):
            raise ValueError(f'Non-finite coordinates at {offset}')
        return values

    def xy(x, y):
        return [x * factor - width / 2, y * factor - height / 2]

    # Temporary points live in two fixed arrays, using indices 1..count.
    for index in range(1, counts[4] + 1):
        x = floats(800 + index * 4, 1)[0]
        y = floats(1204 + index * 4, 1)[0]
        records.append(dict(type='temporary_point', offsets=[800 + index * 4, 1204 + index * 4],
                            raw_coordinates=[x, y], xy=xy(x, y), packed_layer=data[1608 + index]))

    # Order observed in mixed native fixture q070 (different input order).
    string_pool_offset = BODY + counts[0] * 22 + counts[1] * 32 + counts[2] * 24
    string_cursor = string_pool_offset
    for kind, count, fixed in [('line', counts[0], 22), ('arc', counts[1], 32), ('text', counts[2], 24), ('point', counts[3], 12)]:
        if kind == 'point':
            cursor = string_cursor
        for _ in range(count):
            if cursor + fixed > len(data) - TAIL:
                raise ValueError(f'Truncated {kind} at {cursor}')
            start = cursor
            record = dict(type=kind, offset=start)
            if kind in ('line', 'text'):
                coords = floats(start, 4)
                record.update(raw_coordinates=coords, start=xy(*coords[:2]), end=xy(*coords[2:]))
            else:
                coords = floats(start, 3 if kind == 'arc' else 2)
                record.update(raw_coordinates=coords, center=xy(*coords[:2]))
            if kind == 'line':
                record['attributes'] = list(data[start + 16:start + 22])
            elif kind == 'arc':
                flatness, begin, end, tilt = struct.unpack_from('<Hiii', data, start + 12)
                record.update(radius=coords[2] * factor, flatness_raw=flatness,
                              start_deg=begin / 65536, end_deg=end / 65536, tilt_deg=tilt / 65536,
                              attributes=list(data[start + 26:start + 32]))
            elif kind == 'point':
                record['attributes'] = list(data[start + 8:start + 12])
            else:
                record['attributes'] = list(data[start + 16:start + 24])
            cursor += fixed
            if kind == 'text':
                pointer = struct.unpack_from('<I', data, start + 16)[0]
                relative = pointer & 0x3fffffff
                # Observed records refer to consecutive NUL-terminated CP932
                # strings after ALL fixed text records, before normal points.
                if string_pool_offset + relative != string_cursor:
                    raise ValueError(f'Non-contiguous or invalid string reference at {start + 16}')
                end = data.find(b'\0', string_cursor, len(data) - TAIL - counts[3] * 12)
                if end < 0:
                    raise ValueError(f'Unterminated text at {string_cursor}')
                raw = data[string_cursor:end]
                record.update(content=raw.decode('cp932'), encoded_length=len(raw),
                              string_offset=string_cursor, string_relative_offset=relative,
                              string_flags=pointer & 0xc0000000)
                string_cursor = end + 1
            record['size'] = cursor - start
            records.append(record)
    if cursor + TAIL != len(data):
        raise ValueError(f'Unexpected remainder: body ends at {cursor}, total {len(data)}')
    return dict(status='observations_only', profile_promoted=False,
                sha256=hashlib.sha256(data).hexdigest(), size=len(data), header_fields=fields,
                counts=counts, paper_dimensions=[width, height], scales=scales,
                csv_field_count=len(fields), string_pool_offset=string_pool_offset,
                string_pool_size=string_cursor - string_pool_offset,
                text_presets={name: list(struct.unpack_from('<11H', data, offset))
                              for name, offset in [('color', 1709), ('width_tenths', 1731),
                                                   ('height_tenths', 1753), ('spacing_tenths', 1775)]},
                records=records, trailer_offset=cursor,
                trailer_sha256=hashlib.sha256(data[cursor:]).hexdigest())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('files', type=Path, nargs='+')
    args = parser.parse_args()
    for path in args.files:
        result = inspect(path.read_bytes())
        print(json.dumps(dict(file=str(path), **result), ensure_ascii=False))


if __name__ == '__main__':
    main()
