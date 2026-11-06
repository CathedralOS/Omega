"""Run both fixed control groups and retain their actual engine/artifact results.

This is an unshipped qualification consumer. It never edits the frozen inputs,
changes a rejection oracle, or treats an unpinned rejection as acceptance.
"""

import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import subprocess
import sys
import time

sys.path.insert(0, str(Path.cwd() / 'tools'))
from compiler_profile import require_optimized

BASE = Path('build/supplied-output-loan')
ROSTERS = (
    (60, BASE / 'candidate81e-current60-recovered-roster.json',
     '58231da71d2bd92106bdc5902fda2a87c5affbd1ed4b71a92e4b8858dec5c92d'),
    (38, BASE / 'pins-provisioned-scheduler-current38-v4/manifest.json',
     'c35505924030b04ec34901b29fa3267a2552d663e62c88b08198fec69159da65'),
)
INPUT_PATHS = ('omega-rust', 'source/library', 'source/psi', 'source/omega',
               'bootstrap', 'tools/benchmark', 'Cargo.toml', 'Cargo.lock',
               'rust-toolchain.toml', '.cargo')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()



@contextlib.contextmanager
def owned_process_tree(process):
    """Close the whole command's lifetime, including exceptional exits."""
    if os.name != 'nt':
        try:
            yield
        finally:
            # The direct parent may already have exited while descendants
            # still hold this dedicated session's process group.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            finally:
                process.wait(timeout=10)
        return

    import ctypes
    from ctypes import wintypes

    class ThreadEntry(ctypes.Structure):
        _fields_ = [('dwSize', wintypes.DWORD), ('cntUsage', wintypes.DWORD),
                    ('th32ThreadID', wintypes.DWORD), ('th32OwnerProcessID', wintypes.DWORD),
                    ('tpBasePri', wintypes.LONG), ('tpDeltaPri', wintypes.LONG),
                    ('dwFlags', wintypes.DWORD)]

    class BasicLimits(ctypes.Structure):
        _fields_ = [('PerProcessUserTimeLimit', ctypes.c_int64),
                    ('PerJobUserTimeLimit', ctypes.c_int64),
                    ('LimitFlags', wintypes.DWORD),
                    ('MinimumWorkingSetSize', ctypes.c_size_t),
                    ('MaximumWorkingSetSize', ctypes.c_size_t),
                    ('ActiveProcessLimit', wintypes.DWORD),
                    ('Affinity', ctypes.c_size_t),
                    ('PriorityClass', wintypes.DWORD),
                    ('SchedulingClass', wintypes.DWORD)]

    class IoCounters(ctypes.Structure):
        _fields_ = [(name, ctypes.c_uint64) for name in (
            'ReadOperationCount', 'WriteOperationCount', 'OtherOperationCount',
            'ReadTransferCount', 'WriteTransferCount', 'OtherTransferCount')]

    class ExtendedLimits(ctypes.Structure):
        _fields_ = [('BasicLimitInformation', BasicLimits), ('IoInfo', IoCounters),
                    ('ProcessMemoryLimit', ctypes.c_size_t), ('JobMemoryLimit', ctypes.c_size_t),
                    ('PeakProcessMemoryUsed', ctypes.c_size_t), ('PeakJobMemoryUsed', ctypes.c_size_t)]

    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
    kernel.CreateJobObjectW.restype = wintypes.HANDLE
    kernel.SetInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]
    kernel.SetInformationJobObject.restype = wintypes.BOOL
    kernel.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
    kernel.AssignProcessToJobObject.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel.Thread32First.argtypes = [wintypes.HANDLE, ctypes.POINTER(ThreadEntry)]
    kernel.Thread32First.restype = wintypes.BOOL
    kernel.Thread32Next.argtypes = [wintypes.HANDLE, ctypes.POINTER(ThreadEntry)]
    kernel.Thread32Next.restype = wintypes.BOOL
    kernel.OpenThread.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenThread.restype = wintypes.HANDLE
    kernel.ResumeThread.argtypes = [wintypes.HANDLE]
    kernel.ResumeThread.restype = wintypes.DWORD
    job = kernel.CreateJobObjectW(None, None)
    assigned = False
    try:
        if not job:
            raise ctypes.WinError(ctypes.get_last_error())
        limits = ExtendedLimits()
        limits.BasicLimitInformation.LimitFlags = 0x2000  # KILL_ON_JOB_CLOSE
        if not kernel.SetInformationJobObject(job, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
            raise ctypes.WinError(ctypes.get_last_error())
        if not kernel.AssignProcessToJobObject(job, wintypes.HANDLE(int(process._handle))):
            raise ctypes.WinError(ctypes.get_last_error())
        assigned = True
        # CREATE_SUSPENDED prevents application execution before Job custody.
        # Python closes CreateProcess's thread handle, so reopen the sole
        # initial thread through the documented Toolhelp/OpenThread APIs.
        # https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags
        # https://learn.microsoft.com/en-us/windows/win32/toolhelp/thread-walking
        snapshot = kernel.CreateToolhelp32Snapshot(0x00000004, 0)  # SNAPTHREAD
        if snapshot == ctypes.c_void_p(-1).value:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            entry = ThreadEntry()
            entry.dwSize = ctypes.sizeof(entry)
            if not kernel.Thread32First(snapshot, ctypes.byref(entry)):
                raise ctypes.WinError(ctypes.get_last_error())
            threads = []
            while True:
                if entry.th32OwnerProcessID == process.pid:
                    threads.append(entry.th32ThreadID)
                entry.dwSize = ctypes.sizeof(entry)
                if not kernel.Thread32Next(snapshot, ctypes.byref(entry)):
                    if ctypes.get_last_error() != 18:  # NO_MORE_FILES
                        raise ctypes.WinError(ctypes.get_last_error())
                    break
            if len(threads) != 1:
                raise RuntimeError('suspended command has no unique initial thread')
            thread = kernel.OpenThread(0x0002, False, threads[0])  # SUSPEND_RESUME
            if not thread:
                raise ctypes.WinError(ctypes.get_last_error())
            try:
                if kernel.ResumeThread(thread) != 1:
                    raise RuntimeError('command initial thread was not suspended exactly once')
            finally:
                if not kernel.CloseHandle(thread):
                    raise ctypes.WinError(ctypes.get_last_error())
        finally:
            if not kernel.CloseHandle(snapshot):
                raise ctypes.WinError(ctypes.get_last_error())
        yield
    finally:
        try:
            if job and not kernel.CloseHandle(job):
                raise ctypes.WinError(ctypes.get_last_error())
            if not assigned:
                # Failure to establish custody is infrastructure failure;
                # never continue this command without the requested job.
                cleanup = subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'],
                                         stdin=subprocess.DEVNULL, capture_output=True, timeout=10)
                if cleanup.returncode and process.poll() is None:
                    process.kill()
                    raise RuntimeError('Windows command-tree cleanup was not confirmed')
        finally:
            process.wait(timeout=10)


def main(*, expected_input_objects=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preparation', required=True, type=Path)
    parser.add_argument('--expected-compiler-source', required=True)
    parser.add_argument('--input-root', type=Path, default=Path.cwd())
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    root = Path.cwd().resolve()
    input_root = args.input_root.resolve()
    output = args.out.resolve()
    assert re.fullmatch(r'[0-9a-f]{40}', args.expected_compiler_source)
    assert input_root == root or (root / 'build') in input_root.parents
    assert (root / BASE) in output.parents
    assert not output.exists()

    def git(*arguments, optional=False):
        result = subprocess.run(['git', *arguments], cwd=root,
                                stdin=subprocess.DEVNULL, capture_output=True)
        if optional and result.returncode:
            return None
        result.check_returncode()
        return result.stdout.decode().strip()

    revision = git('rev-parse', 'HEAD')
    assert not git('status', '--porcelain')
    preparation_hash = sha(args.preparation)
    preparation = json.loads(args.preparation.read_text())
    assert preparation['schema'] == 'omega-compiler-preparation-v1'
    assert preparation['passed'] is True
    producer = preparation['producer']
    assert preparation['candidate'] == producer['revision'] == revision
    assert producer['tree'] == git('rev-parse', 'HEAD^{tree}')
    assert producer['target'] == preparation['target']
    machine = platform.machine().lower()
    machine = 'arm64' if machine in {'aarch64', 'arm64'} else machine
    machine = 'x86_64' if machine in {'amd64', 'x86_64'} else machine
    system = {'darwin': 'macos', 'windows': 'windows', 'linux': 'linux'}[platform.system().lower()]
    assert producer['target'] == system + '_' + machine
    # CI may carry the reviewed checkpoint's exact object roster without its
    # unpublished commit. Current checkout objects must still match every row.
    source_inputs = ({
        name: git('rev-parse', args.expected_compiler_source + ':' + name, optional=True)
        for name in INPUT_PATHS
    } if expected_input_objects is None else dict(expected_input_objects))
    assert set(source_inputs) == set(INPUT_PATHS)
    # Only the historically absent .cargo path may lack an object.
    assert all((name == '.cargo' and value is None)
               or (isinstance(value, str) and re.fullmatch(r'[0-9a-f]{40}', value))
               for name, value in source_inputs.items())
    cli = Path(producer['cli'])
    runner = Path(producer['runner'])
    assert require_optimized(cli) == producer['cli_profile']
    assert require_optimized(runner) == producer['runner_profile']
    library_root = root / 'source/library'
    library_names = git('ls-files', '-z', '--', 'source/library').split('\0')
    library_inputs = {name: sha(root / name) for name in library_names if name}
    assert library_inputs
    library_modules = {name for name in library_inputs if name.endswith('.omg')}

    inputs = {}
    groups = []
    controls = []
    for count, relative, pinned in ROSTERS:
        roster_path = input_root / relative
        assert sha(roster_path) == pinned
        inputs[str(relative)] = pinned
        roster = json.loads(roster_path.read_text())
        rows = roster['cases'] if count == 60 else roster['fixtures']
        assert len(rows) == count
        group = dict(count=count, roster=str(relative), sha256=pinned,
                     positive=35 if count == 60 else 19,
                     negative=25 if count == 60 else 19)
        groups.append(group)
        for item in rows:
            files = item['files']
            normalized = [
                (entry['path'], entry['sha256']) if count == 60
                else (entry['staged'], entry['staged_sha256'])
                for entry in files
            ]
            for name, pinned_file in normalized:
                path = Path(name)
                assert not path.is_absolute() and '..' not in path.parts
                assert path.parts[:2] in {('build', 'supplied-output-loan'), ('tests', 'omega')}
                assert name not in inputs or inputs[name] == pinned_file
                inputs[name] = pinned_file
            negative = item['negative'] if count == 60 else not item['positive']
            source = item['source'] if count == 60 else next(
                name for name, _ in normalized if name.endswith('/main.omg'))
            assert source in dict(normalized)
            pending = bool(item.get('pending_reject_diagnostic', False))
            if count == 60:
                fragments = item['expected_reject_fragments']
                expected_check = item['expected_check_exit']
            else:
                expected_files = [name for name, _ in normalized if name.endswith('/expected.txt')]
                assert len(expected_files) <= 1
                fragments = ((input_root / expected_files[0]).read_text().splitlines()
                             if expected_files else [])
                fragments = [text for text in fragments if text]
                expected_check = 1 if negative else 0
            if negative:
                assert fragments or pending
            else:
                assert expected_check == 0 and not fragments and not pending
                authored = [int(value) for value in re.findall(
                    r'^// Expected exit: (-?\d+)\.?$', (input_root / source).read_text(), re.M)]
                authored.extend(int((input_root / name).read_text().strip()) for name, _ in normalized
                                if name.endswith(('expected_native_exit.txt', 'expected_interpreter_exit.txt')))
                assert authored and set(authored) == {0}
            controls.append(dict(group=count, name=item.get('name', item.get('fixture')),
                                 source=source, files=normalized, negative=negative,
                                 expected_check=expected_check, fragments=fragments,
                                 pending_reject_diagnostic=pending))
        assert sum((item['negative'] if count == 60 else not item['positive']) for item in rows) == group['negative']
    assert len(controls) == 98 and len(inputs) == 270
    assert controls[0]['source'].endswith('handback-alternative-control/positive/main.omg')

    def validate():
        assert git('rev-parse', 'HEAD') == revision
        assert not git('status', '--porcelain')
        assert sha(args.preparation) == preparation_hash
        assert require_optimized(cli) == producer['cli_profile']
        assert require_optimized(runner) == producer['runner_profile']
        for name, identity in source_inputs.items():
            assert git('rev-parse', 'HEAD:' + name, optional=True) == identity, name
        actual_modules = {str(path.relative_to(root)).replace(os.sep, '/')
                          for path in library_root.rglob('*.omg') if path.is_file()}
        assert actual_modules == library_modules, 'actual toolchain library module inventory changed'
        for name, identity in library_inputs.items():
            path = root / name
            assert path.is_file() and not path.is_symlink(), name
            assert library_root in path.resolve().parents, name
            assert sha(path) == identity, name
        for name, identity in inputs.items():
            path = input_root / name
            assert path.is_file() and not path.is_symlink(), name
            assert input_root in path.resolve().parents, name
            assert sha(path) == identity, name

    validate()
    output.mkdir(parents=True, exist_ok=False)
    environment = {name: value for name, value in os.environ.items()
                   if name not in {'OMEGA_CALL_WRITE_TRACE', 'OMEGA_STRUCT_TRACE', 'OMEGA_COMPILER_CHECK_SITES'}}
    environment['OMEGA_LIBRARY_ROOT'] = str(library_root)
    observations = []
    failures = []
    run_identity = dict(revision=revision, expected_compiler_source=args.expected_compiler_source,
                        compiler_inputs=source_inputs, preparation_sha256=preparation_hash,
                        producer=producer, input_root=str(input_root),
                        explicit_relocation=input_root != root, groups=groups,
                        effective_environment_overrides={'OMEGA_LIBRARY_ROOT': str(library_root)},
                        actual_library_input_sha256=library_inputs)

    def retain():
        (output / 'observations.json').write_text(json.dumps(dict(
            identity=run_identity, observations=observations, failures=failures), indent=2) + '\n')

    def run(control, mode, arguments):
        validate()
        # Preserve the existing capture floor and per-command timeout.
        assert shutil.disk_usage(output).free >= 800_000_000
        stem = str(control['group']) + '-' + control['name'].replace('/', '__') + '-' + mode
        stdout = output / (stem + '.stdout')
        stderr = output / (stem + '.stderr')
        command = [str(cli), *map(str, arguments)]
        started = time.monotonic()
        timed_out = False
        process = None
        command_error = None
        try:
            with stdout.open('xb') as out, stderr.open('xb') as err:
                process = subprocess.Popen(command, cwd=root, env=environment,
                                           stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                           start_new_session=os.name != 'nt',
                                           creationflags=(subprocess.CREATE_NEW_PROCESS_GROUP | 0x00000004) if os.name == 'nt' else 0)
                with owned_process_tree(process):
                    try:
                        process.wait(timeout=300)
                    except subprocess.TimeoutExpired:
                        timed_out = True
        except BaseException as error:
            command_error = type(error).__name__
            failures.append(dict(group=control['group'], name=control['name'],
                                 reason='command infrastructure/interruption', error=command_error))
            retain()
            raise
        finally:
            # This runs after process-tree closure, including interruptions.
            # A changed source, library or producer cannot become acceptance.
            try:
                validate()
            except BaseException as error:
                failures.append(dict(group=control['group'], name=control['name'],
                                     reason='post-command input identity changed', error=type(error).__name__))
                retain()
                raise
        row = dict(group=control['group'], name=control['name'], negative=control['negative'],
                   source=control['source'], mode=mode, command=command,
                   exit=process.returncode, timed_out=timed_out, process_tree_closed=True,
                   seconds=round(time.monotonic() - started, 3),
                   stdout=str(stdout), stdout_sha256=sha(stdout),
                   stderr=str(stderr), stderr_sha256=sha(stderr))
        observations.append(row)
        retain()
        print(stem, row['exit'], row['seconds'], flush=True)
        if timed_out:
            failures.append(dict(group=control['group'], name=control['name'], reason='command timeout'))
            retain()
            validate()
            raise SystemExit(124)
        return row, stderr.read_text(errors='replace')

    for control in controls:
        validate()
        source = input_root / control['source']
        checked, text = run(control, 'check', ['--check', source])
        if checked['exit'] != control['expected_check']:
            failures.append(dict(group=control['group'], name=control['name'],
                                 reason='unexpected source exit', expected=control['expected_check'], actual=checked['exit']))
        if control['negative']:
            missing = [fragment for fragment in control['fragments'] if fragment not in text]
            if missing or control['pending_reject_diagnostic']:
                failures.append(dict(group=control['group'], name=control['name'],
                                     reason='intended rejection missing or still unpinned', missing=missing,
                                     pending=control['pending_reject_diagnostic']))
            retain()
            validate()
            continue
        if checked['exit'] != 0:
            failures.append(dict(group=control['group'], name=control['name'], reason='positive cannot reach engines/admission'))
            retain()
            continue
        actual, text = run(control, 'both', ['run', '--both', '--keep', source])
        native = re.findall(r'^native exit (-?\d+)$', text, re.M)
        terminal = re.findall(r'^interp exit: (-?\d+)$', text, re.M)
        actual['native_exit'] = int(native[0]) if len(native) == 1 else None
        actual['terminal_exit'] = int(terminal[0]) if len(terminal) == 1 else None
        if actual['exit'] != 0 or actual['native_exit'] != 0 or actual['terminal_exit'] != 0:
            failures.append(dict(group=control['group'], name=control['name'], reason='missing or unsuccessful engine agreement'))
        kept = re.findall(r'^build dir kept: (.+)$', text, re.M)
        actual['kept_build_directory'] = kept[0] if len(kept) == 1 else None
        artifact = Path(kept[0]) / 'program.oterm' if len(kept) == 1 else None
        if artifact is None or not artifact.is_file() or artifact.is_symlink():
            failures.append(dict(group=control['group'], name=control['name'], reason='no same-invocation kept artifact'))
            retain()
            continue
        before = sha(artifact)
        retained = output / (str(control['group']) + '-' + control['name'].replace('/', '__') + '-artifacts')
        shutil.copytree(artifact.parent, retained)
        copied = retained / 'program.oterm'
        assert sha(copied) == before == sha(artifact)
        inspected, _ = run(control, 'inspect', ['inspect-terminal', '--artifact', copied])
        after = sha(copied)
        kept_after = sha(artifact)
        inspected.update(artifact=str(copied), sha256_before=before, sha256_after=after,
                         kept_sha256_after=kept_after, same_as_kept_product=before == after == kept_after)
        actual['artifact_sha256'] = before
        if inspected['exit'] != 0 or not inspected['same_as_kept_product']:
            failures.append(dict(group=control['group'], name=control['name'], reason='same-artifact canonical admission failed'))
        retain()
        validate()

    validate()
    for group in groups:
        group_rows = [row for row in observations if row['group'] == group['count']]
        assert sum(row['mode'] == 'check' for row in group_rows) == group['count']
        required = {item['name'] for item in controls if item['group'] == group['count'] and not item['negative']}
        for mode in ['both', 'inspect']:
            missing = sorted(required - {row['name'] for row in group_rows if row['mode'] == mode})
            if missing:
                failures.append(dict(group=group['count'], reason='required ' + mode + ' observations absent', missing=missing))
    report = dict(identity=run_identity, accepted=not failures, failures=failures,
                  checked_controls=98, required_actual_engine_and_admission_controls=54,
                  intended_reject_controls=44,
                  scope='Only both exact frozen groups. Pending rejection pins stay unqualified. Separate native retained-target audit, other controls, permanent fixtures and whole-candidate landing remain required.')
    retain()
    (output / 'acceptance.json').write_text(json.dumps(report, indent=2) + '\n')
    print('FROZEN_COHORTS_ACCEPTED', report['accepted'], 'failures', len(failures), flush=True)
    return 0 if report['accepted'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
