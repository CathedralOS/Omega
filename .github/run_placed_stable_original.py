"""Authenticate one immutable compiler and run only the frozen original pair."""
from pathlib import Path, PurePosixPath
import hashlib
import json
import os
import subprocess
import sys
import traceback
import urllib.error
import urllib.request
import zipfile

root = Path.cwd().resolve()
recipe = json.loads((root / '.github/placed_stable_original_recipe.json').read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()
git = lambda *args: subprocess.check_output(['git', *args], text=True).strip()
def retain_failure(error_type, error, trace):
    failure = root / 'build/placed-stable-original-acquisition'
    failure.mkdir(parents=True, exist_ok=True)
    (failure / 'failure.json').write_text(json.dumps(dict(passed=False, error_type=error_type.__name__, error=str(error), traceback=''.join(traceback.format_exception(error_type, error, trace)), limits='Acquisition or original consumer failed; no retry or build fallback.'), indent=2) + '\n')
    sys.__excepthook__(error_type, error, trace)

sys.excepthook = retain_failure
source = git('rev-parse', 'HEAD')
assert source == os.environ['GITHUB_SHA']
assert git('rev-parse', 'HEAD^') == recipe['program_base']
assert not git('status', '--porcelain')
assert os.environ['OMEGA_CORPUS_JOBS'] == '1' and os.environ['OMEGA_CORPUS_FIXTURE_SECS'] == '120'
assert not os.environ.get('OMEGA_PHASE_PROFILE')
sys.path[:0] = [str(root / 'tools'), str(root / 'tools/ci')]
import compiler_pilot
from compiler_replay_plan import input_objects

assert compiler_pilot.corpus_gate.host_target() == recipe['producer']['target'] == 'linux_x86_64'
assert input_objects(root, source) == input_objects(root, recipe['producer']['revision']) == recipe['compiler_inputs']
inputs = json.loads((root / '.github/placed_stable_original_inputs.json').read_text())
assert len(inputs) == recipe['input_count'] == 184

def verify_inputs():
    assert git('rev-parse', 'HEAD') == source and not git('status', '--porcelain')
    for path, expected in inputs.items():
        assert sha((root / path).read_bytes()) == expected, path

verify_inputs()
positive, negative = compiler_pilot.select_fixtures(','.join(recipe['fixtures']))
assert positive == ('pass/facts/placed_stable_write_through_target_body_exit',)
assert negative == ('fail/facts/placed_stable_write_through_target_body_rejected',)
output = root / 'build/placed-stable-original-acquisition'
output.mkdir(parents=True, exist_ok=False)
retained = root / 'build/placed-stable-retained-producer'
retained.mkdir(exist_ok=False)
api = lambda path: json.loads(subprocess.check_output(['gh', 'api', path], text=True))
origin = api('repos/CathedralOS/Omega/actions/runs/' + str(recipe['origin_run']))
assert origin['head_sha'] == recipe['producer']['revision'] and origin['status'] == 'completed' and origin['conclusion'] == 'success'
artifact = api('repos/CathedralOS/Omega/actions/artifacts/' + str(recipe['artifact']['id']))
assert not artifact['expired'] and artifact['workflow_run']['id'] == recipe['origin_run']
assert artifact['workflow_run']['head_sha'] == recipe['producer']['revision']
for key in ('id', 'name', 'size_in_bytes', 'digest'):
    assert artifact[key] == recipe['artifact'][key]

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *unused, **kwargs):
        return None

token = subprocess.check_output(['gh', 'auth', 'token'], text=True).strip()
request = urllib.request.Request(artifact['archive_download_url'], headers={'Authorization': 'Bearer ' + token, 'Accept': 'application/vnd.github+json'})
try:
    urllib.request.build_opener(NoRedirect()).open(request, timeout=30)
    raise ValueError('Expected immutable artifact redirect')
except urllib.error.HTTPError as error:
    assert error.code == 302
    url = error.headers['Location']
archive_path = retained / 'original.zip'
digest = hashlib.sha256()
size = 0
with urllib.request.urlopen(url, timeout=90) as response, archive_path.open('xb') as destination:
    while chunk := response.read(4 * 1024 * 1024):
        size += len(chunk)
        assert size <= artifact['size_in_bytes']
        digest.update(chunk)
        destination.write(chunk)
assert size == artifact['size_in_bytes'] and artifact['digest'] == 'sha256:' + digest.hexdigest()
members = []
selected = set(recipe['extract_members'])
with zipfile.ZipFile(archive_path) as archive:
    infos = archive.infolist()
    assert len({item.filename for item in infos}) == len(infos)
    assert [dict(name=item.filename, bytes=item.file_size, compressed_bytes=item.compress_size) for item in infos] == recipe['archive_index']
    for item in infos:
        path = PurePosixPath(item.filename)
        assert not path.is_absolute() and '..' not in path.parts and '\\' not in item.filename
        assert path.as_posix() == item.filename and (item.external_attr >> 16) & 0o170000 != 0o120000
        assert not item.is_dir()
        target = retained / path
        saved = item.filename in selected
        if saved:
            target.parent.mkdir(parents=True, exist_ok=True)
        channel = target.open('xb') if saved else None
        member_sha = hashlib.sha256()
        count = 0
        try:
            with archive.open(item) as stream:
                while chunk := stream.read(4 * 1024 * 1024):
                    count += len(chunk)
                    assert count <= item.file_size
                    member_sha.update(chunk)
                    if channel:
                        channel.write(chunk)
        finally:
            if channel:
                channel.close()
        assert count == item.file_size
        members.append(dict(name=item.filename, bytes=count, sha256=member_sha.hexdigest(), saved=saved))
assert {item['name'] for item in members if item['saved']} == selected
read = lambda path: json.loads(path.read_text())
original = read(retained / recipe['producer_record'])
assert original == recipe['producer']
assert read(retained / 'producer.json')['producer'] == original
preparation = read(retained / 'preparation.json')
assert preparation['passed'] and preparation['candidate'] == original['revision'] and preparation['producer'] == original
assert read(retained / 'source/manifest.json') == recipe['source_manifest'] == preparation['source_exports']
for key in ('GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'ACTIONS_ID_TOKEN_REQUEST_TOKEN', 'ACTIONS_ID_TOKEN_REQUEST_URL'):
    os.environ.pop(key, None)
profiles, binaries = {}, {}
producer_dir = (retained / recipe['producer_record']).parent
for kind in ('cli', 'runner'):
    name = Path(original[kind]).name
    verified = read(producer_dir / (name + '.verified-profile.json'))
    assert verified['source_revision'] == original['revision'] and verified['source_tree'] == original['tree']
    assert verified['observation'] == original[kind + '_profile'] and verified['toolchain'] == original['toolchain']
    binary = (producer_dir / name).resolve()
    assert sha(binary.read_bytes()) == original[kind + '_profile']['sha256']
    binary.chmod(0o755)
    actual = compiler_pilot.corpus_gate.require_optimized(binary)
    assert {key: value for key, value in actual.items() if key != 'binary'} == {key: value for key, value in original[kind + '_profile'].items() if key != 'binary'}
    assert actual['profile']['opt_level'] == '1'
    profiles[kind], binaries[kind] = actual, binary
producer = compiler_pilot.CompilerProducer(original['revision'], original['tree'], original['target'], original['toolchain'], binaries['cli'], binaries['runner'], profiles['cli'], profiles['runner'], source, recipe['compiler_inputs'], dict(run=recipe['origin_run'], artifact=artifact['id'], transport='immutable original producer artifact', plan='.github/placed_stable_original_recipe.json'))
producer.verify()
(output / 'producer.json').write_text(json.dumps(dict(schema='omega-compiler-producer-v1', producer=producer.record()), indent=2) + '\n')
(output / 'acquisition.json').write_text(json.dumps(dict(observer=source, program_base=recipe['program_base'], origin_run=recipe['origin_run'], artifact=artifact, zip_sha256=digest.hexdigest(), zip_bytes=size, members=members, producer=producer.record(), inputs_verified=len(inputs), limits='Full producer authenticated only on hosted runner. No Cargo/cache fallback, new compiler or source/pin edit.'), indent=2) + '\n')
verify_inputs()
status = compiler_pilot.run_suite('placed-stable-original-baseline', (), (), fixtures=','.join(recipe['fixtures']), producer=producer)
verify_inputs()
producer.verify()
raise SystemExit(status)
