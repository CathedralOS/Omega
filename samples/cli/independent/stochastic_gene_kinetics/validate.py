"""Validate native/Terminal outputs against frozen independent and analytic oracles."""
import argparse
import hashlib
import json
import math
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
ORACLE = json.loads((ROOT / 'data/frozen_oracle.json').read_text())


def check_ensemble(output, kind):
    lines = output.decode().splitlines()
    assert len(lines) == 3, lines
    first = re.fullmatch(r'ensemble n=(\d+) seed_end=(\d+)', lines[0])
    second = re.fullmatch(r'sum_x=(\d+) sum_x2=(\d+) sum_y=(\d+) sum_y2=(\d+)', lines[1])
    third = re.fullmatch(r'hist_x 0=(\d+) 1=(\d+) 2=(\d+) 3plus=(\d+)', lines[2])
    assert first and second and third, lines
    observed = dict(zip(('n', 'seed_end'), map(int, first.groups())))
    observed.update(zip(('sum_x', 'sum_x2', 'sum_y', 'sum_y2'), map(int, second.groups())))
    observed['hist_x'] = list(map(int, third.groups()))
    expected = ORACLE[kind]
    assert observed == expected, (observed, expected)
    n = observed['n']
    assert sum(observed['hist_x']) == n
    analytic = ORACLE['analytic']
    mean_x = observed['sum_x'] / n
    second_x = observed['sum_x2'] / n
    # X(t) is Poisson. A six-standard-error acceptance band on its exact
    # first and second moments is generous for a 1024-replica diagnostic yet
    # catches material rate/state errors. The second raw moment has variance
    # m+6m²+4m³ for a Poisson variable.
    m = analytic['mean_x']
    assert abs(mean_x-m) <= 6*math.sqrt(m/n), (kind, 'mean_x', mean_x, m)
    second_variance = m + 6*m*m + 4*m*m*m
    assert abs(second_x-analytic['second_x']) <= 6*math.sqrt(second_variance/n), (kind, 'second_x', second_x)
    # Simultaneous four-bin Hoeffding intervals: union-bound failure <=
    # 8 exp(-2 n eps²) = 1e-4 for one ensemble.
    epsilon = math.sqrt(math.log(8/1e-4)/(2*n))
    for count, probability in zip(observed['hist_x'], analytic['hist_x']):
        assert abs(count/n-probability) <= epsilon, (kind, count, probability)
    if kind != 'gene':
        assert observed['sum_y'] == observed['sum_y2'] == 0
    else:
        # Independent analytic first moment for the two-stage gene model.
        mean_y = observed['sum_y'] / n
        empirical_second = observed['sum_y2'] / n
        empirical_variance = empirical_second-mean_y*mean_y
        assert empirical_variance > 0
        assert abs(mean_y-analytic['mean_protein']) <= 6*math.sqrt(empirical_variance/n), ('mean_protein', mean_y)
    return observed


def check_trajectory(output, kind):
    lines = output.decode().splitlines()
    expected_header = 'trajectory seed=100000 model=' + ('gene_expression' if kind == 'gene' else 'immigration_death')
    assert lines[0] == expected_header, lines
    expected = ORACLE['gene_events' if kind == 'gene' else 'immigration_events']
    assert len(lines) == len(expected)+2, (lines, expected)
    previous_time = 0.0
    x = y = 0
    for index, (line, (time, channel, wanted_x, wanted_y)) in enumerate(zip(lines[1:-1], expected), 1):
        match = re.fullmatch(r'event=(\d+) time=(\d+\.\d{6}) channel=(\d+) x=(\d+) y=(\d+)', line)
        assert match, line
        sequence, actual_time, actual_channel, actual_x, actual_y = match.groups()
        sequence, actual_time, actual_channel, actual_x, actual_y = int(sequence), float(actual_time), int(actual_channel), int(actual_x), int(actual_y)
        assert sequence == index and abs(actual_time-time) <= 0.00000051, (line, time)
        assert actual_time >= previous_time and actual_time <= 2
        assert (actual_channel, actual_x, actual_y) == (channel, wanted_x, wanted_y)
        dx, dy = ((1, 0), (0, 1), (-1, 0), (0, -1))[channel-1]
        assert (actual_x, actual_y) == (x+dx, y+dy)
        previous_time, x, y = actual_time, actual_x, actual_y
    assert lines[-1] == f'final time=2.000000 x={x}', lines[-1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--cli', required=True)
    args = parser.parse_args()
    output_dir = ROOT / 'build/validation'
    output_dir.mkdir(parents=True, exist_ok=True)
    records = []
    def run(label, command, data):
        result = subprocess.run([str(item) for item in command], input=data, capture_output=True, timeout=900)
        (output_dir / f'{label}.stdout').write_bytes(result.stdout)
        (output_dir / f'{label}.stderr').write_bytes(result.stderr)
        records.append({"label": label, "command": [str(item) for item in command],
                        "input_hex": data.hex(), "exit": result.returncode})
        (output_dir / "commands.json").write_text(json.dumps(records, indent=2)+"\n")
        return result
    source = ROOT / 'main.omg'
    compiler = Path(args.cli).resolve()
    checked = run('check', [compiler, '--check', '--target', 'linux_x86_64', source], b'')
    assert checked.returncode == 0, checked.stderr.decode(errors='replace')
    compiled = run('compile', [compiler, '--target', 'linux_x86_64', source], b'')
    assert compiled.returncode == 0, compiled.stderr.decode(errors='replace')
    published = re.findall(rb'^published native output to (.+)$', compiled.stdout, re.M)
    assert len(published) == 1, compiled.stdout
    native = Path(published[0].decode()).resolve()
    assert native.is_file()
    cases = [('immigration', b'0\n', 0), ('gene', b'1\n', 0),
             ('trace_immigration', b'2\n', 0), ('negative_rate', b'3\n', 2),
             ('negative_population', b'4\n', 2), ('overflow', b'5\n', 4),
             ('exhaustion', b'6\n', 3), ('alternate_seed', b'7\n', 0),
             ('trace_gene', b'8\n', 0), ('negative_decay', b'9\n', 2),
             ('empty', b'', 2), ('malformed', b'a\n', 2)]
    for name, data, status in cases:
        native_run = run('native_'+name, [native], data)
        if name in ('immigration', 'trace_immigration'):
            repeated = run('native_repeat_'+name, [native], data)
            assert repeated.returncode == native_run.returncode
            assert repeated.stdout == native_run.stdout
        both = run('both_'+name, [compiler, 'run', '--both', '--keep', source], data)
        retained = re.findall(rb'^build dir kept: (.+)$', both.stderr, re.M)
        assert len(retained) == 1, (name, both.stderr)
        terminal = Path(retained[0].decode())
        terminal_stdout = (terminal / 'terminal.stdout').read_bytes()
        terminal_stderr = (terminal / 'terminal.stderr').read_bytes()
        (output_dir / f'terminal_{name}.stdout').write_bytes(terminal_stdout)
        (output_dir / f'terminal_{name}.stderr').write_bytes(terminal_stderr)
        assert native_run.returncode == both.returncode == status, (name, native_run.returncode, both.returncode)
        assert native_run.stdout == both.stdout == terminal_stdout, (name, native_run.stdout, terminal_stdout)
        assert terminal_stderr == b"", (name, terminal_stderr)
        assert b'DECLINED' not in both.stderr and b'DIVERGENCE' not in both.stderr, (name, both.stderr)
        assert f'interp exit: {status}'.encode() in both.stderr, (name, both.stderr)
        if name in ('immigration', 'gene', 'alternate_seed'):
            kind = 'gene' if name == 'gene' else 'immigration' if name == 'immigration' else 'alternate'
            check_ensemble(native_run.stdout, kind)
            check_ensemble(terminal_stdout, kind)
        elif name.startswith('trace_'):
            kind = 'gene' if name == 'trace_gene' else 'immigration'
            check_trajectory(native_run.stdout, kind)
            check_trajectory(terminal_stdout, kind)
        else:
            expected = {'negative_rate': 'invalid model: nonnegative rates and populations, bounded seed required\n',
                        'negative_population': 'invalid model: nonnegative rates and populations, bounded seed required\n',
                        'negative_decay': 'invalid model: nonnegative rates and populations, bounded seed required\n',
                        'overflow': 'population overflow\n', 'exhaustion': 'event budget exhausted\n',
                        'empty': 'invalid input: one digit 0..9 and LF required\n',
                        'malformed': 'invalid input: one digit 0..9 and LF required\n'}[name]
            assert native_run.stdout.decode() == terminal_stdout.decode() == expected, (name, native_run.stdout, terminal_stdout)
    print(f'validated {len(cases)} cases: native and Terminal output, independent trajectories, analytical moments and distribution')


if __name__ == '__main__':
    main()
