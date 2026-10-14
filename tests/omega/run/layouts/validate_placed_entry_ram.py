#!/usr/bin/env python3
"""Qualify the public placed-entry RAM client using actual values and frames."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import time

SAMPLE = Path('samples/cli/systems/placed_entry_ram')
CASES = [('original', '1=2900000003000000', b'41\n', bytes.fromhex('2900000009000000')),
         ('status_87', '1=57000000ff000000', b'87\n', bytes.fromhex('5700000009000000')),
         ('status_27', '1=1b00000006000000', b'27\n', bytes.fromhex('1b00000009000000'))]
CONTROLS = [
    ('missing', [], 'PlacedViewInputsRequireCustodyLowering'),
    ('unknown_position', ['--entry-ram', '2=2900000003000000'], 'answers no declared entry position'),
    ('context_position', ['--entry-ram', '0=2900000003000000'], 'answers no declared entry position'),
    ('duplicate', ['--entry-ram', '1=2900000003000000', '--entry-ram', '1=2900000003000000'], 'PlacedViewEstablishmentDuplicate'),
    ('short_extent', ['--entry-ram', '1=29000000030000'], 'complete nonempty bounded schema extent'),
    ('long_extent', ['--entry-ram', '1=290000000300000000'], 'complete nonempty bounded schema extent'),
    ('noncanonical_padding', ['--entry-ram', '1=2900000003010000'], 'padding must be initialized to canonical zero'),
    ('incomplete_byte', ['--entry-ram', '1=2'], 'nonempty complete bytes'),
    ('noncanonical_hex', ['--entry-ram', '1=2A00000003000000'], 'hex must be canonical lowercase'),
    ('noncanonical_position', ['--entry-ram', '01=2900000003000000'], 'position must be canonical unsigned decimal'),
    ('arbitrary_pointer', ['--entry-ram', '1=0x7fff00000000'], 'hex must be canonical lowercase'),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', required=True, type=Path)
    parser.add_argument('--evidence', required=True, type=Path)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--positive-only', action='store_true')
    modes.add_argument('--negative-only', action='store_true')
    args = parser.parse_args()
    cli = args.cli.resolve()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    assert (SAMPLE / 'expected_stdout.txt').read_bytes() == CASES[0][2]
    assert bytes.fromhex((SAMPLE / 'expected_original_backing.hex').read_text()) == CASES[0][3]
    assert (SAMPLE / 'entry_ram.hex').read_text().strip() == CASES[0][1].split('=')[1]
    results = []

    def run(name, arguments):
        command = [str(cli), *arguments]
        started = time.monotonic()
        output = subprocess.run(command, stdin=subprocess.DEVNULL, capture_output=True, timeout=120)
        (evidence / f'{name}.stdout').write_bytes(output.stdout)
        (evidence / f'{name}.stderr').write_bytes(output.stderr)
        results.append({'id': name, 'command': command, 'exit': output.returncode,
                        'elapsed_seconds': time.monotonic() - started})
        return output

    for name, initializer, stdout, backing in ([] if args.negative_only else CASES):
        output = run(name, ['run', '--both', '--keep', '--entry-ram', initializer, str(SAMPLE / 'main.omg')])
        assert output.returncode == 0, output.stderr.decode()
        assert output.stdout == stdout, (name, output.stdout)
        assert b'interp exit: 0' in output.stderr and b'DECLINED' not in output.stderr, output.stderr.decode()
        kept = re.search(rb'build dir kept: ([^\n]+)', output.stderr)
        assert kept, output.stderr.decode()
        source = Path(kept[1].decode())
        saved = evidence / name
        shutil.copytree(source, saved, dirs_exist_ok=True)
        assert (saved / 'terminal.stdout').read_bytes() == stdout
        assert (saved / 'terminal.stderr').read_bytes() == b''
        assert (saved / 'native.entry-ram.1.bin').read_bytes() == backing
        assert (saved / 'terminal.entry-ram.1.bin').read_bytes() == backing
        effects = (saved / 'terminal.effects').read_text().splitlines()
        assert len(effects) == 5 and all(line.startswith('BoundaryCall {') for line in effects), effects
        reads = [line for line in effects if 'boundary: BoundaryMachineId(4),' in line]
        writes = [line for line in effects if 'boundary: BoundaryMachineId(5),' in line]
        assert len(reads) == len(writes) == 1, effects
        assert 'arguments: []' in reads[0], reads
        assert 'Unsigned(9)' in writes[0], writes
        admitted = run(name + '-canonical', ['inspect-terminal', '--artifact', str(saved / 'program.oterm')])
        assert admitted.returncode == 0, admitted.stderr.decode()
        text = admitted.stdout.decode()
        fingerprint = re.search(r'terminal program_fingerprint=([0-9a-f]{64})', text)
        rows = [line for line in text.splitlines() if line.startswith('placed_input ')]
        assert fingerprint and len(rows) == 1, text
        seal = re.search(r'placement_commitment: \[([0-9, ]+)\]', rows[0])
        assert seal, rows
        header = (b'OMEGARAM' + bytes.fromhex(fingerprint[1])
                  + bytes(int(value) for value in seal[1].split(','))
                  + (1).to_bytes(4, 'little') + (1).to_bytes(4, 'little') + (8).to_bytes(4, 'little'))
        assert len(header) == 84
        assert (saved / 'native.entry-ram.frames').read_bytes() == header + backing
        assert (saved / 'native.object.txt').stat().st_size and (saved / 'omega-program').stat().st_size
        print(f'{name}: actual native and Terminal {stdout.decode().strip()}, original backing {backing.hex()}', flush=True)

    if not args.positive_only:
        for name, arguments, diagnostic in CONTROLS:
            output = run(name, ['run', '--both', *arguments, str(SAMPLE / 'main.omg')])
            assert output.returncode != 0 and diagnostic.encode() in output.stderr, (name, output.stderr.decode())
            assert output.stdout == b'', (name, output.stdout)
        output = run('shared_formal_write', ['--check', 'tests/omega/fail/layouts/placed_shared_formal_cannot_write/main.omg'])
        assert output.returncode != 0 and b'mutable' in output.stderr and b'view' in output.stderr, output.stderr.decode()
    files = {str(p.relative_to(evidence)): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in evidence.rglob('*') if p.is_file() and p != evidence / 'qualification.json'}
    (evidence / 'qualification.json').write_text(json.dumps({
        'producer': str(cli), 'producer_sha256': hashlib.sha256(cli.read_bytes()).hexdigest(),
        'source_sha256': hashlib.sha256((SAMPLE / 'main.omg').read_bytes()).hexdigest(),
        'checks': results, 'files': files, 'actual_values_and_original_storage': True,
    }, indent=2) + '\n')


if __name__ == '__main__':
    main()
