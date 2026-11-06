"""Qualify the unchanged installed-entry row on one missing hosted target.

Preparation builds one optimized producer; every later phase consumes its
retained manifest. The canonical corpus and sample gates own all judgments.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import signal
import time
import subprocess
import sys

R = Path.cwd()
sys.path[:0] = [str(R / 'tools'), str(R / 'tools/ci')]
import corpus_gate
from compiler_pilot import run_suite, select_fixtures, select_samples
from compiler_replay_plan import input_objects

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('phase', choices=('preflight', 'prepare', 'corpus', 'budgets', 'samples'))
args = parser.parse_args()
recipe = json.loads((R / '.github/installed_entry_missing_hosts_recipe.json').read_text())
manifest = json.loads((R / '.github/installed_entry_missing_hosts_inputs.json').read_text())
host = os.environ['INSTALLED_ENTRY_HOST']
assert host in recipe['hosts'] and corpus_gate.host_target() == host
revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
assert revision == os.environ['GITHUB_SHA']
assert subprocess.check_output(['git', 'rev-parse', 'HEAD^'], text=True).strip() == recipe['semantic_source']
assert not subprocess.check_output(['git', 'status', '--porcelain'], text=True).strip()
assert input_objects(R, revision) == manifest['compiled_input_roots']
assert all(hashlib.sha256((R / path).read_bytes()).hexdigest() == digest
           for path, digest in manifest['inputs'].items())
assert os.environ['OMEGA_CORPUS_FIXTURE_SECS'] == '120'
assert os.environ['OMEGA_CORPUS_JOBS'] == '1'
assert not os.environ.get('OMEGA_PHASE_PROFILE')
positive, negative = select_fixtures(','.join(recipe['source_checks']))
assert len(positive) == 16 and len(negative) == 8
assert positive == tuple(recipe['both'])
assert len(recipe['samples']) == 19
assert set(select_samples(manifest['sample_root'])) == set(recipe['samples'])
name = 'installed-entry-' + host
output = R / 'build' / name

if args.phase == 'preflight':
    output.mkdir(parents=True, exist_ok=False)
    # The hosted runner supplies the native target. No QEMU/PRoot command or
    # cross-target override is configured in this recipe. Retain actual facts.
    binding = dict(observer=revision, compiled_and_program_source=revision,
                   fixed_published_source=recipe['fixed_published_source'],
                   host_target=host, uname=platform.uname()._asdict(),
                   processor=platform.processor(),
                   runner_os=os.environ.get('RUNNER_OS'),
                   runner_arch=os.environ.get('RUNNER_ARCH'),
                   runner_name=os.environ.get('RUNNER_NAME'),
                   recipe=recipe, manifest=manifest,
                   execution_route='direct hosted native and Terminal; no emulation command configured')
    (output / 'input-binding.json').write_text(json.dumps(binding, indent=2) + '\n')
    if os.name == 'nt':
        (Path(R.anchor) / 'tmp').mkdir(exist_ok=True)
elif args.phase == 'prepare':
    import build_slot
    with build_slot.hold(R):
        result = run_suite(name, positive, negative, checking_only=(), phase='prepare')
        if result == 0:
            # Preserve a useful successful compiler/cache even if a later unit
            # command fails. Consumers still require the whole step to pass.
            with Path(os.environ['GITHUB_OUTPUT']).open('a') as channel:
                channel.write('compiler_prepared=true\n')
            tests = []
            for index, test in enumerate(recipe['rust_tests']):
                command = (['mbx', 'nextest', 'run', '--status-level', 'pass'] if shutil.which('mbx') else ['cargo', 'test'])
                command += ['--locked', '-p', test['package'], '--lib', test['filter']]
                stdout = output / ('rust-test-' + str(index) + '.stdout')
                stderr = output / ('rust-test-' + str(index) + '.stderr')
                started = time.monotonic()
                timed_out = False
                code = None
                process = None
                error = None
                try:
                    with stdout.open('wb') as out, stderr.open('wb') as err:
                        process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                                   start_new_session=True,
                                                   env=dict(os.environ, CARGO_TERM_COLOR='never'))
                        code = process.wait(timeout=1200)
                except subprocess.TimeoutExpired:
                    timed_out = True
                except Exception as failure:
                    error = repr(failure)
                finally:
                    if process is not None and process.poll() is None:
                        os.killpg(process.pid, signal.SIGTERM)
                        try:
                            process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            os.killpg(process.pid, signal.SIGKILL)
                            process.wait(timeout=5)
                text = ''.join(path.read_text(errors='replace') for path in (stdout, stderr) if path.is_file())
                text = re.sub(r'\x1b\[[0-9;]*m', '', text)
                cargo_names = re.findall(r'(?m)^test ([A-Za-z0-9_:]+) \.\.\. ok\s*$', text)
                nextest_names = re.findall(r'(?m)^\s*PASS\s+\[[^\]\n]*\]\s+\S+\s+([A-Za-z0-9_:]+)\s*$', text)
                actual_names = cargo_names + nextest_names
                exact_names = sorted(actual_names) == sorted(test['names'])
                count = test['count']
                exact_count = re.search(r'test result: ok[.] '+str(count)+r' passed; 0 failed|'+str(count)+r' tests run: '+str(count)+r' passed', text) is not None
                passed = code == 0 and error is None and not timed_out and exact_count and exact_names
                channels = {path.name:dict(bytes=path.stat().st_size, sha256=hashlib.sha256(path.read_bytes()).hexdigest())
                            for path in (stdout, stderr) if path.is_file()}
                tests.append(dict(test=test, command=command, exit=code, timed_out=timed_out, error=error,
                                  elapsed_seconds=time.monotonic()-started, passed=passed,
                                  actual_names=actual_names, exact_names=exact_names, exact_count=exact_count,
                                  channels=channels))
                (output / 'structural-tests.json').write_text(json.dumps(dict(source=revision, tests=tests,
                    passed=len(tests)==len(recipe['rust_tests']) and all(row['passed'] for row in tests)), indent=2) + '\n')
                if not passed:
                    result = 1
                    break
    raise SystemExit(result)
elif args.phase == 'corpus':
    raise SystemExit(run_suite(name, positive, negative, checking_only=(), phase='consume'))
elif args.phase == 'budgets':
    checking = json.loads((output / 'checking.json').read_text())
    both = json.loads((output / 'both.json').read_text())
    assert {row['fixture'] for row in checking['observations']} == set(recipe['source_checks'])
    assert {row['fixture'] for row in both['observations']} == set(recipe['both'])
    assert len(checking['observations']) == 24 and len(both['observations']) == 16
    passed = all(type(row.get('millis')) is int and 0 <= row['millis'] <= 120000
                 for report in (checking, both) for row in report['observations'])
    (output / 'budgets.json').write_text(json.dumps(dict(
        limit_ms=120000, passed=passed, checking=checking['observations'],
        both=both['observations']), indent=2) + '\n')
    assert passed, 'Original Source/BOTH limit remains unmet; preserve every report'
else:
    producer = json.loads((output / 'producer.json').read_text())['producer']
    command = [sys.executable, '.github/installed_entry_samples.py',
               '--cli', producer['cli'], '--source', revision,
               '--manifest', '.github/installed_entry_missing_hosts_inputs.json',
               '--output', str(output / 'samples')]
    result = subprocess.run(command, stdin=subprocess.DEVNULL)
    rows = json.loads((output / 'samples/samples.json').read_text())
    assert len(rows) == 19 and {row['sample'] for row in rows} == set(recipe['samples'])
    assert all(row['state'] == ('rejected' if row['sample'].endswith('/invalid') else 'pass')
               for row in rows)
    raise SystemExit(result.returncode)
