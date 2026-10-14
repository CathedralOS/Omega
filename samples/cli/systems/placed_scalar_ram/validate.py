"""Compare actual native and captured Terminal streams to pre-execution oracles."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--evidence', type=Path, required=True)
    args = parser.parse_args()
    cli = args.cli.resolve()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    for name, expected in json.loads((ROOT / 'oracle-freeze.json').read_text())['files'].items():
        assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == expected, name
    projects = [('scalar_composition', ROOT), ('original_backing', ROOT.parents[3] / 'tests/omega/run/layouts/placed_ram_original_backing'), ('scalar_types', ROOT.parents[3] / 'tests/omega/run/layouts/placed_ram_scalar_types')]
    freeze = json.loads((ROOT / 'oracle-freeze.json').read_text())
    for name, project in projects[1:]:
        for filename, digest in freeze['referenced_fixtures'][name].items():
            assert hashlib.sha256((project / filename).read_bytes()).hexdigest() == digest, (name, filename)
    records = []
    for name, project in projects:
        output = evidence / name
        output.mkdir(exist_ok=True)
        stdin = output / (name + '.stdin')
        stdin.write_bytes(b'')
        command = [str(cli), 'run', '--both', '--keep', str(project / 'main.omg'), '--input', str(stdin), '--outputs', str(output)]
        start = time.monotonic()
        result = subprocess.run(command, input=b'', capture_output=True, timeout=180)
        (output / 'command.stdout').write_bytes(result.stdout)
        (output / 'command.stderr').write_bytes(result.stderr)
        record = dict(name=name, command=command, exit=result.returncode, seconds=time.monotonic()-start)
        records.append(record)
        (evidence / 'commands.json').write_text(json.dumps(records, indent=2) + '\n')
        assert result.returncode == 0, result.stderr.decode(errors='replace')
        with (output / 'results.tsv').open(newline='') as table:
            rows = list(csv.DictReader(table, delimiter='\t'))
        assert len(rows) == 1, rows
        row = rows[0]
        expected_exit = (project / 'expected_native_exit.txt').read_text().strip()
        assert row['case'] == name and row['native'] == row['interpreter_exit'] == expected_exit, row
        assert row['interpreter'] == 'agrees', row
        for stream in ['stdout', 'stderr']:
            expected = (project / ('expected_' + stream + '.txt')).read_bytes()
            native = (output / (name + '.' + stream)).read_bytes()
            terminal = (output / (name + '.terminal.' + stream)).read_bytes()
            assert native == terminal == expected, (name, stream, native, terminal, expected)
        record.update(native='match', terminal='match', lines=len((project / 'expected_stdout.txt').read_bytes().splitlines()))
    summary = dict(producer_sha256=hashlib.sha256(cli.read_bytes()).hexdigest(), checks=records)
    (evidence / 'qualification.json').write_text(json.dumps(summary, indent=2) + '\n')
    print('Native and captured Terminal: exact composition, original-backing and signed/bool/f32/f64 streams match.')


if __name__ == '__main__':
    main()
