"""Materialize one immutable producer, then observe two unchanged Source checks for baseline attribution."""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import urllib.error
import urllib.request
import zipfile

CREDENTIAL_KEYS = ('GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN',
                   'ACTIONS_ID_TOKEN_REQUEST_TOKEN', 'ACTIONS_ID_TOKEN_REQUEST_URL')


def remove_workload_credentials():
    for key in CREDENTIAL_KEYS:
        os.environ.pop(key, None)
    assert not any(key in os.environ for key in CREDENTIAL_KEYS)


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('phase', choices=('materialize', 'check'))
parser.add_argument('--recipe', type=Path, required=True)
args = parser.parse_args()
if args.phase == 'check':
    remove_workload_credentials()
staged = args.recipe.resolve().parent
recipe = json.loads(args.recipe.read_text())
root = Path.cwd().resolve()
output = root / 'build/installed-entry-c417-attribution'
sha = lambda b: hashlib.sha256(b).hexdigest()
read = lambda p: json.loads(p.read_text())
git = lambda *a: subprocess.check_output(['git', *a], text=True).strip()
driver = os.environ['GITHUB_SHA']
source = recipe['source']
assert git('rev-parse', 'HEAD') == source
assert git('rev-parse', 'HEAD^{tree}') == recipe['tree']
assert not git('status', '--porcelain')
assert os.environ['OMEGA_CORPUS_JOBS'] == '1' and os.environ['OMEGA_CORPUS_FIXTURE_SECS'] == '120'
assert not os.environ.get('OMEGA_PHASE_PROFILE')
stage_record = read(staged / 'staged-observer.json')
assert stage_record['observer'] == driver
for path, digest in stage_record['files'].items():
    assert sha((staged / path).read_bytes()) == digest
for path, digest in recipe['observer_files'].items():
    assert stage_record['files'][path] == digest
manifest = read(staged / 'installed_entry_empty_baseline_inputs.json')
assert len(manifest['inputs']) == 375
sys.path[:0] = [str(root / 'tools'), str(root / 'tools/ci')]
import compiler_pilot
from compiler_replay_plan import input_objects
assert compiler_pilot.corpus_gate.host_target() == recipe['target'] == 'linux_x86_64'
assert platform.machine() == 'x86_64'
assert input_objects(root, source) == recipe['compiler_inputs']
positive, negative = compiler_pilot.select_fixtures(','.join(recipe['fixtures']))
assert positive == (recipe['fixtures'][0],) and negative == (recipe['fixtures'][1],)


def verify_inputs():
    assert git('rev-parse', 'HEAD') == source and not git('status', '--porcelain')
    for path, digest in manifest['inputs'].items():
        assert sha((root / path).read_bytes()) == digest


verify_inputs()
os.environ['GITHUB_SHA'] = source
if args.phase == 'materialize':
    output.mkdir(parents=True, exist_ok=False)
    api = lambda p: json.loads(subprocess.check_output(['gh', 'api', p], text=True))
    origin = api(f"repos/CathedralOS/Omega/actions/runs/{recipe['origin_run']}")
    assert origin['head_sha'] == source and origin['status'] == 'completed' and origin['conclusion'] == 'success'
    artifact = api(f"repos/CathedralOS/Omega/actions/artifacts/{recipe['artifact']['id']}")
    assert not artifact['expired'] and artifact['workflow_run']['id'] == recipe['origin_run']
    assert artifact['workflow_run']['head_sha'] == source
    for key in ('name', 'size_in_bytes', 'digest'):
        assert artifact[key] == recipe['artifact'][key]
    retained = output / 'retained'
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
        raise ValueError('Expected signed immutable artifact redirect')
    except urllib.error.HTTPError as error:
        assert error.code == 302
        url = error.headers['Location']
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
        assert len({i.filename for i in infos}) == len(infos)
        assert [dict(name=i.filename, bytes=i.file_size, compressed_bytes=i.compress_size) for i in infos] == recipe['archive_index']
        for info in infos:
            path = PurePosixPath(info.filename)
            assert not path.is_absolute() and '..' not in path.parts and '\\' not in info.filename
            assert path.as_posix() == info.filename and (info.external_attr >> 16) & 0o170000 != 0o120000
            if info.is_dir():
                continue
            target = retained / path
            saved = info.filename in selected
            if saved:
                target.parent.mkdir(parents=True, exist_ok=True)
            channel = target.open('xb') if saved else None
            member_sha = hashlib.sha256()
            count = 0
            try:
                with archive.open(info) as stream:
                    while chunk := stream.read(4 * 1024 * 1024):
                        count += len(chunk)
                        assert count <= info.file_size
                        member_sha.update(chunk)
                        if channel:
                            channel.write(chunk)
            finally:
                if channel:
                    channel.close()
            assert count == info.file_size
            members.append(dict(name=info.filename, bytes=count, sha256=member_sha.hexdigest(), saved=saved))
    assert {m['name'] for m in members if m['saved']} == selected
    original = read(retained / recipe['producer_record'])
    assert original == recipe['original_producer']
    summary = read(retained / 'compiler-pilot/summary.json')
    assert summary['passed'] and summary['candidate'] == source and summary['producer'] == original
    assert original['tree'] == recipe['tree'] and original['target'] == recipe['target']
    assert read(retained / 'compiler-pilot/source/manifest.json') == recipe['original_source_manifest']
    producer_dir = (retained / recipe['producer_record']).parent
    # All remote API/archive/source authentication is complete. Profile
    # queries execute authenticated binaries with no workflow credentials.
    remove_workload_credentials()
    profiles = {}
    binaries = {}
    for kind in ('cli', 'runner'):
        name = Path(original[kind]).name
        verified = read(producer_dir / (name + '.verified-profile.json'))
        assert verified['source_revision'] == source and verified['source_tree'] == recipe['tree']
        assert verified['observation'] == original[kind + '_profile']
        binary = (producer_dir / name).resolve()
        assert sha(binary.read_bytes()) == original[kind + '_profile']['sha256']
        binary.chmod(0o755)  # Only the authenticated copied workload executable.
        actual = compiler_pilot.corpus_gate.require_optimized(binary)
        assert {k: v for k, v in actual.items() if k != 'binary'} == {
            k: v for k, v in original[kind + '_profile'].items() if k != 'binary'}
        assert actual['profile']['opt_level'] == '1'
        profiles[kind], binaries[kind] = actual, binary
    producer = compiler_pilot.CompilerProducer(source, recipe['tree'], recipe['target'], original['toolchain'],
        binaries['cli'], binaries['runner'], profiles['cli'], profiles['runner'])
    producer.verify()
    verify_inputs()
    (output / 'producer.json').write_text(json.dumps(dict(schema='omega-compiler-producer-v1', producer=producer.record()), indent=2) + '\n')
    binding = dict(observer=driver, compiler_program_source=source, recipe=recipe, staged_observer=stage_record,
        origin=origin, artifact=artifact, archive_bytes=size, archive_sha256=digest.hexdigest(), members=members,
        actual_profiles=profiles, input_count=375, host=platform.uname()._asdict(),
        workload_credentials_removed=True, builds_rust=False)
    (output / 'preparation.json').write_text(json.dumps(binding, indent=2) + '\n')
    print(json.dumps(dict(protocol='installed-entry-c417-attribution-v1', kind='prepared', observer=driver,
        source=source, input_count=375, profiles=profiles, archive_sha256=digest.hexdigest(), builds_rust=False)), flush=True)
    raise SystemExit(0)

producer = compiler_pilot.load_producer(output / 'producer.json')
producer.verify()
assert producer.revision == source
assert producer.runner_profile['sha256'] == recipe['original_producer']['runner_profile']['sha256']
assert producer.cli_profile['sha256'] == recipe['original_producer']['cli_profile']['sha256']
verify_inputs()
result = compiler_pilot.run_suite(
    'installed-entry-c417-attribution', (), (), fixtures=','.join(recipe['fixtures']),
    check_only=True, phase='consume',
)
verify_inputs()
assert not (output / 'both.json').exists(), 'Source-only attribution cannot create an execution observation'
record = dict(observer=driver, compiler_program_source=source, original_origin=recipe['origin_run'],
    producer=producer.record(), scope=recipe['scope'], canonical_exit=result,
    input_count=375, inputs_unchanged=True, workload_credentials_removed=True, Rust_builds=0)
(output / 'attribution.json').write_text(json.dumps(record, indent=2) + '\n')
raise SystemExit(result)
