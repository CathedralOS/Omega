"""Run four named formation and payload-history controls with authenticated optimized images."""

import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import build_slot
import compiler_profile

SELECTORS = ROOT / 'tools/ci/reference_body_controls_selector.json'
SELECTOR_SHA256 = '15ab761fd5c9caa752cd2d8f1e67220928cfdfac09c8caf2b29af17aacdf51b1'
COMPILER_SOURCE = '88acbdf1f4eb3b7eecb72b7e566d2699624f6686'
EXPECTED_INPUT_OBJECTS = {'omega-rust': 'fcd65fbeb994e220e33ef3b44f681e00ae7ff9f6',
 'source/library': '0d8f76e96537348f44a8e168d83f4d1cc46f7548',
 'source/psi': '400feb4e6c0c556f0fd5d6b44043e92f93d7f30c',
 'source/omega': '96169168741fcd8265771a647ba3bfadd2bc7e8c',
 'bootstrap': '42230c0aa7138a175590d9dd6a57d8ee71934df1',
 'tools/benchmark': '6c1a4399e3f037c808a23ad59825e76664482e2f',
 'Cargo.toml': '2699e0a8d0be49c789639cbd3c603521634145d9',
 'Cargo.lock': 'b98bd3266eea8729b82f99e4cad45a65b3d8a4d9',
 'rust-toolchain.toml': '34677fc928123970d13e8b81641724e2a9e6f929',
 '.cargo': None}


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def git(*arguments, optional=False):
    result = subprocess.run(['git', *arguments], cwd=ROOT, stdin=subprocess.DEVNULL,
                            capture_output=True, text=True, timeout=30)
    if optional and result.returncode:
        return None
    result.check_returncode()
    return result.stdout.strip()


def members(group):
    result = subprocess.run(['ps', '-axo', 'pid=,pgid=,stat=,comm='],
                            capture_output=True, text=True, timeout=10, check=True)
    return [line.strip() for line in result.stdout.splitlines()
            if len(fields := line.split(None, 3)) == 4
            and int(fields[1]) == group and not fields[2].startswith('Z')]


def command(arguments, directory, stem, environment, timeout):
    directory.mkdir(parents=True, exist_ok=True)
    stdout, stderr = directory / (stem + '.stdout'), directory / (stem + '.stderr')
    started = time.monotonic()
    process = None
    row = dict(command=list(map(str, arguments)), timeout_seconds=timeout, timed_out=False)
    try:
        with stdout.open('xb') as out, stderr.open('xb') as err:
            process = subprocess.Popen(list(map(str, arguments)), cwd=ROOT, env=environment,
                                       stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                       start_new_session=True)
            try:
                process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                row['timed_out'] = True
    except BaseException as error:
        row['error'] = type(error).__name__ + ': ' + str(error)
        raise
    finally:
        cleanup_error = None
        if process is not None:
            try:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=10)
                deadline = time.monotonic() + 10
                remaining = members(process.pid)
                while remaining and time.monotonic() < deadline:
                    time.sleep(0.05)  # bounded active process drain, never CI-job polling
                    remaining = members(process.pid)
                row['remaining_owned_processes'] = remaining
                if remaining:
                    raise RuntimeError('command left owned processes running')
            except BaseException as error:
                cleanup_error = error
                row['cleanup_error'] = type(error).__name__ + ': ' + str(error)
            row['exit'] = process.returncode
        row['seconds'] = round(time.monotonic() - started, 3)
        row['stdout'] = str(stdout)
        row['stderr'] = str(stderr)
        for name, path in [('stdout', stdout), ('stderr', stderr)]:
            if path.exists():
                row[name + '_sha256'] = sha(path)
        (directory / (stem + '-command.json')).write_text(json.dumps(row, indent=2) + '\n')
        if cleanup_error is not None:
            raise RuntimeError('command process-group cleanup failed') from cleanup_error
    if row['timed_out'] or row['exit'] != 0:
        raise ValueError('command failed or timed out: ' + stem)
    return row


def test_events(text, group):
    events = [json.loads(line) for line in text.splitlines() if line.strip()]
    starts = [e for e in events if e.get('type') == 'suite' and e.get('event') == 'started']
    endings = [e for e in events if e.get('type') == 'suite' and e.get('event') in ('ok', 'failed')]
    names = [e['name'] for e in events if e.get('type') == 'test' and e.get('event') == 'ok']
    expected = group['expected_nonignored_passes']
    if (len(starts) != 1 or starts[0].get('test_count') != expected
            or len(endings) != 1 or endings[0].get('event') != 'ok'
            or endings[0].get('passed') != expected
            or any(endings[0].get(key) != 0 for key in ('failed', 'ignored', 'measured'))
            or len(names) != expected or len(set(names)) != expected
            or set(names) != set(group['test_names'])
            or any(not name.startswith(group['filter']) for name in names)
            or any(e.get('type') == 'test' and e.get('event') in ('ignored', 'failed') for e in events)):
        raise ValueError('actual test names/nonignored counts differ from pinned selectors')
    return dict(test_names=names, suite=endings[0])


def raw_test_observations(text, group):
    """Keep failed or partial libtest observations separate from acceptance."""
    events, malformed = [], []
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        try:
            event = json.loads(line)
            if not isinstance(event, dict):
                raise ValueError('libtest event is not an object')
            events.append(event)
        except (ValueError, TypeError) as error:
            malformed.append(dict(line=number, error=str(error)))
    terminal = [event for event in events if event.get('type') == 'test'
                and event.get('event') in ('ok', 'failed', 'ignored')]
    names = [event.get('name') for event in terminal]
    expected = group['test_names']
    return dict(passed_full_names=[event.get('name') for event in terminal if event['event'] == 'ok'],
                failed_full_names=[event.get('name') for event in terminal if event['event'] == 'failed'],
                ignored_full_names=[event.get('name') for event in terminal if event['event'] == 'ignored'],
                terminal_events=terminal,
                suite_starts=[event for event in events if event.get('type') == 'suite'
                              and event.get('event') == 'started'],
                suite_endings=[event for event in events if event.get('type') == 'suite'
                               and event.get('event') in ('ok', 'failed')],
                malformed_lines=malformed,
                exact_expected_full_name_set=(len(names) == len(expected)
                    and all(isinstance(name, str) for name in names)
                    and len(set(names)) == len(names) and set(names) == set(expected)),
                diagnostic_only=True)


def main():
    output = ROOT / 'build/compiler-pilot/reference-body-controls'
    output.mkdir(parents=True, exist_ok=False)
    report = dict(schema='omega-current-reference-body-controls-v2', passed=False, mode='NORMAL',
                  expected_total_passes=4, groups=[], compiler_source=COMPILER_SOURCE)

    def retain():
        # Raw events describe execution even when a failed group cannot be accepted.
        report['actual_passes'] = sum(len(group.get('raw_observed', {}).get('passed_full_names', []))
                                      for group in report['groups'])
        report['actual_failures'] = sum(len(group.get('raw_observed', {}).get('failed_full_names', []))
                                        for group in report['groups'])
        report['actual_ignored'] = sum(len(group.get('raw_observed', {}).get('ignored_full_names', []))
                                       for group in report['groups'])
        report['accepted_groups'] = sum(group['passed'] for group in report['groups'])
        report['accepted_passes'] = sum(len(group.get('observed', {}).get('test_names', []))
                                        for group in report['groups'] if group['passed'])
        (output / 'reference-body-controls.json').write_text(json.dumps(report, indent=2) + '\n')
    try:
        assert platform.system() == 'Darwin' and platform.machine().lower() in ('arm64', 'aarch64')
        revision = git('rev-parse', 'HEAD')
        assert os.environ.get('GITHUB_SHA', revision) == revision
        assert sha(SELECTORS) == SELECTOR_SHA256
        selectors = json.loads(SELECTORS.read_text())
        assert selectors['source'] == COMPILER_SOURCE
        assert selectors['expected_total_passes'] == 4
        assert [g['expected_nonignored_passes'] for g in selectors['groups']] == [1, 1, 1, 1]
        assert all(len(g['test_names']) == g['expected_nonignored_passes'] for g in selectors['groups'])

        library_root = ROOT / 'source/library'
        library_inputs = {name: sha(ROOT / name) for name in
                          git('ls-files', '-z', '--', 'source/library').split('\0') if name}
        assert library_inputs
        library_modules = {name for name in library_inputs if name.endswith('.omg')}

        def validate():
            assert git('rev-parse', 'HEAD') == revision and not git('status', '--porcelain')
            assert sha(SELECTORS) == SELECTOR_SHA256
            for name, identity in EXPECTED_INPUT_OBJECTS.items():
                assert git('rev-parse', '--verify', 'HEAD:' + name, optional=True) == identity, name
            for group in selectors['groups']:
                for item in group['source_files']:
                    assert sha(ROOT / item['path']) == item['sha256'], item['path']
            actual_modules = {str(path.relative_to(ROOT)).replace(os.sep, '/')
                              for path in library_root.rglob('*.omg') if path.is_file()}
            assert actual_modules == library_modules, 'library module inventory changed'
            for name, identity in library_inputs.items():
                path = ROOT / name
                assert path.is_file() and not path.is_symlink()
                assert path.resolve().is_relative_to(library_root) and sha(path) == identity, name

        validate()
        tool = 'mbx' if shutil.which('mbx') else 'cargo'
        report.update(candidate=revision, tree=git('rev-parse', 'HEAD^{tree}'), host='macos_arm64',
                      selector_sha256=SELECTOR_SHA256, consumer_sha256=sha(Path(__file__)),
                      compiler_inputs=EXPECTED_INPUT_OBJECTS, actual_library_input_sha256=library_inputs,
                      scope='Only the four named formation and payload-history controls on source 88ac; actual failures remain failed, with no CLI, program runtime or whole-candidate acceptance.',
                      build_tool=tool,
                      build_tool_path=shutil.which(tool), slot_released=False)
        held_at = time.monotonic()
        with build_slot.hold(ROOT) as (slot, waited):
            assert slot is not None
            report['slot'] = dict(path=str(slot), wait_seconds=waited, one_hold_for_all_groups=True)
            environment = compiler_profile.optimized_environment()
            diagnostic_parent = environment.pop('OMEGA_COMPILER_CHECK_SITES', None)
            environment['OMEGA_LIBRARY_ROOT'] = str(ROOT / 'source/library')
            compiler_overrides = ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER',
                                  'RUSTC_WORKSPACE_WRAPPER', 'RUSTC', 'RUSTDOC', 'CARGO_BUILD_RUSTC')
            keys = ['CARGO_TARGET_DIR', 'CARGO_BUILD_JOBS', 'CARGO_INCREMENTAL',
                    'CARGO_BUILD_FINGERPRINT', 'CARGO_UNSTABLE_CHECKSUM_FRESHNESS',
                    'CARGO_PROFILE_DEV_OPT_LEVEL', 'CARGO_PROFILE_TEST_OPT_LEVEL',
                    'RUSTUP_TOOLCHAIN', 'RUST_MIN_STACK', 'OMEGA_BUILD_SLOT_HELD', 'OMEGA_LIBRARY_ROOT',
                    *compiler_overrides]
            report['effective_environment'] = {k: environment.get(k) for k in keys}
            for key in compiler_overrides:
                assert not environment.get(key), 'unreviewed compiler override: ' + key
            report['diagnostic_switch'] = dict(parent_value=diagnostic_parent, child_present=False)
            command(['rustc', '-Vv'], output, 'rustc-version', environment, 30)
            command(['cargo', '--version'], output, 'cargo-version', environment, 30)
            toolchain = (output / 'rustc-version.stdout').read_text().strip()
            assert toolchain.startswith('rustc 1.100.0-nightly (a69a63265 2026-09-03)')
            assert 'host: aarch64-apple-darwin' in toolchain
            report['toolchain'] = toolchain
            report['cargo_version'] = (output / 'cargo-version.stdout').read_text().strip()
            if tool == 'mbx':
                command(['mbx', '--version'], output, 'mbx-version', environment, 30)
                report['mbx_version'] = (output / 'mbx-version.stdout').read_text().strip()
            for number, group in enumerate(selectors['groups'], 1):
                validate()
                directory = output / (str(number) + '-' + group['package'])
                row = dict(selector=group, passed=False)
                image = retained = before = None
                report['groups'].append(row)
                try:
                    build = [tool, 'test', '--locked', '--lib', '-p', group['package'],
                             '--no-run', '--message-format=json']
                    row['build'] = command(build, directory, 'build', environment, 1800)
                    validate()
                    messages = [json.loads(line) for line in (directory / 'build.stdout').read_text().splitlines()
                                if line.startswith('{')]
                    endings = [m for m in messages if m.get('reason') == 'build-finished']
                    assert len(endings) == 1 and endings[0].get('success') is True
                    candidates = [m for m in messages if m.get('reason') == 'compiler-artifact'
                                  and m.get('executable') and m.get('profile', {}).get('test') is True
                                  and m.get('target', {}).get('kind') == ['lib']
                                  and m['target']['name'] == group['package'].replace('-', '_')]
                    assert len(candidates) == 1, 'Cargo did not select exactly one library test image'
                    artifact = candidates[0]
                    image = Path(artifact['executable'])
                    assert image.is_file() and not image.is_symlink() and os.access(image, os.X_OK)
                    codegen = compiler_profile.artifact_codegen(artifact)
                    assert codegen['opt_level'] == str(artifact['profile']['opt_level'])
                    assert codegen['opt_level'] in ('1', '2', '3', 's', 'z')
                    compiler_profile.require_executable_image(image, image.read_bytes())
                    before = sha(image)
                    retained = directory / 'test-image'
                    shutil.copy2(image, retained)
                    assert sha(retained) == before == sha(image)
                    row.update(cargo_artifact=artifact, effective_codegen=codegen,
                               original_image=str(image), retained_image=str(retained), image_sha256=before)
                    validate()
                    row['test'] = command([str(retained), group['filter'], '-Z', 'unstable-options',
                                           '--format=json', '--nocapture', '--test-threads=1'],
                                          directory, 'test', environment, 300)
                    validate()
                    row['observed'] = test_events((directory / 'test.stdout').read_text(), group)
                    row['passed'] = True
                except (OSError, ValueError, AssertionError, subprocess.SubprocessError) as error:
                    row['error'] = type(error).__name__ + ': ' + str(error)
                finally:
                    # Never replace the command/test failure with a later guard failure.
                    post_errors = []
                    for stem in ('build', 'test'):
                        metadata = directory / (stem + '-command.json')
                        if metadata.is_file():
                            try:
                                row[stem] = json.loads(metadata.read_text())
                            except BaseException as error:
                                post_errors.append(dict(guard=stem + ' command receipt',
                                    error=type(error).__name__ + ': ' + str(error)))
                    try:
                        validate()
                        row['source_library_identity_after'] = True
                    except BaseException as error:
                        row['source_library_identity_after'] = False
                        post_errors.append(dict(guard='source/library identity',
                            error=type(error).__name__ + ': ' + str(error)))
                    if before is not None:
                        row['image_identity_after'] = {}
                        for label, path in (('original', image), ('retained', retained)):
                            try:
                                assert path.is_file() and not path.is_symlink()
                                actual = sha(path)
                                row['image_identity_after'][label] = actual
                                assert actual == before, label + ' test image bytes changed'
                            except BaseException as error:
                                post_errors.append(dict(guard=label + ' image identity',
                                    error=type(error).__name__ + ': ' + str(error)))
                        try:
                            actual_codegen = compiler_profile.artifact_codegen(row['cargo_artifact'])
                            row['effective_codegen_after'] = actual_codegen
                            assert actual_codegen == row['effective_codegen'], 'producer fingerprint changed'
                        except BaseException as error:
                            post_errors.append(dict(guard='producer profile identity',
                                error=type(error).__name__ + ': ' + str(error)))
                    raw = directory / 'test.stdout'
                    if raw.is_file():
                        try:
                            row['raw_observed'] = raw_test_observations(raw.read_text(), group)
                        except BaseException as error:
                            post_errors.append(dict(guard='raw libtest observation',
                                error=type(error).__name__ + ': ' + str(error)))
                    row['post_guard_errors'] = post_errors
                    row['post_guards_passed'] = not post_errors
                    if post_errors:
                        row['passed'] = False
                        row.setdefault('error', 'post-command identity/observation guard failed')
                    retain()
            validate()
        report['slot_released'] = True
        report['whole_hold_seconds'] = round(time.monotonic() - held_at, 3)
        retain()
        report['passed'] = len(report['groups']) == 4 and all(g['passed'] for g in report['groups']) and report['accepted_passes'] == 4
        return 0 if report['passed'] else 1
    except BaseException as error:
        report['error'] = type(error).__name__ + ': ' + str(error)
        return 1
    finally:
        retain()
        print('REFERENCE_BODY_CONTROL_OBSERVATION_PASSED', report['passed'], flush=True)


if __name__ == '__main__':
    raise SystemExit(main())
