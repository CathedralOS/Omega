"""Frame and compare diagnostic observations; the unchanged lower chain executes D.

This is neither a compiler gate nor trust evidence. Input files stay byte exact,
arrays match D's entry, and ordinary Console writes expose the parse-call window.
The independent Epsilon model supplies expected public results, never cost data.
"""
import datetime
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import time

from measure_observer import identity, observe

REPOSITORY = Path(__file__).resolve().parents[3]
FIELDS = ('source_bytes', 'status', 'cursor', 'diagnostic_start', 'diagnostic_end',
          'incomplete_start', 'incomplete_end', 'resource_limit', 'resource_requested', 'root_count')


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def frame(directory, source, stdin):
    receipt = (directory / 'canonical-receipt.gamma').read_bytes()
    assert identity(receipt) == {'bytes': 729060, 'sha256':
        'bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368'}
    closure = (directory / 'epsilon_compiler.delta').read_bytes()
    sealed = (b'EEREQ\x01\x00\x00' + struct.pack('<III', 1, len(source), len(stdin))
              + hashlib.sha256(closure).digest() + source + stdin)
    return struct.pack('<I', len(receipt)) + receipt + sealed



def source_revision():
    # WSL Git needs the translated metadata directory of Windows-created worktrees.
    command = ['git']
    marker = REPOSITORY / '.git'
    if marker.is_file():
        pointer = marker.read_text().strip().removeprefix('gitdir: ')
        if len(pointer) > 2 and pointer[1] == ':':
            directory = subprocess.check_output(['wslpath', '-u', pointer]).decode().strip()
            command += ['--git-dir', directory]
    return subprocess.check_output(command + ['rev-parse', 'HEAD'], cwd=REPOSITORY).decode().strip()


def prepare(directory, frozen=None):
    if (directory / 'inputs.json').exists():
        raise RuntimeError('Frozen inputs already exist; use a fresh result directory')
    prior = json.loads((frozen / 'inputs.json').read_text()) if frozen else None
    record = {'revision': prior['revision'] if prior else source_revision(), 'cases': {}}
    entry = Path(__file__).with_name('measure.epsilon').read_bytes()
    customer = (directory / 'omega_compiler.epsilon').read_bytes() + entry
    (directory / 'customer.epsilon').write_bytes(customer)
    record['customer'] = identity(customer)
    record['prepared_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    record['diagnostic_sources'] = {path.name: identity(path.read_bytes()) for path in
        (Path(__file__), Path(__file__).with_name('measure_observer.py'),
         Path(__file__).with_name('measure_heap.py'), Path(__file__).with_name('measure.epsilon'))}
    for name in record['diagnostic_sources']:
        (directory / name).write_bytes(Path(__file__).with_name(name).read_bytes())
    record['artifacts'] = {name: identity((directory / name).read_bytes()) for name in
                           ('evaluator.exe', 'omega_compiler.epsilon', 'epsilon_compiler.delta',
                            'evaluator_entry.delta', 'epsilon_model.py')}
    sys.path.insert(0, str(directory))
    import epsilon_model as model
    datasets = {'small': [REPOSITORY / 'source/psi/parse/input.omg'],
                'large': [REPOSITORY / 'source/psi/resolve/resolver.omg'],
                'subtree': sorted((REPOSITORY / 'source/psi/parse').glob('*.omg'))}
    for label, paths in datasets.items():
        content, members = b'', []
        if prior:
            content = (frozen / f'{label}.omg').read_bytes()
            members = prior['cases'][label]['members']
        else:
            for path in paths:
                unit = path.read_bytes()
                members.append({'path': path.relative_to(REPOSITORY).as_posix(),
                                'offset': len(content), **identity(unit)})
                content += unit
        (directory / f'{label}.omg').write_bytes(content)
        units = [('control/newline', b'\n'),
                 ('control/answer', b'machine answer() -> u8 { 42 }'),
                 ('control/nul', b'\0')]
        for member in members:
            unit = content[member['offset']:member['offset'] + member['bytes']]
            assert identity(unit) == {key: member[key] for key in ('bytes', 'sha256')}
            assert 0 < len(unit) <= 131072
            units.append((member['path'], unit))
        stdin, unit_records = b'\2', []
        cursor_points, output_points = {0: 'checking-finished', 1: 'mode-read'}, {}
        for index, (path, unit) in enumerate(units):
            header_end = len(stdin) + 4
            stdin += struct.pack('<I', len(unit)) + unit
            cursor_points[header_end] = f'{index}:header-read'
            cursor_points[len(stdin)] = f'{index}:source-read'
            for count in range(1024, len(unit), 1024):
                cursor_points[header_end + count] = f'{index}:source-prefix:{count}'
            output_points[3 + 42*index] = f'{index}:parse-start'
            output_points[4 + 42*index] = f'{index}:parse-end'
            output_points[44 + 42*index] = f'{index}:report-end'
            unit_records.append({'path': path, **identity(unit)})
        stdin += bytes(4)
        cursor_points[len(stdin)] = 'stdin-consumed'
        output_points[3 + 42*len(units)] = 'output-finished'
        (directory / f'{label}.stdin').write_bytes(stdin)
        started = time.monotonic()
        expected = model.observation(customer, stdin)
        assert expected[:7] == bytes(5) + b'L0' and expected[-1:] == b'E'
        assert len(expected) == 8 + 42*len(units)
        results = []
        for index in range(len(units)):
            assert expected[7+42*index:9+42*index] == bytes([240, 241])
            results.append(dict(zip(FIELDS, struct.unpack_from('<10i', expected, 9+42*index))))
        assert [row['status'] for row in results[:3]] == [1, 1, 2]
        (directory / f'{label}.expected').write_bytes(expected)
        record['cases'][label] = {**identity(content), 'members': members,
            'stdin': identity(stdin), 'units': unit_records,
            'cursor_points': cursor_points, 'output_points': output_points,
            'model_results': results, 'model_seconds_not_bootstrap_cost': time.monotonic()-started}
        print(f'{label}: independent model checked controls and {len(members)} unchanged C units', flush=True)
    inventory = []
    for relative in ('source/omega', 'source/psi', 'source/library/std'):
        for path in sorted((REPOSITORY / relative).rglob('*.omg')):
            if 'build' in path.relative_to(REPOSITORY / relative).parts:
                continue
            inventory.append({'path': path.relative_to(REPOSITORY).as_posix(), **identity(path.read_bytes())})
    record['declared_package_inventory'] = prior['declared_package_inventory'] if prior else inventory
    write_json(directory / 'inputs.json', record)


def selfcheck(directory):
    for name, source, pairs in (
        ('observer-scalar', b'(def main () Int 42)', 0),
        ('observer-pair', b'(def main () Int (let retained Int (pair 1 2) 42))', 1)):
        assert observe(directory, name, struct.pack('<I', len(source)) + source, 30) == b'*'
        assert json.loads((directory / f'{name}.json').read_text())['cumulative_pairs'] == pairs
    source = b'''boundary trait Console {
 machine exit_process(value: i32) -> never;
 machine read_byte() -> i32;
 machine write_byte(value: i32);
 machine write_line(text: &[u8]);
}
data Main { console: Console; }
machine Main::main(&mut self) {
 let first: i32 = self.console.read_byte();
 self.console.write_byte(76);
 self.console.write_byte(48);
 let unrelated: &[u8] = "L0";
 let second: i32 = self.console.read_byte();
 let third: i32 = self.console.read_byte();
 self.console.write_byte(first);
 self.console.write_byte(second);
 self.console.write_byte(third);
 self.console.write_byte(69);
}
'''
    output = observe(directory, 'phase-calibration', frame(directory, source, b'ABC'), 120, b'ABC',
                     {0: 'entry-ready', 1: 'read-one', 2: 'read-two', 3: 'read-three'},
                     {3: 'write-one', 4: 'write-two', 5: 'write-three', 6: 'finish'})
    assert output == bytes(5) + b'L0ABCE'
    phases = json.loads((directory / 'phase-calibration.phases.json').read_text())
    assert [row['name'] for row in phases['events']] == [
        'stdin-tree-ready', 'entry-ready', 'read-one', 'output-prefix', 'read-two',
        'read-three', 'write-one', 'write-two', 'write-three', 'finish']


def main():
    if sys.platform != 'linux' or os.uname().machine != 'x86_64':
        raise SystemExit('Use Linux x86-64; Windows: WSL; macOS: x86-64 Linux guest.')
    command, directory = sys.argv[1], Path(sys.argv[2]).resolve()
    if command == 'prepare':
        prepare(directory, Path(sys.argv[3]).resolve() if len(sys.argv) > 3 else None)
    elif command == 'receipt':
        source = (directory / 'epsilon_compiler.delta').read_bytes()
        source += (directory / 'evaluator_entry.delta').read_bytes()
        request = b'DCREQ\x01\x00\x00' + struct.pack('<II', 1, len(source))
        request += source + (directory / 'support.bin').read_bytes()
        compiler = (directory / 'delta_compiler.gamma').read_bytes()
        receipt = observe(directory, 'receipt', struct.pack('<I', len(compiler)) + compiler + request, 1800)
        assert identity(receipt) == {'bytes': 729060, 'sha256':
            'bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368'}
        (directory / 'canonical-receipt.gamma').write_bytes(receipt)
    elif command == 'selfcheck':
        selfcheck(directory)
    elif command == 'cases':
        inputs = json.loads((directory / 'inputs.json').read_text())
        for name, expected in inputs['artifacts'].items():
            assert identity((directory / name).read_bytes()) == expected, name
        customer = (directory / 'customer.epsilon').read_bytes()
        assert identity(customer) == inputs['customer']
        timeout = int(os.environ.get('OMEGA_LAYER0_SECONDS', '14400'))
        assert timeout > 0
        for label in sys.argv[3:] or inputs['cases']:
            case = inputs['cases'][label]
            stdin = (directory / f'{label}.stdin').read_bytes()
            assert identity(stdin) == case['stdin']
            output = observe(directory, label, frame(directory, customer, stdin), timeout, stdin,
                {int(key): value for key, value in case['cursor_points'].items()},
                {int(key): value for key, value in case['output_points'].items()})
            assert output == (directory / f'{label}.expected').read_bytes()
            print(f'{label}: native public output exactly agrees with independent model', flush=True)
    else:
        raise SystemExit('usage: measure.py prepare|receipt|selfcheck|cases DIRECTORY [case names]')


if __name__ == '__main__':
    main()
