#!/usr/bin/env python3
"""Generate original JWW inputs for native JWC experiments; never write JWC.

Only the pinned version-700 header template is supported. Entity fields follow
https://www.jwcad.net/jwdatafmt.txt (JWW, not a JWC specification).
"""
import argparse
import json
import math
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def pack(fmt, *values):
    return struct.pack('<' + fmt, *values)


def cstring(value):
    data = value.encode('utf-16-le')
    length = len(data) // 2
    assert length < 65534
    return b'\xff\xfe\xff' + (pack('B', length) if length < 255 else b'\xff' + pack('H', length)) + data


def serialize(case, template):
    header = bytearray(template)
    # Native template memo is Unicode CRLF; paper at 20, group table at 28.
    assert header[:20] == b'JwwData.' + pack('I', 700) + cstring('\r\n')
    pack_into = lambda fmt, offset, *v: struct.pack_into('<' + fmt, header, offset, *v)
    pack_into('I', 20, case.get('paper_size', 1))
    pack_into('I', 24, case.get('write_layer_group', 0))
    for group in range(16):
        pack_into('d', 36 + group * 148, case.get('scales', {}).get(str(group), 1.0))
        pack_into('I', 32 + group * 148, case.get('write_layers', {}).get(str(group), 0))
        if str(group) in case.get('group_states', {}):
            pack_into('I', 28 + group * 148, case['group_states'][str(group)])
        if str(group) in case.get('group_protect', {}):
            pack_into('I', 44 + group * 148, case['group_protect'][str(group)])
    for layer, state in case.get('layer_states', {}).items():
        pack_into('I', 48 + int(layer) * 8, state)
    for layer, protect in case.get('layer_protect', {}).items():
        pack_into('I', 52 + int(layer) * 8, protect)
    # Pinned header text presets: ten (width, height, spacing, color) rows.
    assert header[16178:16206] == pack('3dI', 2, 2, 0, 1)
    for number, changes in case.get('text_presets', {}).items():
        offset = 16178 + (int(number) - 1) * 28
        assert 1 <= int(number) <= 10
        for key, relative, fmt in [('width', 0, 'd'), ('height', 8, 'd'),
                                   ('spacing', 16, 'd'), ('color', 24, 'I')]:
            if key in changes:
                pack_into(fmt, offset + relative, changes[key])
    # Native template names: 256 layer CStrings, then 16 group CStrings.
    cursor = 28 + 16 * 148 + 84 + 72
    names = []
    start = cursor
    for index in range(272):
        before = cursor
        assert header[cursor:cursor + 3] == b'\xff\xfe\xff'
        cursor += 3
        length = header[cursor]
        cursor += 1
        assert length < 255
        cursor += 2 * length
        names.append(bytes(header[before:cursor]))
    for index, value in case.get('names', {}).items():
        names[int(index)] = cstring(value)
    header[start:cursor] = b''.join(names)
    out = bytearray(header + pack('H', len(case['entities'])))
    class_ids = {}
    next_id = 1
    for entity in case['entities']:
        kind = entity['type']
        name = {'line': 'CDataSen', 'arc': 'CDataEnko', 'point': 'CDataTen', 'text': 'CDataMoji'}[kind]
        if name in class_ids:
            out += pack('H', 0x8000 | class_ids[name])
        else:
            class_ids[name] = next_id
            next_id += 1
            encoded = name.encode('ascii')
            out += pack('HHH', 65535, 700, len(encoded)) + encoded
        next_id += 1
        out += pack('IB5H', entity.get('group', 0), entity.get('pen_style', 1), entity.get('pen_color', 1), entity.get('pen_width', 0), entity.get('layer', 0), entity.get('layer_group', 0), entity.get('flags', 0))
        if kind == 'line':
            out += pack('4d', *entity['xy'])
        elif kind == 'arc':
            out += pack('7dI', *entity['center'], entity['radius'], math.radians(entity.get('start_deg', 0)), math.radians(entity.get('sweep_deg', 360)), math.radians(entity.get('tilt_deg', 0)), entity.get('flatness', 1.), entity.get('full', False))
        elif kind == 'point':
            out += pack('2dI', *entity['xy'], entity.get('temporary', False))
        else:
            x, y = entity.get('xy', [1, 2])
            end_x, end_y = entity.get('end_xy', [x + 12, y])
            out += pack('4dI4d', x, y, end_x, end_y, entity.get('text_type', 1), entity.get('size_x', 3), entity.get('size_y', 3), entity.get('spacing', .5), entity.get('angle', 0))
            out += cstring('ＭＳ ゴシック') + cstring(entity['content'])
    # Native empty block-definition list and trailing DWORD, observed in seed.
    return bytes(out + b'\0' * 6)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'jwc_samples/inputs')
    parser.add_argument('--cases', type=Path, default=ROOT / 'jwc_samples/cases.json')
    args = parser.parse_args()
    cases = json.loads(args.cases.read_text())
    template = (ROOT / 'jwc_samples/inputs/header_700.bin').read_bytes()
    args.output.mkdir(parents=True, exist_ok=True)
    for case in cases:
        (args.output / (case['id'] + '.jww')).write_bytes(serialize(case, template))
    print(f'Generated {len(cases)} JWW inputs; no JWC was generated by this script.')


if __name__ == '__main__':
    main()
