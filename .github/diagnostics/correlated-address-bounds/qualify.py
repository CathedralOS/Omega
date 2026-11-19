from pathlib import Path
import hashlib
import json
import os
import resource
import shutil
import subprocess
import time

root = Path('/workspace/Omega')
os.chdir(root)
out = root / 'build/correlated-address-bounds/hosted1'
out.mkdir(parents=True, exist_ok=False)
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def save(name, value):
    (out / name).write_text(json.dumps(value, indent=2) + '\n')
def git(*arguments):
    return subprocess.check_output(['git', *arguments], text=True).strip()
def run(name, arguments, directory=root, expected=0, timeout=1500):
    started = time.monotonic()
    with (out / (name + '.stdout')).open('wb') as stdout, (out / (name + '.stderr')).open('wb') as stderr:
        process = subprocess.run(arguments, cwd=directory, stdout=stdout, stderr=stderr, timeout=timeout)
    result = {'arguments': arguments, 'directory': str(directory), 'returncode': process.returncode,
              'elapsed_seconds': time.monotonic() - started,
              'maximum_child_rss_kib_so_far': resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss}
    save(name + '.json', result)
    print(name, result, flush=True)
    if process.returncode != expected:
        print((out / (name + '.stderr')).read_text()[-20000:], flush=True)
        raise SystemExit(1)
    return result

candidate = os.environ['CANDIDATE_SHA']
allowed = ['.github/workflows/post-merge-check.yml', '.github/diagnostics/correlated-address-bounds/qualify.py']
assert all(path in allowed for path in git('diff', '--name-only', candidate, 'HEAD').splitlines())
fixtures = ['tests/fixtures/pass/arithmetic/correlated_addr_bounds', 'tests/fixtures/fail/arithmetic/correlated_addr_bounds']
inputs = {str(path): digest(root / path) for folder in fixtures for path in sorted(Path(folder).glob('*.omg'))}
save('inputs.json', {'candidate_revision': candidate, 'validation_revision': git('rev-parse', 'HEAD'),
                    'fixture_sha256': inputs, 'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
                    'environment': {name: os.environ.get(name) for name in ['CARGO_INCREMENTAL', 'CARGO_BUILD_JOBS', 'CARGO_PROFILE_DEV_OPT_LEVEL', 'CARGO_PROFILE_TEST_OPT_LEVEL', 'MBX_LEARNED_INCREMENTAL', 'MBX_GC_AUTO', 'RUST_MIN_STACK']}})
run('metadata', ['mbx', 'check', '--locked', '-p', 'source-compiler', '-p', 'proof-admission', '--tests'])
run('kernel-controls', ['mbx', 'nextest', 'run', '--locked', '-p', 'proof-admission', '--lib', '-E',
    'test(exact_address_arithmetic_preserves_its_typed_mathematical_value) | test(total_widening_preserves_the_mathematical_integer)', '--no-capture'])
run('source-controls', ['mbx', 'nextest', 'run', '--locked', '-p', 'source-compiler', '--lib', '-E',
    'test(mathematical_address_images_keep_the_declared_carrier) | test(mathematical_sum_bounds_follow_a_smaller_count_without_changing_carriers) | test(a_correlated_endpoint_identity_keeps_its_independent_address_fit) | test(remaining_capacity_bounds_the_exact_sum_and_its_use)', '--no-capture'])
run('clippy', ['mbx', 'clippy', '--locked', '-p', 'source-compiler', '-p', 'proof-admission', '--all-targets', '--', '-D', 'warnings'])
run('format', ['mbx', 'xtask', 'fmt', '--check'])
run('boundary', ['mbx', 'xtask', 'check-boundary'])
run('cli-build', ['mbx', 'build', '--locked', '-p', 'omega', '--bin', 'omega', '--message-format=json'])
artifacts = [json.loads(line) for line in (out / 'cli-build.stdout').read_text().splitlines() if line.startswith('{')]
artifacts = [row for row in artifacts if row.get('reason') == 'compiler-artifact' and row['target']['name'] == 'omega' and row.get('executable')]
assert len(artifacts) == 1
artifact = artifacts[0]
assert artifact['profile']['opt_level'] in ['1', '2', '3']
cli = Path(artifact['executable'])
preserved = out / 'preserved-producer'
preserved.mkdir()
shutil.copy2(cli, preserved / 'omega')
cli_digest = digest(cli)
save('producer.json', {'candidate_revision': candidate, 'validation_revision': git('rev-parse', 'HEAD'),
                      'executable': str(cli), 'sha256': cli_digest, 'cargo_artifact': artifact})
arguments = [str(cli), 'run', '--both', '--keep', '--verbose', '--build-input', 'build.omg', '--build-input', 'main.omg', 'main.omg']
run('positive', arguments, root / fixtures[0], timeout=180)
positive = (out / 'positive.stderr').read_text()
assert 'native exit 0' in positive and 'interp exit: 0' in positive, positive
run('missing-fit', arguments, root / fixtures[1], expected=200, timeout=180)
negative = (out / 'missing-fit.stderr').read_text()
assert 'local `endpoint`' in negative and ('may overflow' in negative or 'operation safety' in negative), negative
assert 'native exit' not in negative and 'interp exit:' not in negative
assert inputs == {str(path): digest(root / path) for folder in fixtures for path in sorted(Path(folder).glob('*.omg'))}
assert digest(cli) == cli_digest
save('complete.json', {'candidate_revision': candidate, 'positive': 'native 0 / Psi 0',
                       'negative': 'address formation refused at local endpoint', 'producer_sha256': cli_digest})
print('COMPLETE: native 0 / Psi 0; missing-fit control refused at endpoint formation', flush=True)
