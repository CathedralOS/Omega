"""Materialize one immutable producer, then observe one bounded Source prefix."""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import json
import os
import platform
import select
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('phase', choices=('materialize', 'observe'))
parser.add_argument('--recipe', type=Path, required=True)
args = parser.parse_args()
staged = args.recipe.resolve().parent
recipe = json.loads(args.recipe.read_text())
root = Path.cwd().resolve()
output = root / 'build/installed-entry-current-resource'
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
assert hasattr(os, 'pidfd_open') and hasattr(signal, 'pidfd_send_signal')
stage_record = read(staged / 'staged-observer.json')
assert stage_record['observer'] == driver
for path, digest in stage_record['files'].items():
    assert sha((staged / path).read_bytes()) == digest
for path, digest in recipe['observer_files'].items():
    assert stage_record['files'][path] == digest
manifest = read(staged / 'installed_entry_current_inputs.json')
assert len(manifest['inputs']) == 358
sys.path[:0] = [str(root / 'tools'), str(root / 'tools/ci')]
import compiler_pilot
from compiler_replay_plan import input_objects
assert compiler_pilot.corpus_gate.host_target() == recipe['target'] == 'linux_x86_64'
assert platform.machine() == 'x86_64'
assert input_objects(root, source) == recipe['compiler_inputs']
positive, negative = compiler_pilot.select_fixtures(recipe['fixture'])
assert positive == (recipe['fixture'],) and negative == ()


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
        actual_profiles=profiles, input_count=358, host=platform.uname()._asdict(), builds_rust=False)
    (output / 'preparation.json').write_text(json.dumps(binding, indent=2) + '\n')
    print(json.dumps(dict(protocol='installed-entry-current-resource-v1', kind='prepared', observer=driver,
        source=source, input_count=358, profiles=profiles, archive_sha256=digest.hexdigest(), builds_rust=False)), flush=True)
    raise SystemExit(0)

producer = compiler_pilot.load_producer(output / 'producer.json')
producer.verify()
assert producer.revision == source
assert producer.runner_profile['sha256'] == recipe['original_producer']['runner_profile']['sha256']
assert producer.cli_profile['sha256'] == recipe['original_producer']['cli_profile']['sha256']
golden = compiler_pilot.observation_golden(output, both=False, target=recipe['target'])
checking = output / 'checking-prefix.json'
command = [sys.executable, 'tools/corpus_gate.py', '--runner', str(producer.runner), '--filter',
    recipe['fixture'], '--json', str(checking), '--golden', str(golden)]
environment = {k: os.environ[k] for k in ('PATH', 'HOME', 'LANG', 'LC_ALL', 'TMPDIR', 'LD_LIBRARY_PATH') if k in os.environ}
environment.update(RUST_MIN_STACK='67108864', OMEGA_LIBRARY_ROOT=str(root / 'source/library'),
    OMEGA_CORPUS_ROOT=str(root), OMEGA_CORPUS_JOBS='1', OMEGA_CORPUS_FIXTURE_SECS='120', OMEGA_CORPUS_SWEEP='1')
assert not any(k in environment for k in ('GH_TOKEN', 'GITHUB_TOKEN', 'OMEGA_PHASE_PROFILE',
    'OMEGA_CORPUS_NATIVE', 'OMEGA_CORPUS_INTERPRETED', 'DEBUGINFOD_URLS'))
TAG = 'installed-entry-current-resource-v1'
seen = {}
rows = []
errors = []
signals = []
process = None
began = None
cleanup_phase = False
cleanup = {}
stop_reason = 'not_started'
runner_pid = None


def emit(kind, **fields):
    row = dict(protocol=TAG, kind=kind, **fields)
    rows.append(row)
    print(json.dumps(row, sort_keys=True), flush=True)


def stat(pid):
    tail = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    return dict(state=tail[0], ppid=int(tail[1]), pgrp=int(tail[2]), session=int(tail[3]),
        utime_ticks=int(tail[11]), stime_ticks=int(tail[12]), threads=int(tail[17]),
        start_ticks=int(tail[19]), virtual_bytes=int(tail[20]), rss_pages=int(tail[21]))


def register(pid, parent=None):
    global runner_pid
    if pid in seen:
        return
    fd = None
    try:
        before = stat(pid)
        if parent is not None and before['ppid'] != parent:
            return
        fd = os.pidfd_open(pid)
        after = stat(pid)
        if after['start_ticks'] != before['start_ticks'] or (parent is not None and after['ppid'] != parent):
            return
        image = Path(os.readlink(f'/proc/{pid}/exe')).resolve()
        # Custody precedes role classification, so even an unexpected invocation
        # remains held for cleanup. Argument bytes are never logged or retained.
        seen[pid] = dict(fd=fd, start_ticks=after['start_ticks'], image=str(image), role='owned_descendant')
        fd = None
        if image == producer.runner.resolve():
            expected = os.fsencode(str(producer.runner.resolve())) + b'\0'
            profile_query = expected + b'--build-profile\0'
            with Path(f'/proc/{pid}/cmdline').open('rb') as stream:
                invocation = stream.read(len(profile_query) + 1)
            assert stat(pid)['start_ticks'] == after['start_ticks']
            assert Path(os.readlink(f'/proc/{pid}/exe')).resolve() == image
            if invocation == profile_query:
                role = 'compiler_profile_query'
            elif invocation == expected:
                role = 'source_no_arguments'
            else:
                raise RuntimeError('Unexpected argument shape for the owned expected runner')
            del invocation
            seen[pid]['role'] = role
            assert sha(Path(f'/proc/{pid}/exe').read_bytes()) == recipe['original_producer']['runner_profile']['sha256']
            if role == 'source_no_arguments':
                assert runner_pid is None, 'More than one owned Source runner'
                runner_pid = pid
        emit('owned_process', pid=pid, parent=parent, executable=str(image), role=seen[pid]['role'], **after)
    except (FileNotFoundError, ProcessLookupError):
        pass
    finally:
        if fd is not None:
            os.close(fd)


def discover():
    pending = list(seen)
    visited = set()
    while pending:
        pid = pending.pop()
        if pid in visited:
            continue
        visited.add(pid)
        try:
            assert stat(pid)['start_ticks'] == seen[pid]['start_ticks']
            for task in Path(f'/proc/{pid}/task').iterdir():
                try:
                    children = (task / 'children').read_text().split()
                except (FileNotFoundError, ProcessLookupError):
                    continue
                for value in children:
                    child = int(value)
                    register(child, pid)
                    if child in seen:
                        pending.append(child)
        except (FileNotFoundError, ProcessLookupError):
            pass


def exited(pid):
    poll = select.poll()
    poll.register(seen[pid]['fd'], select.POLLIN)
    return bool(poll.poll(0))


def snapshot(stream):
    discover()
    counters = []
    rss = None
    for pid, identity in seen.items():
        try:
            current = stat(pid)
            assert current['start_ticks'] == identity['start_ticks']
            counters.append(dict(pid=pid, role=identity['role'], **current))
            if pid == runner_pid:
                rss = current['rss_pages'] * os.sysconf('SC_PAGE_SIZE')
        except (FileNotFoundError, ProcessLookupError):
            counters.append(dict(pid=pid, role=identity['role'], start_ticks=identity['start_ticks'], exited=True))
    if stream:
        emit('owned_resources', elapsed=round(time.monotonic() - began, 3), runner_pid=runner_pid,
            runner_rss_bytes=rss, page_bytes=os.sysconf('SC_PAGE_SIZE'), clock_ticks=os.sysconf('SC_CLK_TCK'),
            processes=counters, limits='Owned processes only; RSS is resident pages, not allocation attribution.')
    return rss


def interrupted(signum, frame):
    row = dict(signal=signum, elapsed=None if began is None else round(time.monotonic() - began, 3))
    signals.append(row)
    if cleanup_phase:
        errors.append(dict(type='CleanupPhaseSignal', **row))
        return
    raise InterruptedError(f'Controller received signal {signum}')


for sig in (signal.SIGTERM, signal.SIGINT):
    signal.signal(sig, interrupted)
try:
    emit('launch', observer=driver, compiler_program_source=source, fixture=recipe['fixture'], command=command,
        prefix_seconds=20, cleanup_deadline_seconds=30, runner_rss_stop_bytes=6 * 1024**3,
        fixture_seconds=120, one_worker=True, phase_profile=False, diagnostic_only=True)
    began = time.monotonic()
    process = subprocess.Popen(command, cwd=root, env=environment, stdin=subprocess.DEVNULL,
        stdout=None, stderr=subprocess.STDOUT, start_new_session=True)
    register(process.pid)
    assert process.pid in seen and stat(process.pid)['pgrp'] == stat(process.pid)['session'] == process.pid
    next_stream = 0
    while time.monotonic() - began < 20:
        elapsed = time.monotonic() - began
        stream = elapsed >= next_stream
        rss = snapshot(stream)
        if stream:
            next_stream = elapsed + 1
        if rss is not None and rss >= 6 * 1024**3:
            emit('rss_early_stop', elapsed=round(time.monotonic() - began, 3), runner_pid=runner_pid,
                observed_rss_bytes=rss, threshold_bytes=6 * 1024**3)
            stop_reason = 'diagnostic_runner_rss_threshold'
            break
        if exited(process.pid):
            stop_reason = 'source_command_exited_early'
            break
        time.sleep(min(.1, max(0, 20 - (time.monotonic() - began))))
    else:
        stop_reason = 'diagnostic_twenty_second_prefix'
    assert runner_pid is not None, 'No authenticated owned runner observed'
except BaseException as error:
    errors.append(dict(type=type(error).__name__, message=str(error)))
    stop_reason = 'observer_or_controller_failure'
finally:
    cleanup_phase = True
    if process is not None:
        # No discovery, reporting or channel operation can skip escalation/reap.
        try:
            discover()
        except BaseException as error:
            errors.append(dict(type=type(error).__name__, message=str(error), phase='cleanup_discover'))
        for sig, deadline in ((signal.SIGTERM, 24), (signal.SIGKILL, 28)):
            try:
                os.killpg(process.pid, sig)
            except ProcessLookupError:
                pass
            except BaseException as error:
                errors.append(dict(type=type(error).__name__, message=str(error), phase='group_signal'))
            for pid, identity in seen.items():
                try:
                    signal.pidfd_send_signal(identity['fd'], sig)
                except ProcessLookupError:
                    pass
                except BaseException as error:
                    errors.append(dict(type=type(error).__name__, message=str(error), phase='held_signal', pid=pid))
            wait_until = min(deadline, time.monotonic() - began + 2)
            while time.monotonic() - began < wait_until:
                try:
                    if all(exited(pid) for pid in seen):
                        break
                except BaseException as error:
                    errors.append(dict(type=type(error).__name__, message=str(error), phase='held_poll'))
                    break
                time.sleep(.05)
            try:
                if all(exited(pid) for pid in seen):
                    break
            except BaseException as error:
                errors.append(dict(type=type(error).__name__, message=str(error), phase='held_poll'))
        remaining = []
        for pid in seen:
            try:
                if not exited(pid):
                    remaining.append(pid)
            except BaseException as error:
                remaining.append(pid)
                errors.append(dict(type=type(error).__name__, message=str(error), phase='final_poll', pid=pid))
        try:
            code = process.wait(timeout=max(.01, 29 - (time.monotonic() - began)))
            cleanup = dict(root_exit=code, remaining_owned_pids=remaining,
                all_held_processes_exited=not remaining, elapsed=round(time.monotonic() - began, 3))
            assert not remaining and cleanup['elapsed'] <= 30
        except BaseException as error:
            errors.append(dict(type=type(error).__name__, message=str(error), phase='root_reap'))
    for identity in seen.values():
        try:
            os.close(identity['fd'])
        except BaseException as error:
            errors.append(dict(type=type(error).__name__, message=str(error), phase='pidfd_close'))
    result = dict(diagnostic_only=True, observer=driver, compiler_program_source=source, fixture=recipe['fixture'],
        stop_reason=stop_reason, cleanup=cleanup, errors=errors, controller_signals=signals, observations=rows,
        checking_report_present=checking.exists(), acceptance='Stopped prefix is no Source verdict or acceptance. All missing18/13/19 host scope and parent gaps remain open.',
        service_limit='External service SIGKILL cannot be recovered by this observer.')
    (output / 'resource-diagnostic.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(protocol=TAG, kind='final', **{k: v for k, v in result.items() if k != 'observations'})), flush=True)
producer.verify()
verify_inputs()
raise SystemExit(1 if errors else 0)
