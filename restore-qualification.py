#!/usr/bin/env python3
"""Verify ordered qualification parts and optionally restore the original ZIP."""
import argparse
import hashlib
import json
import re
from pathlib import Path


def require(condition, reason):
    if not condition:
        raise RuntimeError(reason)


def file_digest(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def scan(parts, output=None):
    whole = hashlib.sha256()
    total = 0
    for path, row in parts:
        part = hashlib.sha256()
        size = 0
        with path.open('rb') as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b''):
                part.update(block)
                whole.update(block)
                size += len(block)
                total += len(block)
                if output is not None:
                    output.write(block)
        require(size == row['bytes'] and part.hexdigest() == row['sha256'], 'Part changed or is corrupt: ' + row['name'])
    return total, whole.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--verify-only', action='store_true', help='Hash the full stream without creating another ZIP copy')
    parser.add_argument('--manifest', type=Path, default=Path(__file__).resolve().with_name('qualification.parts.json'))
    options = parser.parse_args()
    manifest = json.loads(options.manifest.read_text())
    require(manifest['schema'] == 'omega-qualification-parts-v1', 'Unknown parts manifest schema')
    require(manifest['archive'] == 'qualification.zip', 'Unexpected output archive name')
    require(isinstance(manifest['bytes'], int) and manifest['bytes'] > 0, 'Invalid archive size')
    require(re.fullmatch('[0-9a-f]{64}', manifest['sha256']), 'Invalid archive digest')
    rows = manifest['parts']
    require(rows, 'No archive parts')
    parts = []
    for index, row in enumerate(rows, 1):
        require(row['name'] == f'qualification.zip.part{index:03d}', 'Parts are missing, reordered or have unsafe names')
        require(isinstance(row['bytes'], int) and row['bytes'] > 0 and re.fullmatch('[0-9a-f]{64}', row['sha256']), 'Invalid part size/digest')
        parts.append((options.manifest.resolve().parent / row['name'], row))
    expected = (manifest['bytes'], manifest['sha256'])
    # Every part and the entire ordered stream are authenticated before writes.
    require(scan(parts) == expected, 'Whole archive size/digest mismatch')
    destination = options.manifest.resolve().parent / manifest['archive']
    if destination.exists():
        require(destination.is_file() and destination.stat().st_size == manifest['bytes']
                and file_digest(destination) == manifest['sha256'], 'Refusing to overwrite a different existing qualification.zip')
        print('Verified parts and identical existing archive:', manifest['sha256'])
        return
    if options.verify_only:
        print('Verified ordered archive stream:', manifest['sha256'], manifest['bytes'], 'bytes; no ZIP copy written')
        return
    created = False
    try:
        with destination.open('xb') as output:
            created = True
            require(scan(parts, output) == expected, 'Archive changed during restoration')
    except BaseException:
        if created:
            destination.unlink()
        raise
    print('Restored:', destination, manifest['sha256'], manifest['bytes'], 'bytes')


if __name__ == '__main__':
    main()
