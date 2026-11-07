"""Restore one observed positive and run its one ignored forwarding-omission test.

No producer or positive program is rebuilt or rerun. Archive and selected payload
hashes must verify before preparing the optimized codec test image.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import platform
import shutil
import stat
import subprocess
import sys
import time
import zipfile

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
OUT = ROOT / 'build/supplied-output-loan/ci-installed-empty-component'
CAPTURE = OUT / 'capture'
PINS = {
    'compiler-inputs.json': '12bc18c28617ae4d2fe31f15e899b5bf4bdee7059717b13908ca99990d3dc16d',
    'capture-binding.json': 'c84f2204275ecb8c9c97bbe671708436f4541a6cab8c624504cc384c40a8d27e',
    'transport-plan.json': '9a81acde8c9702ead192208a9dbd62e32f0db692ddd99272dc1f2373ced0a6f7',
    'qualify_installed_empty.py': '0655921494307e88263a190941aaa4ea740c8f98f56e17ed54e94c4a63c24c33',
    'ci_steps.sh': '3112c3ec8b07af68ce3dfd7485599df819bf58fb88c0fd14de96352bda4d72bd',
}
SOURCE = '88acbdf1f4eb3b7eecb72b7e566d2699624f6686'
TEST_PATH = 'omega-rust/psi/compilation/representations/terminal-psi/codec/src/sections/semantic_module/reference_forwarding_tests.rs'
TEST_SHA = '6ba5e0d3edabe01771b6976df388114b2e8db7e6b60630eb8262cc961967e766'

def sha(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()

def require(condition, reason):
    if not condition:
        raise ValueError(reason)

def read(path):
    return json.loads(Path(path).read_text())

def git(*args):
    row = subprocess.run(['git', *args], cwd=ROOT, capture_output=True,
                         text=True, timeout=30)
    row.check_returncode()
    return row.stdout.strip()

def preflight():
    require(platform.system() == 'Linux' and platform.machine().lower() == 'x86_64',
            'this capture requires its original Linux x86_64 host')
    revision = git('rev-parse', 'HEAD')
    require(os.environ.get('GITHUB_SHA') == revision and not git('status', '--porcelain'),
            'test checkout is not its clean actual CI head')
    for name, expected in PINS.items():
        path = HERE / name
        require(path.is_file() and not path.is_symlink() and sha(path) == expected,
                'reviewed component input changed: ' + name)
    roster = read(HERE / 'compiler-inputs.json')
    require(roster['schema'] == 'omega-reviewed-compiler-input-map-v1'
            and roster['compiled_source'] == SOURCE, 'reviewed source map changed')
    expected = roster['all_ten_compiler_input_objects']
    require(set(expected) == {'omega-rust', 'source/library', 'source/psi', 'source/omega',
            'bootstrap', 'tools/benchmark', 'Cargo.toml', 'Cargo.lock',
            'rust-toolchain.toml', '.cargo'}, 'source map is incomplete')
    for name, identity in expected.items():
        row = subprocess.run(['git', 'rev-parse', '--verify', 'HEAD:' + name],
                             cwd=ROOT, capture_output=True, text=True, timeout=30)
        require((row.stdout.strip() if row.returncode == 0 else None) == identity,
                'compiled source differs: ' + name)
        require(identity is not None or name == '.cargo', 'missing required source input')
    require(sha(ROOT / TEST_PATH) == TEST_SHA, 'the adopted ignored test changed')
    binding, plan = read(HERE / 'capture-binding.json'), read(HERE / 'transport-plan.json')
    require(binding['schema'] == 'omega-installed-empty-capture-binding-v1'
            and plan['schema'] == 'omega-installed-empty-transport-plan-v1',
            'unknown capture transport schema')
    require(binding['compiled_source'] == SOURCE
            and binding['all_ten_compiler_input_objects'] == expected
            and binding['capture_run'] == plan['capture_run']
            and binding['capture_ci'] == plan['capture_ci'], 'capture and test source differ')
    require(binding['original_kept_product_exported'] is False
            and plan['original_kept_product_exported'] is False
            and plan['original_kept_locator'] not in binding['restored_files'],
            'the unexported original must never be recreated')
    library = {name: sha(ROOT / name) for name in
               git('ls-files', '-z', '--', 'source/library').split('\0') if name}
    require(library == binding['library_input_sha256'], 'bundled library differs')
    for name in library:
        path = ROOT / name
        require(not path.is_symlink() and path.resolve().is_relative_to(ROOT / 'source/library'),
                'bundled library escapes its owner')
    require({p.relative_to(ROOT).as_posix() for p in (ROOT / 'source/library').rglob('*.omg')
             if p.is_file()} == {p for p in library if p.endswith('.omg')},
            'extra or missing bundled module')
    require(len(plan['files']) == len(binding['restored_files']) == 20,
            'capture file closure is incomplete')
    require(len({row['locator'] for row in plan['files']}) == 20
            and len({row['member'] for row in plan['files']}) == 20,
            'capture transport has duplicate source or destination identities')
    for row in plan['files']:
        require(binding['restored_files'][row['locator']] ==
                {'member': row['member'], 'sha256': row['sha256']},
                'transport association changed')
    return revision, binding, plan

def normalized(name):
    path = PurePosixPath(name)
    require(name and not path.is_absolute() and str(path) == name
            and all(part not in ('.', '..') for part in path.parts)
            and '\\' not in name, 'unsafe archive member')
    return path

def restore(archive, selected, expected_count):
    restored = []
    with zipfile.ZipFile(archive) as data:
        entries = data.infolist()
        require(len(entries) == expected_count
                and len({row.filename for row in entries}) == len(entries),
                'archive member roster changed or contains duplicates')
        for entry in entries:
            normalized(entry.filename.rstrip('/') if entry.is_dir() else entry.filename)
        for row in selected:
            entry = data.getinfo(row['archive_member'])
            require(not entry.is_dir() and not stat.S_ISLNK(entry.external_attr >> 16)
                    and stat.S_IFMT(entry.external_attr >> 16) in (0, stat.S_IFREG),
                    'selected export is not an ordinary file')
            require(entry.file_size == row['bytes'], 'selected export size changed')
            destination = CAPTURE.joinpath(*normalized(row['member']).parts)
            require(destination.resolve().is_relative_to(CAPTURE), 'restored path escaped')
            destination.parent.mkdir(parents=True, exist_ok=True)
            require(shutil.disk_usage(OUT).free >= 800_000_000 + entry.file_size,
                    'restoration capture floor is unavailable')
            digest = hashlib.sha256()
            with data.open(entry) as source, destination.open('xb') as target:
                for block in iter(lambda: source.read(1024 * 1024), b''):
                    target.write(block)
                    digest.update(block)
            require(digest.hexdigest() == row['sha256'], 'restored payload changed')
            os.chmod(destination, row['mode'])
            restored.append(dict(row, actual_path=str(destination), actual_sha256=sha(destination)))
    return restored

def invoke(component, arguments):
    previous = sys.argv
    try:
        sys.argv = [str(HERE / 'qualify_installed_empty.py'), *map(str, arguments)]
        return component.main()
    finally:
        sys.argv = previous

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--preflight-only', action='store_true')
    args = parser.parse_args()
    revision, binding, plan = preflight()
    if args.preflight_only:
        print('INSTALLED_EMPTY_SOURCE_PREFLIGHT_PASSED', revision, flush=True)
        return 0
    OUT.mkdir(parents=True, exist_ok=False)
    report = dict(schema='omega-installed-empty-restored-ci-v1', passed=False,
                  test_ci=dict(revision=revision, tree=git('rev-parse', 'HEAD^{tree}')),
                  capture_ci=binding['capture_ci'], capture_run=binding['capture_run'],
                  compiler_source=SOURCE, full98_accepted=False,
                  source_use_attribution='pending-independent-current-artifact-review',
                  component_acceptance_complete=False, slot_released=False,
                  restoration_complete=False,
                  post_source_and_restored_files_unchanged=False)
    def validate():
        require(preflight()[0] == revision, 'test source changed')
    try:
        spec = importlib.util.spec_from_file_location(
            'reviewed_installed_empty', HERE / 'qualify_installed_empty.py')
        component = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(component)
        sys.path.insert(0, str(ROOT / 'tools'))
        import build_slot
        import compiler_profile
        environment = component.environment(ROOT, compiler_profile)
        transport = OUT / 'transport'
        transport.mkdir()
        CAPTURE.mkdir()
        restored = []
        report['restored_files'] = restored
        for key, description in plan['archives'].items():
            if description['kind'] == 'github':
                artifact_id = str(description['id'])
                metadata = component.command(ROOT, transport, key + '-metadata',
                    ['gh', 'api', 'repos/CathedralOS/Omega/actions/artifacts/' + artifact_id],
                    environment, validate, 60)
                actual = read(metadata['stdout'])
                require(actual['id'] == description['id'] and not actual['expired']
                        and actual['name'] == description['name']
                        and actual['size_in_bytes'] == description['bytes']
                        and actual['digest'] == 'sha256:' + description['sha256']
                        and actual['workflow_run']['id'] == binding['capture_run']['run_id']
                        and actual['workflow_run']['head_sha'] == binding['capture_ci']['revision'],
                        'artifact metadata is not the pinned capture')
                require(shutil.disk_usage(OUT).free >= 800_000_000 + description['bytes'],
                        'archive download capture floor is unavailable')
                download = component.command(ROOT, transport, key + '-archive',
                    ['gh', 'api', 'repos/CathedralOS/Omega/actions/artifacts/' + artifact_id + '/zip'],
                    environment, validate, 600)
                archive = Path(download['stdout'])
            else:
                archive = ROOT / description['path']
                require(not archive.is_symlink(), 'frozen archive is not an ordinary tracked file')
            require(archive.stat().st_size == description['bytes']
                    and sha(archive) == description['sha256'], 'archive bytes differ')
            restored.extend(restore(archive,
                [row for row in plan['files'] if row['archive'] == key],
                description['member_count']))
        require(len(restored) == len(plan['files']), 'capture restoration is incomplete')
        report['restoration_complete'] = True
        def captured(locator):
            row = binding['restored_files'][locator]
            path = CAPTURE / row['member']
            require(not path.is_symlink() and sha(path) == row['sha256'],
                    'restored capture changed')
            return path
        acceptance = read(captured(plan['acceptance_locator']))
        require(acceptance['accepted'] is False
                and acceptance['failures'] == plan['full98_failures'],
                'unrelated full98 failure was lost')
        report['full98_failures'] = acceptance['failures']
        for locator in binding['restored_files']:
            captured(locator)
        common = ['--repository', ROOT, '--expected-compiler-source', SOURCE,
                  '--compiler-input-roster', HERE / 'compiler-inputs.json',
                  '--compiler-input-roster-sha256', PINS['compiler-inputs.json']]
        started = time.monotonic()
        with build_slot.hold(ROOT) as (slot, waited):
            require(slot is not None, 'build hold unavailable')
            report['slot'] = dict(path=str(slot), wait_seconds=waited, one_hold_for_chain=True)
            report['prepare_exit'] = invoke(component, [
                'prepare', *common, '--out', OUT / 'codec-image'])
            if report['prepare_exit'] == 0:
                prepared = read(OUT / 'codec-image/preparation.json')
                report['qualify_exit'] = invoke(component, [
                    'qualify', *common, '--capture-binding', HERE / 'capture-binding.json',
                    '--capture-binding-sha256', PINS['capture-binding.json'],
                    '--capture-root', CAPTURE,
                    '--preparation', captured(binding['preparation_locator']),
                    '--results', captured(binding['results_locator']),
                    '--producer-image', captured(binding['producer']['cli']),
                    '--receiver-image', captured(binding['producer']['cli']),
                    '--source', captured(plan['source_locator']),
                    '--build', captured(plan['build_locator']),
                    '--artifact', captured(plan['artifact_locator']),
                    '--test-preparation', OUT / 'codec-image/preparation.json',
                    '--test-image', prepared['retained_image'], '--out', OUT / 'omission'])
        report['slot_released'] = True
        report['hold_seconds'] = round(time.monotonic() - started, 3)
        require(report['prepare_exit'] == 0 and report.get('qualify_exit') == 0,
                'codec preparation or ignored omission execution failed')
        result = read(OUT / 'omission/result.json')
        require(result['passed'] is True and result['actual_passes'] == 1
                and result['component_acceptance_complete'] is False
                and result['source_use_attribution'] == 'pending-independent-current-artifact-review',
                'component result is incomplete or overclaims acceptance')
        report['component_observed'] = True
        report['passed'] = True
    except BaseException as error:
        report['error'] = type(error).__name__ + ': ' + str(error)
    finally:
        post_errors = []
        try:
            validate()
            report['post_source_unchanged'] = True
        except BaseException as error:
            report['post_source_unchanged'] = False
            post_errors.append(type(error).__name__ + ': ' + str(error))
        states = []
        for row in plan['files']:
            path = CAPTURE / row['member']
            state = dict(member=row['member'], expected_sha256=row['sha256'])
            try:
                metadata = path.lstat()
                require(stat.S_ISREG(metadata.st_mode) and not path.is_symlink(),
                        'restored evidence is not a regular non-symlink file')
                require(path.resolve().is_relative_to(CAPTURE),
                        'restored evidence escaped its capture root')
                state['actual_sha256'] = sha(path)
                require(state['actual_sha256'] == row['sha256'],
                        'restored evidence changed after failure')
                state['status'] = 'verified'
            except FileNotFoundError:
                state['status'] = 'missing'
            except BaseException as error:
                state['status'] = 'invalid'
                state['error'] = type(error).__name__ + ': ' + str(error)
            states.append(state)
        report['post_restored_file_states'] = states
        if report['restoration_complete']:
            all_verified = (len(states) == len(plan['files'])
                            and all(row['status'] == 'verified' for row in states))
            if not all_verified:
                post_errors.append('one or more completed capture files are missing or changed')
            report['post_source_and_restored_files_unchanged'] = (
                report['post_source_unchanged'] and all_verified)
        else:
            report['partial_restoration'] = True
            report['passed'] = False
        if post_errors:
            report['passed'] = False
            report['post_error'] = '; '.join(post_errors)
        (OUT / 'ci-result.json').write_text(json.dumps(report, indent=2) + '\n')
        print('INSTALLED_EMPTY_COMPONENT_OBSERVED', report['passed'], flush=True)
    return 0 if report['passed'] else 1

if __name__ == '__main__':
    raise SystemExit(main())
