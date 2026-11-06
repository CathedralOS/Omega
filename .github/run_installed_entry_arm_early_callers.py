"""Existing-tool probe and owned LLDB watchdog; no build or qualification."""
from pathlib import Path
import hashlib
import json
import os
import select
import selectors
import signal
import subprocess
import sys
import time

R = Path.cwd()
sys.path[:0] = [str(R / 'tools'), str(R / 'tools/ci')]
from compiler_pilot import load_producer
import corpus_gate

TAG = 'installed-entry-early-callers-v1'
TOOL = Path('/usr/share/swift/usr/bin/lldb')
setup_path = R / 'build/installed-entry-arm-debugger-tool.json'
recipe = json.loads((R / '.github/installed_entry_arm_early_callers_recipe.json').read_text())
phase = sys.argv[1]
assert phase in ('tools', 'observe')
clean_environment = {k: os.environ[k] for k in ('PATH', 'HOME', 'LANG', 'LC_ALL', 'TMPDIR', 'LD_LIBRARY_PATH') if k in os.environ}
clean_environment['PYTHONDONTWRITEBYTECODE'] = '1'
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
assert head == os.environ['GITHUB_SHA']
assert subprocess.check_output(['git', 'rev-parse', 'HEAD^'], text=True).strip() == recipe['parent']
assert corpus_gate.host_target() == 'linux_arm64' and os.uname().machine == 'aarch64'
assert not subprocess.check_output(['git', 'status', '--porcelain'], text=True).strip()
assert os.environ['OMEGA_CORPUS_JOBS'] == '1' and os.environ['OMEGA_CORPUS_FIXTURE_SECS'] == '120'
assert not os.environ.get('OMEGA_PHASE_PROFILE')
assert hasattr(os, 'pidfd_open') and hasattr(signal, 'pidfd_send_signal')
if phase == 'tools':
    # No install/fallback. Missing normal tool support fails before acquisition
    # or any compiler workload, with the exact tool result retained.
    setup_path.parent.mkdir(exist_ok=True)
    setup = dict(observer=head, tool=str(TOOL), status='pending', commands=[])
    try:
        actual = TOOL.resolve(strict=True)
        stat = actual.stat()
        assert actual.is_file() and os.access(actual, os.X_OK) and stat.st_uid == 0 and not stat.st_mode & 0o022
        setup.update(resolved_tool=str(actual), sha256=sha(actual))
        commands = [[str(actual), '--no-lldbinit', '--version'],
                    [str(actual), '--no-lldbinit', '--batch', '-o',
                     'script import lldb; assert hasattr(lldb.SBProcess, "Stop") and hasattr(lldb.SBTarget, "Launch"); print("installed-entry-lldb-api-v1")']]
        for command in commands:
            p = subprocess.run(command, cwd=R, env=clean_environment, stdin=subprocess.DEVNULL,
                               capture_output=True, text=True, timeout=10)
            setup['commands'].append(dict(command=command, exit=p.returncode, stdout=p.stdout, stderr=p.stderr))
            assert p.returncode == 0
        assert 'installed-entry-lldb-api-v1' in setup['commands'][-1]['stdout']
        setup['status'] = 'existing_tool_probe_passed'
    except BaseException as error:
        setup.update(status='tool_probe_failed', error=dict(type=type(error).__name__, message=str(error)))
        raise
    finally:
        setup_path.write_text(json.dumps(setup, indent=2) + '\n')
        print(json.dumps(dict(protocol=TAG, kind='tool_probe', **setup)), flush=True)
    raise SystemExit(0)

setup = json.loads(setup_path.read_text())
assert setup['status'] == 'existing_tool_probe_passed' and setup['observer'] == head
lldb_path = Path(setup['resolved_tool'])
assert sha(lldb_path) == setup['sha256']
output = R / 'build/installed-entry-linux_arm64'
producer = load_producer(output / 'producer.json')
producer.verify()
assert producer.revision == recipe['compiler_source'] and producer.consumer_revision == head
assert producer.runner_profile['sha256'] == recipe['runner_sha256']
manifest = json.loads((R / '.github/installed_entry_missing_hosts_inputs.json').read_text())
assert all(hashlib.sha256((R / p).read_bytes()).hexdigest() == h for p, h in manifest['inputs'].items())
environment = dict(clean_environment, RUST_MIN_STACK='67108864', OMEGA_LIBRARY_ROOT=str(R / 'source/library'),
                   OMEGA_CORPUS_ROOT=str(R), OMEGA_CORPUS_FIXTURE_FILTER=recipe['fixture'],
                   OMEGA_CORPUS_FIXTURE_SECS='120', OMEGA_CORPUS_JOBS='1',
                   OMEGA_CORPUS_TIMINGS=str(R / 'tests/omega/corpus_outcomes.txt'),
                   OMEGA_CORPUS_RECORDS=str(output / 'early-source.records.txt'))
assert not any(k in environment for k in ('GH_TOKEN', 'GITHUB_TOKEN', 'OMEGA_PHASE_PROFILE', 'DEBUGINFOD_URLS', 'OMEGA_CORPUS_NATIVE', 'OMEGA_CORPUS_INTERPRETED'))
config = dict(observer=head, recipe=recipe, runner=str(producer.runner),
              inferior_environment=[k + '=' + v for k, v in environment.items()],
              inferior_stdout=str(output / 'early-source.stdout'), inferior_stderr=str(output / 'early-source.stderr'))
(output / 'early-callers-runtime.json').write_text(json.dumps(config, indent=2) + '\n')
module = (R / '.github/installed_entry_arm_early_lldb.py').resolve()
command = [str(lldb_path), '--no-lldbinit', '--batch', '-o', 'command script import ' + str(module),
           '-o', 'script installed_entry_arm_early_lldb.run(lldb.debugger)']
seen = {}
protocol = []
rawfiles = {}
selector = selectors.DefaultSelector()
process = None
began = None
errors = []
cleanup = {}


def emit(kind, **fields):
    print(json.dumps(dict(protocol=TAG, kind=kind, **fields)), flush=True)


def stat(pid):
    row = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    return dict(start_ticks=int(row[19]), ppid=int(row[1]), pgrp=int(row[2]), session=int(row[3]),
                utime_ticks=int(row[11]), stime_ticks=int(row[12]), rss_pages=int(row[21]))


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
        seen[pid] = dict(fd=fd, **after)
        fd = None
        emit('held_process', pid=pid, parent=parent, **after)
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


def pump(timeout):
    for key, _ in selector.select(timeout):
        data = os.read(key.fileobj.fileno(), 65536)
        if not data:
            selector.unregister(key.fileobj)
            continue
        stream = key.data
        rawfiles[stream].write(data)
        rawfiles[stream].flush()
        assert rawfiles[stream].tell() <= 8 * 1024 * 1024
        # LLDB formats are fixed to omit arguments/values; preserve channels
        # continuously even if the service later skips its final upload.
        sys.stdout.buffer.write(data)
        sys.stdout.buffer.flush()
        buffers[stream] += data
        while b'\n' in buffers[stream]:
            line, buffers[stream] = buffers[stream].split(b'\n', 1)
            text = line.decode(errors='replace')
            if text.startswith('{'):
                try:
                    row = json.loads(text)
                except ValueError:
                    continue
                if row.get('protocol') == TAG:
                    protocol.append(row)
                    if row['kind'] == 'inferior_identity':
                        pid = row['pid']
                        discover()
                        assert pid in seen and seen[pid]['start_ticks'] == row['start_ticks']
                        assert Path(os.readlink(f'/proc/{pid}/exe')).resolve() == producer.runner.resolve()
                        assert sha(Path(f'/proc/{pid}/exe')) == recipe['runner_sha256']


def interrupted(signum, frame):
    emit('controller_signal', signal=signum, elapsed=None if began is None else round(time.monotonic() - began, 3))
    raise InterruptedError(f'Controller received signal {signum}')


for sig in (signal.SIGINT, signal.SIGTERM):
    signal.signal(sig, interrupted)
try:
    emit('outer_launch', observer=head, compiler_source=producer.revision, fixture=recipe['fixture'],
         debugger_sha256=setup['sha256'], runner_sha256=recipe['runner_sha256'],
         snapshots=[15,25,35], watchdog_seconds=60, inherited_credentials=False, diagnostic_only=True)
    began = time.monotonic()
    process = subprocess.Popen(command, cwd=R, env=clean_environment, stdin=subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    register(process.pid)
    assert process.pid in seen and seen[process.pid]['session'] == seen[process.pid]['pgrp'] == process.pid
    buffers = {}
    for stream, pipe in [('stdout',process.stdout),('stderr',process.stderr)]:
        os.set_blocking(pipe.fileno(), False)
        selector.register(pipe, selectors.EVENT_READ, stream)
        rawfiles[stream] = (output / ('early-lldb.' + stream)).open('wb')
        buffers[stream] = b''
    next_resource = 0
    while time.monotonic() - began < 50:
        pump(.1)
        discover()
        elapsed = time.monotonic() - began
        if elapsed >= next_resource:
            counters = []
            for pid, identity in seen.items():
                try:
                    value = stat(pid)
                    assert value['start_ticks'] == identity['start_ticks']
                    counters.append(dict(pid=pid, **value))
                except (FileNotFoundError, ProcessLookupError):
                    pass
            emit('outer_resources', elapsed=round(elapsed,3), processes=counters,
                 clock_ticks=os.sysconf('SC_CLK_TCK'), page_bytes=os.sysconf('SC_PAGE_SIZE'))
            next_resource = elapsed + 5
        if exited(process.pid):
            pump(0)
            break
    finals = [r for r in protocol if r['kind'] == 'inner_final']
    snapshots = [r for r in protocol if r['kind'] == 'snapshot']
    assert len(finals) == 1 and not finals[0]['failed'], 'Missing/failed bounded LLDB result'
    assert [r['scheduled'] for r in snapshots] == [15,25,35]
except BaseException as error:
    errors.append(dict(type=type(error).__name__, message=str(error)))
    emit('outer_error', error=errors[-1])
finally:
    if process is not None:
        try:
            discover()
            # Do not reap the debugger leader before closing its own group.
            for sig, until in ((signal.SIGTERM,55),(signal.SIGKILL,58)):
                try:
                    os.killpg(process.pid,sig)
                except ProcessLookupError:
                    pass
                for pid, identity in seen.items():
                    if not exited(pid):
                        try:
                            signal.pidfd_send_signal(identity['fd'],sig)
                        except ProcessLookupError:
                            pass
                while time.monotonic()-began < until and not all(exited(pid) for pid in seen):
                    pump(.05)
                if all(exited(pid) for pid in seen):
                    break
            remaining = [pid for pid in seen if not exited(pid)]
            code = process.wait(timeout=max(.01,59-(time.monotonic()-began)))
            cleanup = dict(debugger_exit=code,remaining_owned_pids=remaining,elapsed=round(time.monotonic()-began,3))
            assert not remaining and cleanup['elapsed'] <= 60
            emit('outer_cleanup',**cleanup)
        except BaseException as error:
            errors.append(dict(type=type(error).__name__,message=str(error)))
            emit('outer_cleanup_error',error=errors[-1])
    for identity in seen.values():
        os.close(identity['fd'])
    for file in rawfiles.values():
        file.close()
    result = dict(diagnostic_only=True,observer=head,compiler_source=producer.revision,
                  protocol_rows=protocol,cleanup=cleanup,errors=errors,
                  limits='No qualification, allocation-stack, OOM or dominant-cost inference. Original18/13/19 remains owed.')
    print(json.dumps(dict(protocol=TAG,kind='outer_final',**{k:v for k,v in result.items() if k!='protocol_rows'})),flush=True)
    (output/'early-callers.json').write_text(json.dumps(result,indent=2)+'\n')
producer.verify()
raise SystemExit(1 if errors else 0)
