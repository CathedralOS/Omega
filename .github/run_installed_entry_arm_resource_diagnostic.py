"""Observe one unchanged Source selection; this prefix is not qualification."""
from pathlib import Path
import hashlib
import json
import os
import select
import signal
import subprocess
import sys
import time

R = Path.cwd()
sys.path[:0] = [str(R / 'tools'), str(R / 'tools/ci')]
import corpus_gate
from compiler_pilot import load_producer, observation_golden

recipe = json.loads((R / '.github/installed_entry_arm_resource_recipe.json').read_text())
original = json.loads((R / '.github/installed_entry_arm_replay_recipe.json').read_text())
manifest = json.loads((R / '.github/installed_entry_missing_hosts_inputs.json').read_text())
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
assert head == os.environ['GITHUB_SHA']
assert subprocess.check_output(['git', 'rev-parse', 'HEAD^'], text=True).strip() == recipe['parent']
assert corpus_gate.host_target() == 'linux_arm64' and os.uname().machine == 'aarch64'
assert os.environ['OMEGA_CORPUS_JOBS'] == '1' and os.environ['OMEGA_CORPUS_FIXTURE_SECS'] == '120'
assert not os.environ.get('OMEGA_PHASE_PROFILE')
assert hasattr(os, 'pidfd_open') and hasattr(signal, 'pidfd_send_signal')
assert not subprocess.check_output(['git', 'status', '--porcelain'], text=True).strip()
assert all(hashlib.sha256((R / p).read_bytes()).hexdigest() == h for p, h in manifest['inputs'].items())
assert recipe['source_checks'] == original['source_checks'] and len(recipe['source_checks']) == 18
output = R / 'build/installed-entry-linux_arm64'
producer = load_producer(output / 'producer.json')
producer.verify()
assert producer.revision == recipe['compiler_source'] and producer.consumer_revision == head
assert producer.runner_profile['sha256'] == recipe['runner_sha256']
assert producer.cli_profile['sha256'] == recipe['cli_sha256']
assert producer.runner_profile['profile']['opt_level'] == producer.cli_profile['profile']['opt_level'] == '1'
report = output / 'resource-diagnostic.json'
checking = output / 'checking-diagnostic.json'
golden = observation_golden(output, both=False, target='linux_arm64')
command = [sys.executable, 'tools/corpus_gate.py', '--runner', str(producer.runner),
           '--filter', ','.join(recipe['source_checks']), '--json', str(checking), '--golden', str(golden)]
environment = dict(os.environ, OMEGA_CORPUS_SWEEP='1',
                   OMEGA_CORPUS_CAPTURE_ARTIFACTS=str(output / 'diagnostic-artifacts/checking'))
# The sole child uses its own session. No host process listing, arguments,
# environment, stacks, credentials or kernel/system settings are inspected.
seen = {}
observations = []
process = None
started = None
stop_reason = 'not_started'
cleanup = {}
errors = []

def emit(kind, **values):
    row = dict(protocol='installed-entry-resource-v1', kind=kind, **values)
    print(json.dumps(row, sort_keys=True), flush=True)
    observations.append(row)


def proc_stat(pid):
    text = Path(f'/proc/{pid}/stat').read_text()
    tail = text[text.rfind(')') + 2:].split()
    return dict(state=tail[0], ppid=int(tail[1]), pgrp=int(tail[2]), session=int(tail[3]),
                utime_ticks=int(tail[11]), stime_ticks=int(tail[12]),
                child_utime_ticks=int(tail[13]), child_stime_ticks=int(tail[14]),
                threads=int(tail[17]), start_ticks=int(tail[19]),
                virtual_bytes=int(tail[20]), rss_pages=int(tail[21]))


def register(pid, parent=None):
    if pid in seen:
        return
    fd = None
    try:
        before = proc_stat(pid)
        if parent is not None and before['ppid'] != parent:
            return  # A stale children-list PID grants no process identity.
        fd = os.pidfd_open(pid, 0)
        after = proc_stat(pid)
        if after['start_ticks'] != before['start_ticks'] or (parent is not None and after['ppid'] != parent):
            return
        seen[pid] = dict(pidfd=fd, start_ticks=after['start_ticks'])
        fd = None  # The owned inventory now retains this descriptor.
        emit('owned_process', pid=pid, parent=parent, start_ticks=after['start_ticks'],
             pgrp=after['pgrp'], session=after['session'],
             executable=os.readlink(f'/proc/{pid}/exe'))
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
            stat = proc_stat(pid)
            if stat['start_ticks'] != seen[pid]['start_ticks']:
                raise RuntimeError('Held process identity changed')
            # Linux records children per thread. Read only the tasks belonging
            # to already authenticated descendants, never /proc's global roster.
            for task in Path(f'/proc/{pid}/task').iterdir():
                try:
                    children = (task / 'children').read_text().split()
                except (FileNotFoundError, ProcessLookupError):
                    continue
                for value in children:
                    child = int(value)
                    register(child, parent=pid)
                    if child in seen:
                        pending.append(child)
        except (FileNotFoundError, ProcessLookupError):
            pass


def snapshot():
    discover()
    rows = []
    for pid, identity in seen.items():
        try:
            stat = proc_stat(pid)
            assert stat['start_ticks'] == identity['start_ticks']
            status = {}
            wanted = {'VmPeak', 'VmSize', 'VmHWM', 'VmRSS', 'RssAnon', 'RssFile', 'RssShmem', 'Threads'}
            for line in Path(f'/proc/{pid}/status').read_text().splitlines():
                key, _, value = line.partition(':')
                if key in wanted:
                    status[key] = value.strip()
            rows.append(dict(pid=pid, **stat, memory=status))
        except (FileNotFoundError, ProcessLookupError):
            rows.append(dict(pid=pid, exited=True, start_ticks=identity['start_ticks']))
    emit('owned_resources', elapsed=round(time.monotonic() - started, 3),
         clock_ticks=os.sysconf('SC_CLK_TCK'), page_bytes=os.sysconf('SC_PAGE_SIZE'), processes=rows,
         limits='Per-process counters only; shared pages may occur in several RSS counters.')


def exited(pid):
    poll = select.poll()
    poll.register(seen[pid]['pidfd'], select.POLLIN)
    return bool(poll.poll(0))


def stop(signum, frame):
    emit('controller_signal', signal=signum, elapsed=round(time.monotonic() - started, 3) if started else None)
    raise InterruptedError(f'Controller received signal {signum}')


for signum in (signal.SIGTERM, signal.SIGINT):
    signal.signal(signum, stop)
try:
    emit('launch', observer=head, compiler_source=producer.revision, original_run=recipe['origin_run'],
         command=command, target='linux_arm64', source_count=18,
         fixture_seconds=120, prefix_seconds=105, cleanup_deadline_seconds=120,
         flags='one worker; phase profile off; no engines, samples, compiler build or Rust tests')
    started = time.monotonic()
    process = subprocess.Popen(command, cwd=R, env=environment, stdin=subprocess.DEVNULL,
                               stdout=None, stderr=subprocess.STDOUT, start_new_session=True)
    register(process.pid)
    assert process.pid in seen
    root = proc_stat(process.pid)
    assert root['pgrp'] == root['session'] == process.pid
    while time.monotonic() - started < 105:
        snapshot()
        if exited(process.pid):
            stop_reason = 'source_command_completed_early'
            break
        time.sleep(min(5, max(0, 105 - (time.monotonic() - started))))
    else:
        stop_reason = 'diagnostic_prefix_budget'
except BaseException as error:
    errors.append(dict(type=type(error).__name__, message=str(error)))
    stop_reason = 'observer_or_controller_failure'
    emit('observer_error', error=errors[-1])
finally:
    if process is not None:
        try:
            discover()
            # The root remains unreaped until after group cleanup. Its held
            # identity therefore reserves this session/group number throughout.
            for signum, until in ((signal.SIGTERM, 110), (signal.SIGKILL, 116)):
                try:
                    os.killpg(process.pid, signum)
                except ProcessLookupError:
                    pass
                for pid, identity in seen.items():
                    if not exited(pid):
                        try:
                            signal.pidfd_send_signal(identity['pidfd'], signum)
                        except ProcessLookupError:
                            pass
                emit('cleanup_signal', signal=signum, elapsed=round(time.monotonic() - started, 3), owned_pids=list(seen))
                while time.monotonic() - started < until and not all(exited(pid) for pid in seen):
                    time.sleep(.1)
                if all(exited(pid) for pid in seen):
                    break
            remaining = [pid for pid in seen if not exited(pid)]
            code = process.wait(timeout=max(.01, 118 - (time.monotonic() - started)))
            cleanup = dict(root_exit=code, remaining_owned_pids=remaining,
                           elapsed=round(time.monotonic() - started, 3), all_held_processes_exited=not remaining)
            assert not remaining and cleanup['elapsed'] <= 120
            emit('cleanup_result', **cleanup)
        except BaseException as error:
            errors.append(dict(type=type(error).__name__, message=str(error)))
            emit('cleanup_error', error=errors[-1])
    for identity in seen.values():
        os.close(identity['pidfd'])
    result = dict(diagnostic_only=True, observer=head, compiler_source=producer.revision,
                  stop_reason=stop_reason, cleanup=cleanup, errors=errors, observations=observations,
                  acceptance='All original18 Source/13 pairs/19 samples remain owed. A stopped prefix establishes no qualification.',
                  checking_report_present=checking.exists())
    report.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(protocol='installed-entry-resource-v1', kind='final', **{k:v for k,v in result.items() if k!='observations'})), flush=True)
producer.verify()
raise SystemExit(1 if errors else 0)
