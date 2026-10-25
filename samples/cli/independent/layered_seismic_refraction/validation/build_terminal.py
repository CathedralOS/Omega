"""Link private validation against the verified CLI's exact recorded Rust units.

The CLI binary is compared byte for byte with its Cargo/mbx unit output. The
library closure is selected using the unit's actual dependency fingerprints,
not timestamps or whichever duplicate library happens to appear first.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--omega', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    options = parser.parse_args()
    binary = options.omega.resolve()
    release = binary.parent
    binary_hash = digest(binary)
    candidates = []
    for metadata in release.rglob('bin-omega.json'):
        unit_output = metadata.parent.parent / 'out/omega'
        if unit_output.is_file() and digest(unit_output) == binary_hash:
            candidates.append(metadata)
    assert len(candidates) == 1, 'need one matching verified CLI build receipt'
    binary_metadata = json.loads(candidates[0].read_text())
    units = {}
    for metadata in release.rglob('lib-*.json'):
        fingerprint = metadata.with_suffix('')
        if fingerprint.is_file():
            identity = int.from_bytes(bytes.fromhex(fingerprint.read_text().strip()), 'little')
            name = metadata.stem.removeprefix('lib-')
            units.setdefault((name, identity), []).append(metadata)
    roots = {'compiler', 'terminal_runtime', 'package_manager', 'target', 'terminal_interpreter', 'terminal_codec'}
    selected = {}
    pending = [(row[1], row[3]) for row in binary_metadata['deps'] if row[1] in roots]
    while pending:
        name, identity = pending.pop()
        key = (name, identity)
        if key in selected:
            continue
        matches = units.get(key, [])
        assert len(matches) == 1, f'ambiguous or missing exact dependency: {key}'
        metadata = matches[0]
        libraries = [path for path in (metadata.parent.parent / 'out').glob(f'lib{name}-*')
                     if path.suffix in {'.rlib', '.so', '.dylib', '.dll'}]
        assert len(libraries) == 1, f'missing compiled library: {name}'
        selected[key] = libraries[0]
        dependency_metadata = json.loads(metadata.read_text())
        for row in dependency_metadata['deps']:
            dependency_key = (row[1], row[3])
            # Cargo build scripts are execution units, not linked Rust libraries.
            if row[1] != 'build_script_build':
                assert dependency_key in units, f'missing dependency receipt: {dependency_key}'
                pending.append(dependency_key)
    arguments = ['rustc', '--edition=2024', '--crate-name', 'w217_terminal_validation',
                 '-C', 'opt-level=0', '-C', 'debuginfo=0',
                 str(Path(__file__).with_name('terminal.rs')), '-o', str(options.output)]
    for directory in sorted({library.parent for library in selected.values()}):
        arguments.extend(['-L', f'dependency={directory}'])
    for name in sorted(roots):
        libraries = [library for (selected_name, _), library in selected.items() if selected_name == name]
        assert len(libraries) == 1, f'direct root mismatch: {name}'
        full_metadata = libraries[0].with_suffix('.rmeta')
        assert full_metadata.is_file(), f'missing full metadata: {name}'
        arguments.extend(['--extern', f'{name}={full_metadata}', '--extern', f'{name}={libraries[0]}'])
    options.output.parent.mkdir(parents=True, exist_ok=True)
    completed = subprocess.run(arguments)
    if completed.returncode:
        raise SystemExit(completed.returncode)
    receipt = {'compiler_binary_sha256': binary_hash, 'cli_unit_metadata': str(candidates[0]),
               'command': arguments, 'harness_sha256': digest(options.output),
               'libraries': [{'name': name, 'fingerprint': identity, 'path': str(library),
                              'sha256': digest(library)} for (name, identity), library in sorted(selected.items())]}
    options.output.with_suffix('.build.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(f'linked exact verified compiler closure: {len(selected)} units')


if __name__ == '__main__':
    main()
