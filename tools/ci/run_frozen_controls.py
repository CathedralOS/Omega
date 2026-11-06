"""Prepare exact frozen inputs and package locks, then observe both fixed groups.

All program bytes and oracles stay unchanged. Pending review decisions use the
same recorded accept/resume path as the ordinary CLI fixture consumer.
"""

import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import stat
import subprocess
import sys
import time
import zipfile

ROOT = Path(__file__).resolve().parents[2]
CI = ROOT / 'tools/ci'
ARCHIVE = CI / 'frozen_controls_inputs.zip'
TRANSPORT = CI / 'frozen_controls_transport.json'
CONSUMER = CI / 'qualify_current_frozen_controls.py'
PINS = {
    ARCHIVE: '33a324fbe0f993a119a2cd14e54131806af915b8ab4891fceb1d47b872942a06',
    TRANSPORT: '7fdac256d292b9b6adc87882f23f356e9526c38b758f78a3e2cd055c92ddfa12',
    CONSUMER: 'd1917ac12cb7e0e366cb840194014158ab48ce8f93d157639076fc7e09f7b66e',
}
COMPILER_SOURCE = '2e9a696a90f329dccb6e0cc40b5cb610e7ed98b6'


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    os.chdir(ROOT)
    for path, identity in PINS.items():
        assert sha(path) == identity, str(path)
    transport = json.loads(TRANSPORT.read_text())
    assert transport['schema'] == 'omega-frozen-control-input-transport-v1'
    assert transport['archive']['sha256'] == PINS[ARCHIVE]
    assert transport['archive']['bytes'] == ARCHIVE.stat().st_size
    expected = {entry['path']: entry for entry in transport['member_files']}
    assert len(expected) == len(transport['member_files']) == 270
    checked = {}
    with zipfile.ZipFile(ARCHIVE) as archive:
        entries = archive.infolist()
        assert len(entries) == 270 and {entry.filename for entry in entries} == set(expected)
        assert len({entry.filename.casefold() for entry in entries}) == 270
        for entry in entries:
            name = entry.filename
            path = PurePosixPath(name)
            assert str(path) == name and not path.is_absolute() and '..' not in path.parts
            assert '\\' not in name and '\x00' not in name
            assert path.parts[:2] in {('build', 'supplied-output-loan'), ('tests', 'omega')}
            mode = entry.external_attr >> 16
            assert not entry.is_dir() and stat.S_ISREG(mode) and stat.S_IMODE(mode) == 0o644
            assert entry.file_size == expected[name]['bytes']
            data = archive.read(entry)
            assert hashlib.sha256(data).hexdigest() == expected[name]['sha256']
            checked[name] = data
    assert sum(map(len, checked.values())) == transport['archive']['uncompressed_bytes']

    base = ROOT / 'build/supplied-output-loan'
    inputs = base / 'ci-frozen-inputs'
    observations = base / 'ci-frozen-observations'
    records = ROOT / 'build/compiler-pilot/frozen-controls'
    assert not inputs.exists() and not observations.exists() and not records.exists()
    inputs.mkdir(parents=True, exist_ok=False)
    assert inputs.resolve().is_relative_to(ROOT / 'build')
    for name, data in checked.items():
        destination = inputs / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        assert destination.resolve().is_relative_to(inputs)
        with destination.open('xb') as output:
            output.write(data)
        assert sha(destination) == expected[name]['sha256']
    records.mkdir(parents=True, exist_ok=False)
    receipt = dict(schema='omega-frozen-controls-preparation-v1', passed=False,
                   archive_sha256=PINS[ARCHIVE], transport_sha256=PINS[TRANSPORT],
                   consumer_sha256=PINS[CONSUMER], compiler_source=COMPILER_SOURCE,
                   original_archive_locator=transport['archive']['path'],
                   actual_archive_locator=str(ARCHIVE), isolated_input_root=str(inputs),
                   no_tracked_overwrite=True, materialized_files=270, package_setup=[])

    def retain():
        (records / 'preparation.json').write_text(json.dumps(receipt, indent=2) + '\n')

    retain()
    specification = importlib.util.spec_from_file_location('fixed_control_consumer', CONSUMER)
    consumer = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(consumer)
    sys.path.insert(0, str(ROOT / 'tools'))
    from sample_native_gate import DECISION

    def git(*arguments, optional=False):
        result = subprocess.run(['git', *arguments], cwd=ROOT, stdin=subprocess.DEVNULL,
                                capture_output=True, text=True, timeout=30)
        if optional and result.returncode:
            return None
        result.check_returncode()
        return result.stdout.strip()

    revision = git('rev-parse', 'HEAD')
    preparation_path = ROOT / 'build/compiler-pilot/preparation.json'
    preparation_sha = sha(preparation_path)
    preparation = json.loads(preparation_path.read_text())
    assert preparation['passed'] is True and preparation['candidate'] == revision
    producer = preparation['producer']
    assert producer['revision'] == revision
    # A shallow CI checkout lacks the earlier source commit used by both
    # input checks. Fetch that exact commit without changing the checkout.
    source_fetch = ['fetch', '--no-tags', '--depth=1', 'origin', COMPILER_SOURCE]
    receipt['compiler_source_fetch'] = {'command': ['git', *source_fetch], 'passed': False}
    retain()
    git(*source_fetch)
    assert git('rev-parse', COMPILER_SOURCE + '^{commit}') == COMPILER_SOURCE
    receipt['compiler_source_fetch']['passed'] = True
    retain()
    compiler_inputs = {name: git('rev-parse', COMPILER_SOURCE + ':' + name, optional=True)
                       for name in consumer.INPUT_PATHS}
    assert all(value is not None for name, value in compiler_inputs.items() if name != '.cargo')
    library = ROOT / 'source/library'
    library_inputs = {name: sha(ROOT / name) for name in git('ls-files', '-z', '--', 'source/library').split('\0') if name}
    library_modules = {name for name in library_inputs if name.endswith('.omg')}
    environment = dict(os.environ, OMEGA_LIBRARY_ROOT=str(library))
    for key in ('OMEGA_CALL_WRITE_TRACE', 'OMEGA_STRUCT_TRACE', 'OMEGA_COMPILER_CHECK_SITES'):
        environment.pop(key, None)
    receipt.update(candidate=revision, producer=producer, producer_preparation_sha256=preparation_sha,
                   compiler_inputs=compiler_inputs, library_root=str(library), library_input_sha256=library_inputs)

    def verify():
        assert git('rev-parse', 'HEAD') == revision and not git('status', '--porcelain')
        assert sha(preparation_path) == preparation_sha
        for path, identity in PINS.items():
            assert sha(path) == identity
        for name, identity in compiler_inputs.items():
            assert git('rev-parse', 'HEAD:' + name, optional=True) == identity, name
        assert {str(path.relative_to(ROOT)).replace(os.sep, '/') for path in library.rglob('*.omg') if path.is_file()} == library_modules
        for name, identity in library_inputs.items():
            path = ROOT / name
            assert path.is_file() and not path.is_symlink() and path.resolve().is_relative_to(library)
            assert sha(path) == identity, name
        for name, entry in expected.items():
            path = inputs / name
            assert path.is_file() and not path.is_symlink() and path.resolve().is_relative_to(inputs)
            assert sha(path) == entry['sha256'], name
        assert consumer.require_optimized(Path(producer['cli'])) == producer['cli_profile']
        assert consumer.require_optimized(Path(producer['runner'])) == producer['runner_profile']

    def command(arguments, directory, stage):
        verify()
        assert shutil.disk_usage(records).free >= 800_000_000
        stdout, stderr = directory / (stage + '.stdout'), directory / (stage + '.stderr')
        started = time.monotonic()
        timed_out = False
        try:
            with stdout.open('xb') as out, stderr.open('xb') as err:
                process = subprocess.Popen(list(map(str, arguments)), cwd=ROOT, env=environment,
                                           stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                           start_new_session=os.name != 'nt',
                                           creationflags=(subprocess.CREATE_NEW_PROCESS_GROUP | 0x4) if os.name == 'nt' else 0)
                with consumer.owned_process_tree(process):
                    try:
                        process.wait(timeout=600)
                    except subprocess.TimeoutExpired:
                        timed_out = True
        finally:
            verify()
        event = dict(stage=stage, command=list(map(str, arguments)), exit=process.returncode,
                     timed_out=timed_out, seconds=round(time.monotonic() - started, 3),
                     stdout=str(stdout), stdout_sha256=sha(stdout), stderr=str(stderr), stderr_sha256=sha(stderr))
        (directory / (stage + '-command.json')).write_text(json.dumps(event, indent=2) + '\n')
        if timed_out:
            raise TimeoutError('package setup timed out')
        return event

    verify()
    positives = []
    for count, roster_path, pin in consumer.ROSTERS:
        assert sha(inputs / roster_path) == pin
        roster = json.loads((inputs / roster_path).read_text())
        for control in (roster['cases'] if count == 60 else roster['fixtures']):
            positive = not control['negative'] if count == 60 else control['positive']
            if positive:
                source = control['source'] if count == 60 else next(
                    entry['staged'] for entry in control['files'] if entry['staged'].endswith('/main.omg'))
                positives.append((count, control.get('name', control.get('fixture')), inputs / source))
    assert len(positives) == 54
    for count, name, source in positives:
        project = source.parent
        assert source.name == 'main.omg' and (project / 'build.omg').is_file()
        directory = records / (str(count) + '-' + name.replace('/', '__'))
        directory.mkdir(exist_ok=False)
        row = dict(group=count, name=name, project=str(project), source_sha256=sha(source), passed=False, commands=[])
        receipt['package_setup'].append(row)
        try:
            event = command([producer['cli'], 'update', '--project', project, '--target', producer['target']], directory, 'update')
            row['commands'].append(event)
            review = project / 'build/package-manager' / ('review-' + producer['target'] + '.txt')
            if review.is_file():
                original = review.read_bytes()
                accepted, pending = DECISION.subn(rb'\1 accept', original)
                (directory / 'review.original.txt').write_bytes(original)
                if pending:
                    assert event['exit'] == 3, 'pending decisions without ReviewRequired'
                    verify()
                    review.write_bytes(accepted)
                    (directory / 'review.accepted.txt').write_bytes(accepted)
                    row['accepted_pending_rows'] = pending
                    event = command([producer['cli'], 'update', '--project', project, '--resume'], directory, 'resume')
                    row['commands'].append(event)
            if event['exit'] != 0 or not (project / 'omega.lock').is_file():
                raise ValueError('package setup refused; must be resolved before qualifying this runtime')
            shutil.copyfile(project / 'omega.lock', directory / 'accepted.omega.lock')
            row['accepted_lock_sha256'] = sha(directory / 'accepted.omega.lock')
            row['passed'] = True
        except (OSError, ValueError, AssertionError, subprocess.SubprocessError) as error:
            row['error'] = type(error).__name__ + ': ' + str(error)
        finally:
            retain()
            verify()
    receipt['package_setup_passed'] = all(row['passed'] for row in receipt['package_setup'])
    receipt['scope'] = 'Setup records never substitute for source/engine/admission observations. Failed setup remains explicit; all98 controls still run and pending rejection pins remain unqualified.'
    receipt['consumer_arguments'] = [str(CONSUMER), '--preparation', str(preparation_path),
                                     '--expected-compiler-source', COMPILER_SOURCE,
                                     '--input-root', str(inputs), '--out', str(observations)]
    retain()
    verify()
    # Reuse the exact reviewed main in this process. Its own finally blocks
    # close every command lifetime, including an interruption of this wrapper.
    previous_arguments = sys.argv
    consumer_code = None
    try:
        sys.argv = list(receipt['consumer_arguments'])
        consumer_code = consumer.main()
        assert isinstance(consumer_code, int)
    except BaseException as error:
        receipt['consumer_error'] = type(error).__name__ + ': ' + str(error)
        raise
    finally:
        sys.argv = previous_arguments
        receipt['consumer_exit'] = consumer_code
        receipt['passed'] = receipt['package_setup_passed'] and consumer_code == 0
        retain()
    return consumer_code if consumer_code != 0 else (0 if receipt['passed'] else 1)


if __name__ == '__main__':
    raise SystemExit(main())
