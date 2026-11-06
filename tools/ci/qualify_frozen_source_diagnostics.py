"""Retain source-check diagnostics for four fixed frozen positives only.

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


SOURCE_DIAGNOSTIC_CONTROLS = ((60, 'compatible_view_from_supplied_reference'), (60, 'runtime_selected_shared_descriptor_store'), (60, 'retained_incoming_conditional'), (38, 'tests/omega/pass/borrows/supplied_output_conditional_helper_keeps_reference_exit'))

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


def main(*, expected_input_objects, source_diagnostics=True):
    assert source_diagnostics is True and expected_input_objects is not None
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
    source_inputs = dict(expected_input_objects)
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
    if source_diagnostics:
        indexed = {(item['group'], item['name']): item for item in controls}
        assert len(indexed) == 98
        controls = [indexed[key] for key in SOURCE_DIAGNOSTIC_CONTROLS]
        assert len(controls) == 4 and all(not item['negative'] for item in controls)

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
    if source_diagnostics:
        environment['OMEGA_COMPILER_CHECK_SITES'] = '1'
    observations = []
    failures = []
    run_identity = dict(revision=revision, expected_compiler_source=args.expected_compiler_source,
                        compiler_inputs=source_inputs, preparation_sha256=preparation_hash,
                        producer=producer, input_root=str(input_root),
                        explicit_relocation=input_root != root, groups=groups,
                        effective_environment_overrides={'OMEGA_LIBRARY_ROOT': str(library_root)},
                        actual_library_input_sha256=library_inputs)
    if source_diagnostics:
        run_identity.update(schema='omega-four-frozen-source-diagnostics-v1',
                            scope='Source05 checking of four fixed positives only; no engines or canonical admission.',
                            full98_accepted=False,
                            effective_environment_overrides=dict(OMEGA_LIBRARY_ROOT=str(library_root),
                                                                 OMEGA_COMPILER_CHECK_SITES='1'))

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
        if checked['exit'] != 0:
            failures.append(dict(group=control['group'], name=control['name'],
                                 reason='source check refused', actual=checked['exit']))
        retain()
        validate()

    validate()
    assert [(row['group'], row['name']) for row in observations] == list(SOURCE_DIAGNOSTIC_CONTROLS)
    assert all(row['mode'] == 'check' for row in observations)
    report = dict(schema='omega-four-frozen-source-diagnostics-v1', identity=run_identity,
                  source_checks_completed=4, all_four_check_exit_zero=not failures,
                  full98_accepted=False, engine_observations=0, failures=failures,
                  scope='Source05 diagnostics only. No BOTH/native/interpreter/artifact receiving or full98 qualification.')
    retain()
    (output / 'source-diagnostics.json').write_text(json.dumps(report, indent=2) + '\n')
    print('FROZEN_SOURCE_CHECKS_ZERO', report['all_four_check_exit_zero'],
          'FULL98_ACCEPTED', False, flush=True)
    return 0 if report['all_four_check_exit_zero'] else 1


if __name__ == '__main__':
    raise SystemExit('Use run_frozen_source_diagnostics.py with an independently reviewed ten-object source map')
