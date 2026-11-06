"""Normal-user GDB/MI child launch; bounded frames and owned resources only."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import select
import selectors
import signal
import subprocess
import sys
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--recipe', type=Path, required=True)
args = parser.parse_args()
staged = args.recipe.resolve().parent
recipe = json.loads(args.recipe.read_text())
root = Path.cwd().resolve()
output = root / 'build/installed-entry-current-callers'
output.mkdir(parents=True, exist_ok=False)
sha = lambda b: hashlib.sha256(b).hexdigest()
read = lambda p: json.loads(p.read_text())
git = lambda *a: subprocess.check_output(['git', *a], text=True).strip()
driver = os.environ['GITHUB_SHA']
source = recipe['source']
assert os.geteuid() != 0 and git('rev-parse', 'HEAD') == source and not git('status', '--porcelain')
assert git('rev-parse', 'HEAD^{tree}') == recipe['tree']
assert os.environ['OMEGA_CORPUS_JOBS'] == '1' and os.environ['OMEGA_CORPUS_FIXTURE_SECS'] == '120'
assert not os.environ.get('OMEGA_PHASE_PROFILE')
stage_record = read(staged / 'staged-observer.json')
assert stage_record['observer'] == driver
for name, digest in stage_record['files'].items():
    assert sha((staged / name).read_bytes()) == digest
for name, digest in recipe['observer_files'].items():
    assert stage_record['files'][name] == digest
inputs = read(staged / 'installed_entry_current_inputs.json')['inputs']
assert len(inputs) == 358
sys.path[:0] = [str(root / 'tools'), str(root / 'tools/ci')]
from compiler_pilot import load_producer
producer_path = root / 'build/installed-entry-current-resource/producer.json'
os.environ['GITHUB_SHA'] = source
producer = load_producer(producer_path)
producer.verify()
assert producer.revision == source and producer.runner_profile['sha256'] == recipe['runner_sha256']
setup = read(root / 'build/installed-entry-current-gdb-setup/summary.json')
assert setup['passed'] and setup['uid'] == os.geteuid() != 0
GDB = Path(setup['gdb']['path'])
assert sha(GDB.read_bytes()) == setup['gdb']['sha256']
assert hasattr(os, 'pidfd_open') and hasattr(signal, 'pidfd_send_signal')
probe = os.pidfd_open(os.getpid())
os.close(probe)
for path, digest in inputs.items():
    assert sha((root / path).read_bytes()) == digest
cache = output / 'source-cache'
cache.mkdir()
environment = {k: os.environ[k] for k in ('PATH', 'HOME', 'LANG', 'LC_ALL', 'TMPDIR', 'LD_LIBRARY_PATH') if k in os.environ}
environment.update(RUST_MIN_STACK='67108864', OMEGA_LIBRARY_ROOT=str(root / 'source/library'),
    XDG_CACHE_HOME=str(cache), OMEGA_CORPUS_ROOT=str(root), OMEGA_CORPUS_FIXTURE_FILTER=recipe['fixture'],
    OMEGA_CORPUS_FIXTURE_SECS='120', OMEGA_CORPUS_JOBS='1', OMEGA_CORPUS_RECORDS=str(output / 'source.records.txt'))
assert not any(k in environment for k in ('GH_TOKEN', 'GITHUB_TOKEN', 'OMEGA_PHASE_PROFILE',
    'DEBUGINFOD_URLS', 'OMEGA_CORPUS_NATIVE', 'OMEGA_CORPUS_INTERPRETED', 'OMEGA_CORPUS_CHECKED_INTERPRETER'))
command = [str(GDB), '--quiet', '-nx', '-nh', '-iex', 'set auto-load off', '--interpreter=mi3']
TAG = 'installed-entry-current-gdb-v1'
seen, rawfiles, buffers, results = {}, {}, {}, {}
rows, errors, signals, stops, snapshots = [], [], [], [], []
streams = bytearray()
selector = selectors.DefaultSelector()
process = None
runner_pid = None
outer_began = None
source_began = None
cleanup_phase = False
cleanup = {}
token = 0
allow_stop = False
stop_reason = 'not_started'
next_resource = 0


class DiagnosticStop(Exception):
    pass


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
    if pid in seen:
        return
    fd = None
    try:
        before = stat(pid)
        if parent is not None and before['ppid'] != parent:
            return
        fd = os.pidfd_open(pid)
        after = stat(pid)
        if before['start_ticks'] != after['start_ticks'] or (parent is not None and after['ppid'] != parent):
            return
        seen[pid] = dict(fd=fd, start_ticks=after['start_ticks'])
        fd = None
        emit('held_process', pid=pid, parent=parent, **after)
    except (FileNotFoundError, ProcessLookupError):
        pass
    finally:
        if fd is not None:
            os.close(fd)


def discover():
    pending, visited = list(seen), set()
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
                for child in map(int, children):
                    register(child, pid)
                    if child in seen:
                        pending.append(child)
        except (FileNotFoundError, ProcessLookupError):
            pass


def exited(pid):
    poll = select.poll()
    poll.register(seen[pid]['fd'], select.POLLIN)
    return bool(poll.poll(0))


def resources(stream):
    discover()
    counters, rss = [], None
    for pid, identity in seen.items():
        try:
            current = stat(pid)
            assert current['start_ticks'] == identity['start_ticks']
            counters.append(dict(pid=pid, **current))
            if pid == runner_pid:
                rss = current['rss_pages'] * os.sysconf('SC_PAGE_SIZE')
        except (FileNotFoundError, ProcessLookupError):
            counters.append(dict(pid=pid, start_ticks=identity['start_ticks'], exited=True))
    if stream:
        emit('owned_resources', source_seconds=None if source_began is None else round(time.monotonic() - source_began, 3),
            outer_seconds=round(time.monotonic() - outer_began, 3), runner_pid=runner_pid, runner_rss_bytes=rss,
            page_bytes=os.sysconf('SC_PAGE_SIZE'), clock_ticks=os.sysconf('SC_CLK_TCK'), processes=counters)
    if rss is not None and rss >= 6 * 1024**3:
        emit('rss_early_stop', source_seconds=round(time.monotonic() - source_began, 3),
            runner_pid=runner_pid, observed_rss_bytes=rss, threshold_bytes=6 * 1024**3)
        raise DiagnosticStop('diagnostic_runner_rss_threshold')


def bind_inferior(pid):
    global runner_pid
    assert runner_pid is None, 'Unexpected second inferior'
    register(pid, process.pid)
    assert pid in seen
    assert stat(pid)['ppid'] == process.pid
    image = Path(os.readlink(f'/proc/{pid}/exe')).resolve()
    assert image == producer.runner.resolve()
    expected = os.fsencode(str(producer.runner.resolve())) + b'\0'
    with Path(f'/proc/{pid}/cmdline').open('rb') as stream:
        invocation = stream.read(len(expected) + 1)
    assert invocation == expected, 'Owned inferior is not the exact no-argument Source invocation'
    del invocation
    assert sha(Path(f'/proc/{pid}/exe').read_bytes()) == recipe['runner_sha256']
    assert stat(pid)['start_ticks'] == seen[pid]['start_ticks']
    runner_pid = pid
    emit('source_identity', pid=pid, start_ticks=seen[pid]['start_ticks'], parent_pid=process.pid,
        image=str(image), sha256=recipe['runner_sha256'], role='source_no_arguments')


def pump(timeout):
    global next_resource
    now = time.monotonic()
    if now - outer_began >= 22 or (source_began is not None and now - source_began >= 20):
        raise DiagnosticStop('diagnostic_work_budget')
    resources(now >= next_resource)
    if now >= next_resource:
        next_resource = now + 1
    for key, _ in selector.select(min(timeout, .1)):
        data = os.read(key.fileobj.fileno(), 65536)
        if not data:
            selector.unregister(key.fileobj)
            continue
        channel = key.data
        rawfiles[channel].write(data)
        rawfiles[channel].flush()
        assert rawfiles[channel].tell() <= 8 * 1024 * 1024
        streams.extend(data)
        assert len(streams) <= 16 * 1024 * 1024
        sys.stdout.buffer.write(data)
        sys.stdout.buffer.flush()
        buffers[channel] += data
        while b'\n' in buffers[channel]:
            line, buffers[channel] = buffers[channel].split(b'\n', 1)
            text = line.decode(errors='replace')
            if channel != 'stdout':
                continue
            match = re.match(r'^(\d+)\^(done|running|error|exit)(?:,|$)', text)
            if match:
                results[int(match[1])] = (match[2], text)
            if re.match(r'^\d*\*stopped(?:,|$)', text):
                stops.append(dict(source_seconds=None if source_began is None else round(time.monotonic() - source_began, 3), raw=text))
                assert allow_stop, 'Uncommanded inferior stop; no resume allowed'
            match = re.match(r'^=thread-group-started,id="i1",pid="(\d+)"$', text)
            if match:
                bind_inferior(int(match[1]))
            assert not re.match(r'^=thread-group-started,id="(?!i1")', text), 'Unexpected inferior group'


def send(text, expected='done', seconds=3):
    global token
    token += 1
    current = token
    process.stdin.write((str(current) + text + '\n').encode())
    process.stdin.flush()
    until = time.monotonic() + seconds
    while current not in results:
        assert not exited(process.pid) and time.monotonic() < until, 'Bounded GDB command failed to acknowledge'
        pump(min(.1, max(0, until - time.monotonic())))
    result = results.pop(current)
    assert result[0] == expected, result
    return result[1]


def interrupted(signum, frame):
    row = dict(signal=signum, outer_seconds=None if outer_began is None else round(time.monotonic() - outer_began, 3))
    signals.append(row)
    if cleanup_phase:
        errors.append(dict(type='CleanupPhaseSignal', **row))
        return
    raise InterruptedError(f'Controller received signal {signum}')


for sig in (signal.SIGTERM, signal.SIGINT):
    signal.signal(sig, interrupted)
try:
    emit('launch', observer=driver, compiler_program_source=source, fixture=recipe['fixture'],
        debugger=setup['gdb'], snapshot_seconds=[12, 14, 16], work_seconds=20, outer_stop_seconds=22,
        cleanup_deadline_seconds=30, runner_rss_stop_bytes=6 * 1024**3,
        qualification=False, meaning='Active frames and counters only; no allocation caller, OOM or dominant-cost inference.')
    outer_began = time.monotonic()
    process = subprocess.Popen(command, cwd=root, env=environment, stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    register(process.pid)
    assert process.pid in seen and stat(process.pid)['pgrp'] == stat(process.pid)['session'] == process.pid
    for channel, pipe in [('stdout', process.stdout), ('stderr', process.stderr)]:
        os.set_blocking(pipe.fileno(), False)
        selector.register(pipe, selectors.EVENT_READ, channel)
        rawfiles[channel] = (output / ('gdb.' + channel)).open('wb')
        buffers[channel] = b''
    for setting in ['mi-async on', 'non-stop off', 'startup-with-shell off', 'disable-randomization off',
            'debuginfod enabled off', 'pagination off', 'confirm off', 'may-call-functions off',
            'print frame-arguments none', 'print entry-values no', 'print thread-events off']:
        send('-gdb-set ' + setting)
    send('-file-exec-and-symbols ' + json.dumps(str(producer.runner)), seconds=5)
    send('-exec-arguments')
    source_began = time.monotonic()
    send('-exec-run', expected='running')
    root_deadline = min(source_began + 5, outer_began + 20)
    while runner_pid is None or b'corpus_runner: fixture-root-v1 ' not in streams:
        assert time.monotonic() < root_deadline, 'No exact Source/root acknowledgement'
        pump(.05)
    roots = re.findall(rb'corpus_runner: fixture-root-v1 ([0-9a-f]+)', streams)
    assert len(roots) == 1 and bytes.fromhex(roots[0].decode()).decode() == str(root)
    emit('source_root', root=str(root), pid=runner_pid)
    for scheduled in (12, 14, 16):
        while time.monotonic() - source_began < scheduled:
            assert not stops and not exited(process.pid) and not exited(runner_pid)
            pump(min(.1, max(0, scheduled - (time.monotonic() - source_began))))
        allow_stop = True
        before = len(stops)
        send('-exec-interrupt --all')
        until = time.monotonic() + 2
        while len(stops) == before:
            assert time.monotonic() < until and not exited(process.pid)
            pump(.05)
        assert len(stops) == before + 1 and 'reason="signal-received"' in stops[-1]['raw'] and 'signal-name="SIGINT"' in stops[-1]['raw']
        start = rawfiles['stdout'].tell()
        send('-interpreter-exec console ' + json.dumps(
            'thread apply all backtrace -frame-arguments none -entry-values no -no-filters 32'), seconds=3)
        end = rawfiles['stdout'].tell()
        row = dict(scheduled_source_seconds=scheduled, observed_source_seconds=round(time.monotonic() - source_began, 3),
            stop=stops[-1], stdout_start=start, stdout_end=end)
        snapshots.append(row)
        emit('snapshot', **row)
        if scheduled != 16:
            stops.clear()
            allow_stop = False
            send('-exec-continue', expected='running')
    stop_reason = 'three_bounded_snapshots_complete'
except DiagnosticStop as stop:
    stop_reason = str(stop)
except BaseException as error:
    errors.append(dict(type=type(error).__name__, message=str(error)))
    stop_reason = 'observer_or_debugger_failure'
finally:
    cleanup_phase = True
    if process is not None:
        # Every cleanup action is independent. Neither pumping nor reporting is
        # attempted before escalation/reap, and repeated signals cannot unwind.
        try:
            discover()
        except BaseException as error:
            errors.append(dict(type=type(error).__name__, message=str(error), phase='cleanup_discover'))
        for sig, deadline in ((signal.SIGTERM, 26), (signal.SIGKILL, 28)):
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
            wait_until = min(deadline, time.monotonic() - outer_began + 2)
            while time.monotonic() - outer_began < wait_until:
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
            code = process.wait(timeout=max(.01, 29 - (time.monotonic() - outer_began)))
            cleanup = dict(debugger_exit=code, remaining_owned_pids=remaining,
                all_held_processes_exited=not remaining, outer_seconds=round(time.monotonic() - outer_began, 3),
                source_seconds=None if source_began is None else round(time.monotonic() - source_began, 3))
            assert not remaining and cleanup['outer_seconds'] <= 30
        except BaseException as error:
            errors.append(dict(type=type(error).__name__, message=str(error), phase='debugger_reap'))
    for identity in seen.values():
        try:
            os.close(identity['fd'])
        except BaseException as error:
            errors.append(dict(type=type(error).__name__, message=str(error), phase='pidfd_close'))
    for stream in rawfiles.values():
        try:
            stream.close()
        except BaseException as error:
            errors.append(dict(type=type(error).__name__, message=str(error), phase='channel_close'))
    selector.close()
    result = dict(diagnostic_only=True, qualification=False, observer=driver, compiler_program_source=source,
        fixture=recipe['fixture'], snapshots=snapshots, complete_three_snapshots=len(snapshots) == 3,
        stop_reason=stop_reason, errors=errors, cleanup=cleanup, controller_signals=signals, observations=rows,
        setup=setup, producer_record=producer.record(), acceptance='No Source or host verdict from this deliberately stopped prefix. All original host and parent acceptance remains open.',
        service_limit='An external service SIGKILL cannot be recovered by the observer.')
    (output / 'observation.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(protocol=TAG, kind='final', **{k: v for k, v in result.items() if k not in ('observations', 'setup', 'producer_record')})), flush=True)
producer.verify()
assert sha(GDB.read_bytes()) == setup['gdb']['sha256']
for path, digest in inputs.items():
    assert sha((root / path).read_bytes()) == digest
raise SystemExit(1 if errors else 0)
