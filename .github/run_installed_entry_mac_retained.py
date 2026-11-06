"""Observe the existing installed-entry row with one authenticated Mac producer.

This consumer has no Rust build route. The normal corpus and sample gates own
all source, engine and original-backing judgments. Failed consumers stay failed;
the independent sample gate still runs after a corpus refusal or deadline.
"""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
import urllib.error
import urllib.request
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--recipe', type=Path, required=True)
args = parser.parse_args()
staged = args.recipe.resolve().parent
settings = json.loads(args.recipe.read_text())
root = Path.cwd().resolve()
out = root / 'build' / settings['label']
out.mkdir(parents=True, exist_ok=False)
sha = lambda raw: hashlib.sha256(raw).hexdigest()
git = lambda *argv: subprocess.check_output(['git', *argv], text=True).strip()
api = lambda path: json.loads(subprocess.check_output(['gh', 'api', path], text=True))
read = lambda p: json.loads(p.read_text())
assert settings['producer_metadata_complete'] is True, 'Original producer metadata is not bound'
source = settings['source']
driver = os.environ['GITHUB_SHA']
assert git('rev-parse', 'HEAD') == source and git('rev-parse', 'HEAD^{tree}') == settings['tree']
assert not git('status', '--porcelain')
assert os.environ['OMEGA_CORPUS_JOBS'] == '1'
assert os.environ['OMEGA_CORPUS_FIXTURE_SECS'] == '120'
assert not os.environ.get('OMEGA_PHASE_PROFILE')
stage_record = read(staged / 'staged-observer.json')
assert stage_record['observer'] == driver
for name, digest in stage_record['files'].items():
    assert sha((staged / name).read_bytes()) == digest
for name, digest in settings['observer_files'].items():
    assert stage_record['files'][name] == digest
manifest_path = staged / 'installed_entry_mac_inputs.json'
manifest = read(manifest_path)
assert len(manifest['inputs']) == 358
assert manifest['source_checks'] == settings['source_checks']
assert manifest['both'] == settings['both'] and manifest['samples'] == settings['samples']
sys.path[:0] = [str(root / 'tools'), str(root / 'tools/ci')]
import compiler_pilot

assert compiler_pilot.corpus_gate.host_target() == settings['target'] == 'macos_arm64'
compiler_pilot.compiler_replay_plan.check_inputs(root, source, settings['compiler_inputs'])
positive, negative = compiler_pilot.select_fixtures(','.join(settings['source_checks']))
assert len(positive) == 13 and len(negative) == 5 and list(positive) == settings['both']
assert set(compiler_pilot.select_samples(manifest['sample_root'])) == set(settings['samples'])
assert len(settings['samples']) == 19


def verify_inputs():
    assert git('rev-parse', 'HEAD') == source and not git('status', '--porcelain')
    for path, digest in manifest['inputs'].items():
        assert sha((root / path).read_bytes()) == digest


verify_inputs()
origin = api(f"repos/CathedralOS/Omega/actions/runs/{settings['origin_run']}")
assert origin['head_sha'] == settings['origin_driver']
assert origin['status'] in ('in_progress', 'completed')
artifact = api(f"repos/CathedralOS/Omega/actions/artifacts/{settings['artifact_id']}")
for key in ('name', 'size_in_bytes', 'digest'):
    assert artifact[key] == settings['artifact'][key]
assert not artifact['expired'] and artifact['workflow_run']['id'] == settings['origin_run']
assert artifact['workflow_run']['head_sha'] == settings['origin_driver']
retained = out / 'retained'
retained.mkdir()
archive_path = retained / 'original.zip'


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *unused, **kwargs):
        return None


token = subprocess.check_output(['gh', 'auth', 'token'], text=True).strip()
request = urllib.request.Request(artifact['archive_download_url'], headers={
    'Authorization': 'Bearer ' + token, 'Accept': 'application/vnd.github+json'})
try:
    urllib.request.build_opener(NoRedirect()).open(request, timeout=30)
    raise ValueError('Expected signed artifact redirect')
except urllib.error.HTTPError as error:
    assert error.code == 302
    url = error.headers['Location']
digest = hashlib.sha256()
size = 0
with urllib.request.urlopen(url, timeout=90) as response, archive_path.open('xb') as target:
    while chunk := response.read(4 * 1024 * 1024):
        size += len(chunk)
        assert size <= artifact['size_in_bytes']
        digest.update(chunk)
        target.write(chunk)
assert size == artifact['size_in_bytes'] and artifact['digest'] == 'sha256:' + digest.hexdigest()
members = []
with zipfile.ZipFile(archive_path) as archive:
    infos = archive.infolist()
    assert len({i.filename for i in infos}) == len(infos)
    files = [i for i in infos if not i.is_dir()]
    assert sum(i.file_size for i in files) == settings['archive_expanded_bytes']
    for info in files:
        path = PurePosixPath(info.filename)
        assert not path.is_absolute() and '..' not in path.parts
        assert '\\' not in info.filename and path.as_posix() == info.filename
        assert (info.external_attr >> 16) & 0o170000 != 0o120000
        assert info.file_size <= settings['archive_largest_member_bytes']
        data = archive.read(info)
        target = retained / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        members.append(dict(name=info.filename, bytes=len(data), sha256=sha(data)))
original = read(retained / settings['producer_record'])
assert original == settings['original_producer']
prep = read(retained / 'preparation.json')
assert prep['passed'] is True and prep['producer'] == original
assert prep['candidate'] == original['revision'] == source
assert prep['tree'] == original['tree'] == settings['tree']
assert prep['target'] == original['target'] == settings['target']
origin_driver = read(retained / 'driver.json')
assert origin_driver == settings['original_driver_record']
assert origin_driver['source'] == source and origin_driver['driver'] == settings['origin_driver']
assert origin_driver['target'] == settings['target']
tests = read(retained / 'term-comparison-tests.json')
assert tests['source'] == source and tests['driver'] == settings['origin_driver']
assert tests['passed'] is True and tests['expected_existing_tests'] == 27
assert tests['no_new_tests'] is True
assert len(tests['scopes']) == len(settings['expected_unit_scopes']) == 3
tested_names = []
test_copy = out / 'preparation-tests'
test_copy.mkdir()
for actual, expected in zip(tests['scopes'], settings['expected_unit_scopes']):
    assert actual['filter'] == expected['filter']
    assert actual['expected_names'] == expected['names']
    assert actual['expected_existing_tests'] == expected['count']
    assert actual['source'] == source and actual['driver'] == settings['origin_driver']
    assert actual['passed'] is True and actual['exit'] == 0
    assert actual['existing_test_count_confirmed'] is True
    assert actual['named_existing_tests_confirmed'] is True
    assert len(actual['channels']) == 2
    output_text = ''
    for name, channel in actual['channels'].items():
        assert Path(name).name == name and name.endswith(('.stdout', '.stderr'))
        raw = (retained / name).read_bytes()
        assert len(raw) == channel['bytes'] and sha(raw) == channel['sha256']
        (test_copy / name).write_bytes(raw)
        output_text += raw.decode(errors='replace')
    count = expected['count']
    assert re.search(r'test result: ok[.] ' + str(count) + r' passed; 0 failed|'
                     + str(count) + r' tests run: ' + str(count) + r' passed', output_text)
    assert all(name in output_text for name in expected['names'])
    tested_names.extend(expected['names'])
assert len(tested_names) == len(set(tested_names)) == 27
(test_copy / 'term-comparison-tests.json').write_text(json.dumps(tests, indent=2) + '\n')
producer_dir = (retained / settings['producer_record']).parent
for kind in ('cli', 'runner'):
    assert original[kind] and original[kind + '_profile']
    profile = read(producer_dir / (Path(original[kind]).name + '.verified-profile.json'))
    assert profile['source_revision'] == source and profile['source_tree'] == settings['tree']
    assert profile['observation'] == original[kind + '_profile']
cli = (producer_dir / Path(original['cli']).name).resolve()
runner = (producer_dir / Path(original['runner']).name).resolve()
for binary in (cli, runner):
    binary.chmod(0o755)
profiles = {kind: compiler_pilot.corpus_gate.require_optimized(binary)
            for kind, binary in [('cli', cli), ('runner', runner)]}


def identity(profile):
    result = {k: v for k, v in profile.items() if k != 'binary'}
    if isinstance(result.get('build_receipt'), dict):
        result['build_receipt'] = {k: v for k, v in result['build_receipt'].items() if k != 'path'}
    return result


for kind in ('cli', 'runner'):
    assert identity(profiles[kind]) == identity(original[kind + '_profile'])
    assert profiles[kind]['profile']['opt_level'] == '1'
producer = compiler_pilot.CompilerProducer(
    original['revision'], original['tree'], original['target'], original['toolchain'],
    cli, runner, profiles['cli'], profiles['runner'], source, settings['compiler_inputs'],
    dict(run=settings['origin_run'], driver=settings['origin_driver'],
         artifact=settings['artifact_id'], transport='Full SHA256 authenticated immutable original artifact'))
os.environ['GITHUB_SHA'] = source
producer.verify()
(out / 'retained-origin.json').write_text(json.dumps(dict(
    driver=driver, program_source=source, settings=settings, original_producer=original,
    original_run_status=origin['status'], original_run_conclusion=origin['conclusion'], artifact=artifact,
    original_preparation_tests=tests,
    archive_sha256=digest.hexdigest(), archive_members=members, actual_profiles=profiles,
    staged_observer=stage_record, host=dict(system=platform.system(), machine=platform.machine()),
    builds_rust=False), indent=2) + '\n')
code = compiler_pilot.run_suite(settings['label'], (), (), producer=producer,
    fixtures=','.join(settings['source_checks']))
verify_inputs()
producer.verify()
sample_command = [sys.executable, str(staged / 'installed_entry_samples.py'), '--cli', str(cli),
    '--source', source, '--manifest', str(manifest_path), '--output', str(out / 'samples')]
sample_result = subprocess.run(sample_command, stdin=subprocess.DEVNULL)
verify_inputs()
producer.verify()
latencies = {}
for name, roster in [('checking', settings['source_checks']), ('both', settings['both'])]:
    path = out / (name + '.json')
    report = read(path) if path.is_file() else {}
    rows = report.get('observations', [])
    complete = len(rows) == len(roster) and {r['fixture'] for r in rows} == set(roster)
    within = complete and all(type(r.get('millis')) is int and 0 <= r['millis'] <= 120000 for r in rows)
    latencies[name] = dict(complete=complete, within_120000ms=within,
        observations=[dict(fixture=r['fixture'], status=r['status'], millis=r.get('millis')) for r in rows])
sample_path = out / 'samples/samples.json'
samples = read(sample_path) if sample_path.is_file() else []
sample_ok = sample_result.returncode == 0 and len(samples) == 19 and {
    r['sample'] for r in samples} == set(settings['samples']) and all(
    r['state'] == ('rejected' if r['sample'].endswith('/invalid') else 'pass') for r in samples)
budgets_ok = all(v['within_120000ms'] for v in latencies.values())
result_code = code or (0 if sample_ok and budgets_ok else 1)
(out / 'observer-result.json').write_text(json.dumps(dict(
    driver=driver, actual_compiler_program_source=source, origin_run=settings['origin_run'],
    standard_corpus_exit=code, sample_gate_exit=sample_result.returncode,
    source_and_both_latencies=latencies, original_corpus_budget_met=budgets_ok,
    complete_samples=sample_ok, profiles_verified_after=True, builds_rust=False, exit=result_code),
    indent=2) + '\n')
raise SystemExit(result_code)
