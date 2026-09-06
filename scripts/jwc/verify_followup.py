#!/usr/bin/env python3
"""Validate P0 follow-up evidence, including native JWW attributes and ellipses.

The JWW reader must be built from the recorded repository revision. DXF checks
remain limited to ENTITIES; this does not certify any production JWC profile.
"""
import argparse
import hashlib
import json
import math
import sys
from pathlib import Path

from inspect_fixture import inspect
from verify_fixtures import check_case, dxf_entities, f32_error, tag

ROOT = Path(__file__).resolve().parents[2]


def near(actual, expected, tolerance, label):
    if not math.isfinite(actual) or abs(actual - expected) > tolerance:
        raise ValueError(f'{label}: {actual} != {expected} (tolerance {tolerance})')


def native_jww_check(document, native, allowed_flag_changes=None):
    allowed_flag_changes = allowed_flag_changes or {}
    metadata = {entry['entity_index'] for entry in native['metadata_settings']}
    remaining = [entity for index, entity in enumerate(native['entities']) if index not in metadata]
    if len(remaining) != len(document['records']):
        raise ValueError(f'Native JWW count: {len(remaining)} != {len(document["records"])}')
    text_attributes = []
    for record in document['records']:
        kind = record['type']
        native_kind = 'POINT' if kind == 'temporary_point' else kind.upper()
        if kind == 'arc':
            native_kind = 'CIRCLE' if record['start_deg'] == record['end_deg'] else 'ARC'
        candidates = [entity for entity in remaining if entity['type'] == native_kind
                      and (native_kind != 'POINT' or entity['is_temporary'] == (kind == 'temporary_point'))]
        if not candidates:
            raise ValueError(f'Native JWW missing {kind}')
        entity = candidates[0]
        remaining.remove(entity)
        if kind in ('line', 'text'):
            values = list(record['start'])
            keys = ['start_x', 'start_y']
            if kind == 'line':
                values += record['end']
                keys += ['end_x', 'end_y']
        else:
            values = record['xy'] if kind == 'temporary_point' else record['center']
            keys = ['center_x', 'center_y'] if kind == 'arc' else ['x', 'y']
        for key, value in zip(keys, values):
            near(entity[key], value, 1e-4 if kind == 'text' else 1e-7, f'{kind}.{key}')
        attrs = record.get('attributes', [])
        layer = record['packed_layer'] if kind == 'temporary_point' else (
            attrs[0] if kind == 'point' else attrs[5] if kind == 'text' else attrs[2])
        if (entity['base']['layer_group'] << 4 | entity['base']['layer']) != layer:
            raise ValueError(f'{kind}: native JWW layer mismatch')
        if kind in ('line', 'arc', 'point'):
            if entity['base']['pen_color'] != attrs[1]:
                raise ValueError(f'{kind}: native JWW color mismatch')
            if kind != 'point' and entity['base']['pen_style'] != attrs[0]:
                raise ValueError(f'{kind}: native JWW style mismatch')
            flags = attrs[-2] | attrs[-1] << 8
            expected_flag = allowed_flag_changes.get(kind, {}).get(str(flags), flags)
            if entity['base']['flag'] != expected_flag:
                raise ValueError(f'{kind}: native JWW flags mismatch')
        if kind == 'arc':
            near(entity['radius'], record['radius'], 1e-7, 'radius')
            near(entity['flatness'], record['flatness_raw'] / 10000, 1e-10, 'flatness')
            near(entity['tilt_angle'], math.radians(record['tilt_deg']), 1e-10, 'tilt')
        elif kind == 'text':
            if entity['content'] != record['content']:
                raise ValueError('Native JWW text mismatch')
            number = attrs[4]
            if entity['text_type'] != number:
                raise ValueError('Native JWW text preset mismatch')
            for key, table in [('size_x', 'width_tenths'), ('size_y', 'height_tenths'),
                               ('spacing', 'spacing_tenths')]:
                near(entity[key], document['text_presets'][table][number] / 10, 1e-7, key)
            if entity['base']['pen_color'] != document['text_presets']['color'][number]:
                raise ValueError('Native JWW text preset color mismatch')
            text_attributes.append({key: entity[key] for key in ['text_type', 'size_x', 'size_y', 'spacing']})
    return dict(entity_count=len(document['records']), metadata_entities_excluded=len(metadata),
                geometry_and_attributes=True, text_attributes=text_attributes,
                allowed_flag_changes=allowed_flag_changes)


def header_check(data, native):
    groups = native['header']['layer_groups']
    entries = groups + [layer for group in groups for layer in group['layers']]
    for index, entry in enumerate(entries):
        edit, visible = data[1861 + index], data[2133 + index]
        if edit & ~3 or visible not in (0, 1):
            raise ValueError('Uninvestigated header state bits')
        state = (2 if edit & 1 else 1) if visible else 0
        if min(entry['state'], 2) != state or entry['protect'] != (edit >> 1):
            raise ValueError(f'Native JWW header state mismatch at index {index}')
    return dict(state_and_protection_entries=len(entries))


def ellipse_check(case, document, dxf_path):
    # These controlled cases contain one full ellipse. Jw_cad exports it as a
    # closed chain of LINEs. Check every endpoint against the rotated conic,
    # continuity and a complete winding; do not compare only bounding boxes.
    if len(case['entities']) != 1 or not case['entities'][0].get('full'):
        raise ValueError('Ellipse oracle only covers a single full ellipse')
    source = case['entities'][0]
    record, = document['records']
    width, height = document['paper_dimensions']
    for actual, expected, raw in zip(record['center'], source['center'], record['raw_coordinates']):
        near(actual, expected, f32_error(raw, width / 518) + 1e-9, 'ellipse center')
    near(record['radius'], source['radius'], f32_error(record['raw_coordinates'][2], width / 518) + 1e-9, 'ellipse radius')
    near(record['flatness_raw'] / 10000, source['flatness'], 1e-4, 'ellipse flatness')
    near(record['tilt_deg'], source.get('tilt_deg', 0), 1 / 65536 + 1e-9, 'ellipse tilt')
    entities = dxf_entities(dxf_path.read_bytes())
    scale = document['scales'][case.get('write_layer_group', 0)]
    center = [(record['center'][0] + width / 2) * scale, (record['center'][1] + height / 2) * scale]
    major = record['radius'] * scale
    minor = major * record['flatness_raw'] / 10000
    angle = math.radians(record['tilt_deg'])
    points = []
    for entity in entities:
        if entity['type'] != 'LINE':
            raise ValueError('Unexpected native ellipse DXF entity')
        segment = []
        for xcode, ycode in [(10, 20), (11, 21)]:
            x, y = float(tag(entity, xcode)), float(tag(entity, ycode))
            dx, dy = x - center[0], y - center[1]
            u = (dx * math.cos(angle) + dy * math.sin(angle)) / major
            v = (-dx * math.sin(angle) + dy * math.cos(angle)) / minor
            near(u * u + v * v, 1, 1e-7, 'DXF ellipse conic')
            segment.append((x, y, math.atan2(v, u)))
        points.append(segment)
    if len(points) < 4:
        raise ValueError('Incomplete ellipse tessellation')
    winding = 0
    for index, (start, end) in enumerate(points):
        following = points[(index + 1) % len(points)][0]
        for a, b in zip(end[:2], following[:2]):
            near(a, b, 1e-7, 'DXF ellipse continuity')
        winding += (end[2] - start[2] + math.pi) % (2 * math.pi) - math.pi
    near(abs(winding), 2 * math.pi, 1e-7, 'DXF ellipse winding')
    return dict(known_geometry_check=True, native_dxf_entities_check=True,
                native_dxf_full_validation=False, ellipse_line_segments=len(points))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', required=True, type=Path)
    parser.add_argument('--cases', type=Path, default=ROOT / 'jwc_samples/followup_cases.json')
    parser.add_argument('--jww-reader-path', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--role', choices=['all', 'candidate', 'holdout'], default='all')
    args = parser.parse_args()
    sys.path.insert(0, str(args.jww_reader_path))
    from ezjww import read_document
    cases = json.loads(args.cases.read_text())
    reports = []
    for case in cases:
        if args.role != 'all' and case['role'] != args.role:
            continue
        name = case['id']
        data = (args.directory / (name + '.jwc')).read_bytes()
        document = inspect(data)
        native = read_document(str(args.directory / (name + 'rt.jww')))
        jww = native_jww_check(document, native, case.get('expected_native_flag_changes'))
        header = header_check(data, native)
        if any(e.get('flatness', 1) != 1 for e in case['entities']):
            geometry = ellipse_check(case, document, args.directory / (name + 'rt.dxf'))
        else:
            geometry = check_case(case, args.directory)
            geometry.pop('geometry', None)
        reports.append(dict(id=name, role=case['role'], native_jww=jww, header=header, geometry=geometry))
    result = dict(profile_promoted=False, cases=reports,
                  inspector_sha256=hashlib.sha256((Path(__file__).parent / 'inspect_fixture.py').read_bytes()).hexdigest())
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    print(f'{len(reports)} native follow-up cases passed JWW and DXF checks.')


if __name__ == '__main__':
    main()
