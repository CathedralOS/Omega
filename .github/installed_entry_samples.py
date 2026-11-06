"""Retain the existing sample gate's commands and decoded result channels.

The ordinary sample gate remains responsible for package review, execution and
oracles. This observer calls its unchanged run operation and records what that
operation returns. A timeout retains the gate's absent result, not invented
output or execution. No compiler build is permitted through this observer.
"""
from pathlib import Path
import argparse
import hashlib
import json
import subprocess
import sys
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--cli', required=True, type=Path)
parser.add_argument('--source', required=True)
parser.add_argument('--manifest', required=True, type=Path)
parser.add_argument('--output', required=True, type=Path)
args = parser.parse_args()
root = Path.cwd().resolve()
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip() == args.source
assert not subprocess.check_output(['git', 'status', '--porcelain'], text=True).strip()
manifest = json.loads(args.manifest.read_text())
inputs = manifest['inputs']
verify_inputs = lambda: all(hashlib.sha256((root / path).read_bytes()).hexdigest() == digest
                            for path, digest in inputs.items())
assert verify_inputs()
args.output.mkdir(parents=True, exist_ok=False)
sys.path.insert(0, str(root / 'tools'))
import sample_native_gate as gate
from compiler_profile import require_optimized
profile = require_optimized(args.cli)
assert profile['profile']['opt_level'] == '1'
original = gate.run
stages = []
executables = {}


def recorded_run(command, timeout, stdin=subprocess.DEVNULL, cwd=None):
    assert timeout == 300
    executable = str(Path(command[0]).resolve())
    if executable not in executables:
        digest = hashlib.sha256(Path(executable).read_bytes()).hexdigest()
        assert digest == profile['sha256']
        executables[executable] = digest
    started = time.monotonic()
    event = dict(index=len(stages), command=[str(item) for item in command],
                 timeout=timeout, cwd=None if cwd is None else str(cwd))
    stages.append(event)
    try:
        result = original(command, timeout, stdin=stdin, cwd=cwd)
        event['timed_out'] = result is None
        if result is not None:
            event['exit_code'] = result.returncode
            for channel in ('stdout', 'stderr'):
                text = getattr(result, channel)
                assert isinstance(text, str)
                path = args.output / f"stage-{event['index']:03d}.{channel}"
                path.write_text(text, encoding='utf-8')
                event[channel] = dict(path=path.name,
                                     sha256=hashlib.sha256(path.read_bytes()).hexdigest())
        return result
    except BaseException as error:
        event['observer_error'] = repr(error)
        raise
    finally:
        event['seconds'] = round(time.monotonic() - started, 3)
        (args.output / 'stages.json').write_text(json.dumps(stages, indent=2) + '\n')


gate.run = recorded_run
sys.argv = ['sample_native_gate.py', '--cli', str(args.cli.resolve()), '--facts',
            '--target', gate.host_target(), '--jobs', '1', '--timeout', '300',
            '--filter', 'samples/cli/facts/installed_entry_storage',
            '--write-results', str(args.output / 'samples.json'), '--history-dir', '']
exit_code = 2
try:
    exit_code = gate.main()
finally:
    unchanged_profile = require_optimized(args.cli) == profile
    unchanged_inputs = verify_inputs()
    unchanged_copies = all(hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest
                           for path, digest in executables.items())
    record = dict(program_source=args.source, compiler_profile=profile,
                  gate_exit=exit_code, inputs_unchanged=unchanged_inputs,
                  compiler_unchanged=unchanged_profile, copied_executables=executables,
                  copied_executables_unchanged=unchanged_copies,
                  limits='Original sample gate: one worker, 300 seconds per CLI call. '
                         'Stage channels are the gate UTF-8 decoded strings; timeouts '
                         'have no returned channels. No corpus or final-head claim.')
    (args.output / 'observation.json').write_text(json.dumps(record, indent=2) + '\n')
    assert unchanged_profile and unchanged_inputs and unchanged_copies
raise SystemExit(exit_code)
