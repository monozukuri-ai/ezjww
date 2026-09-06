#!/usr/bin/env python3
"""Check pinned native evidence and the limited P0 geometry/layout hypothesis.

Uses raw native DXF ENTITIES tags as an independent numeric comparison. This is
not a general DXF validator and does not promote any JWC profile automatically.
"""
import argparse
import hashlib
import json
import math
import struct
from pathlib import Path

from inspect_fixture import inspect

ROOT = Path(__file__).resolve().parents[2]


def dxf_entities(data):
    lines = data.decode('cp932').splitlines()
    if len(lines) % 2:
        raise ValueError('Unpaired DXF tag')
    pairs = [(int(lines[i]), lines[i + 1]) for i in range(0, len(lines), 2)]
    if pairs[-1] != (0, 'EOF'):
        raise ValueError('Missing DXF EOF')
    in_entities = False
    result = []
    for code, value in pairs:
        if (code, value) == (2, 'ENTITIES'):
            in_entities = True
        elif in_entities:
            if (code, value) == (0, 'ENDSEC'):
                return result
            if code == 0:
                result.append({'type': value, 'tags': {}})
            else:
                result[-1]['tags'].setdefault(str(code), []).append(value)
    raise ValueError('Missing ENTITIES terminator')


def tag(entity, code):
    return entity['tags'][str(code)][0]


def f32_error(raw, factor):
    # Half of one binary32 ULP, transformed to the JWW paper unit.
    if raw == 0:
        return 2 ** -150 * factor
    return 2 ** (math.floor(math.log2(abs(raw))) - 24) * factor


def check_case(case, directory):
    name = case['id']
    document = inspect((directory / (name + '.jwc')).read_bytes())
    expected = [e for e in case['entities'] if not (e['type'] == 'text' and not e['content'])]
    if 'expected_stored_order' in case:
        expected = [expected[index] for index in case['expected_stored_order']]
    actual = document['records']
    if len(actual) != len(expected):
        raise ValueError(f'{name}: entity count mismatch')
    width, height = document['paper_dimensions']
    if int(document['header_fields'][11]) != case.get('paper_size', 1):
        raise ValueError(f'{name}: paper mismatch')
    for group, scale in enumerate(document['scales']):
        if scale != case.get('scales', {}).get(str(group), 1):
            raise ValueError(f'{name}: scale mismatch')
    expected_order = {'temporary_point': 0, 'line': 1, 'arc': 2, 'text': 3, 'point': 4}
    expected = sorted(expected, key=lambda e: expected_order['temporary_point' if e.get('temporary') else e['type']])
    checks = []
    for source, record in zip(expected, actual):
        kind = 'temporary_point' if source.get('temporary') else source['type']
        if record['type'] != kind:
            raise ValueError(f'{name}: entity type mismatch')
        if kind == 'line':
            values = record['start'] + record['end']
            known = source['xy']
            raw = record['raw_coordinates']
        elif kind in ('arc', 'point'):
            values = record['center']
            known = source['center'] if kind == 'arc' else source['xy']
            raw = record['raw_coordinates'][:2]
        elif kind == 'temporary_point':
            values = record['xy']
            known = source['xy']
            raw = [(values[0] + width / 2) * 518 / width, (values[1] + height / 2) * 518 / width]
        else:
            values = record['start']
            known = source.get('xy', [1, 2])
            raw = record['raw_coordinates'][:2]
            if record['content'] != source['content']:
                raise ValueError(f'{name}: text content mismatch')
            angle = math.degrees(math.atan2(record['end'][1] - record['start'][1], record['end'][0] - record['start'][0]))
            if abs(angle - source.get('angle', 0)) > .002:
                raise ValueError(f'{name}: text angle mismatch')
        errors = [abs(x - y) for x, y in zip(values, known)]
        bounds = [f32_error(x, width / 518) + 1e-9 for x in raw]
        if any(error > bound for error, bound in zip(errors, bounds)):
            raise ValueError(f'{name}: coordinate error {errors} exceeds {bounds}')
        if kind == 'arc':
            if abs(record['radius'] - source['radius']) > f32_error(record['raw_coordinates'][2], width / 518) + 1e-9:
                raise ValueError(f'{name}: radius mismatch')
            if not source.get('full'):
                begin = source.get('start_deg', 0)
                end = (begin + source.get('sweep_deg', 360)) % 360
                if max(abs(record['start_deg'] - begin), abs(record['end_deg'] - end)) > 1 / 65536 + 1e-9:
                    raise ValueError(f'{name}: arc angle mismatch')
        if kind in ('line', 'arc', 'point'):
            attrs = record['attributes']
            if attrs[1] != source.get('pen_color', 1):
                raise ValueError(f'{name}: pen color mismatch')
            if kind != 'point' and attrs[0] != source.get('pen_style', 1):
                raise ValueError(f'{name}: pen style mismatch')
            layer_byte = attrs[0] if kind == 'point' else attrs[2]
            if layer_byte != (source.get('layer_group', 0) << 4 | source.get('layer', 0)):
                raise ValueError(f'{name}: layer mismatch')
        checks.append(dict(type=kind, coordinate_errors=errors, coordinate_error_bounds=bounds))

    dxf = dxf_entities((directory / (name + 'rt.dxf')).read_bytes())
    # Native DXF includes temporary points and changes the origin to the sheet
    # corner. In these native exports every entity uses the current write-group
    # scale, including q054 whose entity belongs to a differently scaled group.
    drawing_pairs = list(zip(expected, actual))
    if len(dxf) != len(drawing_pairs):
        raise ValueError(f'{name}: native DXF entity count mismatch')
    remaining = list(dxf)
    for source, record in drawing_pairs:
        kind = record['type']
        dxf_kind = 'CIRCLE' if kind == 'arc' and source.get('full') else ('POINT' if kind == 'temporary_point' else kind.upper())
        candidates = [e for e in remaining if e['type'] == dxf_kind]
        if not candidates:
            raise ValueError(f'{name}: native DXF type mismatch')
        entity = candidates[0]
        remaining.remove(entity)
        position = record['start'] if kind in ('line', 'text') else (record['xy'] if kind == 'temporary_point' else record['center'])
        scale = document['scales'][case.get('write_layer_group', 0)]
        expected_position = [(position[0] + width / 2) * scale, (position[1] + height / 2) * scale]
        actual_position = [float(tag(entity, 10)), float(tag(entity, 20))]
        # Text insertion is recomputed by Jw_cad from the quantized baseline.
        tolerance = (1e-4 if kind == 'text' else 1e-7) * scale
        if max(abs(x - y) for x, y in zip(expected_position, actual_position)) > tolerance:
            raise ValueError(f'{name}: native DXF insertion/center mismatch')
        if kind == 'text' and tag(entity, 1) != source['content']:
            raise ValueError(f'{name}: native DXF text mismatch')
        if kind == 'line':
            expected_end = [(record['end'][0] + width / 2) * scale,
                            (record['end'][1] + height / 2) * scale]
            actual_end = [float(tag(entity, 11)), float(tag(entity, 21))]
            if max(abs(x - y) for x, y in zip(expected_end, actual_end)) > tolerance:
                raise ValueError(f'{name}: native DXF line endpoint mismatch')
        if kind == 'arc':
            if abs(float(tag(entity, 40)) - record['radius'] * scale) > tolerance:
                raise ValueError(f'{name}: native DXF radius mismatch')
            if dxf_kind == 'ARC':
                if max(abs(float(tag(entity, 50)) - record['start_deg']),
                       abs(float(tag(entity, 51)) - record['end_deg'])) > 1e-7:
                    raise ValueError(f'{name}: native DXF arc angles mismatch')
        if kind == 'text':
            angle = math.degrees(math.atan2(record['end'][1] - record['start'][1],
                                           record['end'][0] - record['start'][0]))
            if abs(float(tag(entity, 50)) - angle) > 1e-7:
                raise ValueError(f'{name}: native DXF text angle mismatch')
    return dict(id=name, role=case['role'], boundary_check=True,
                known_geometry_check=True, native_dxf_entities_check=True,
                native_dxf_full_validation=False, profile_promoted=False,
                source_entities=len(case['entities']), stored_entities=len(actual),
                dxf_entities=len(dxf), geometry=checks)


def negative_checks(data, text_data, multi_text_data=None):
    samples = [b'', data[:39], data[:2421], data[:-1], data + b'\0']
    corrupt_count = bytearray(data)
    corrupt_count[200] = ord('9')
    samples.append(bytes(corrupt_count))
    nonfinite = bytearray(data)
    struct.pack_into('<f', nonfinite, 2421, math.nan)
    samples.append(bytes(nonfinite))
    missing_terminator = bytearray(text_data)
    missing_terminator[-2305] = ord('X')
    samples.append(bytes(missing_terminator))
    if multi_text_data is not None:
        # A valid shared string pool must not turn bad offsets into accidental
        # CP932 text. Corrupt the second reference, the pool and fixed records.
        for relative in (0, 5, 0x3fffffff):
            broken = bytearray(multi_text_data)
            struct.pack_into('<I', broken, 2421 + 24 + 16, 0x40000000 | relative)
            samples.append(bytes(broken))
        samples.append(multi_text_data[:2421 + 47] + multi_text_data[-2304:])
        samples.append(multi_text_data[:-2305] + b'X' + multi_text_data[-2304:])
    for broken in samples:
        try:
            inspect(broken)
        except (ValueError, KeyError, struct.error):
            continue
        raise AssertionError('Damaged input passed the boundary check')
    return len(samples)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--role', choices=['candidate', 'holdout', 'all'], default='candidate')
    args = parser.parse_args()
    cases = json.loads((ROOT / 'jwc_samples/cases.json').read_text())
    report = []
    for case in cases:
        if args.role != 'all' and case['role'] != args.role:
            continue
        report.append(check_case(case, args.directory))
    negative = negative_checks((args.directory / 'q001.jwc').read_bytes(),
                               (args.directory / 'q030.jwc').read_bytes(),
                               (args.directory / 'r000.jwc').read_bytes()
                               if (args.directory / 'r000.jwc').exists() else None)
    result = dict(profile_promoted=False, tested_role=args.role, cases=report,
                  damaged_input_checks=negative,
                  inspector_sha256=hashlib.sha256((Path(__file__).parent / 'inspect_fixture.py').read_bytes()).hexdigest())
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    print(f'{len(report)} native cases and {negative} damaged-input boundary checks passed.')


if __name__ == '__main__':
    main()
