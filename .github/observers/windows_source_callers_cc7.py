"""Bounded caller observations of one original corpus-runner Source prefix.

This deliberately interrupted diagnostic never grades a fixture or writes goldens.
"""
from pathlib import Path
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import threading
import time
import traceback


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def section(text, label):
    match = re.search(r'^' + re.escape(label + '_BEGIN') + r'\r?\n(.*?)^'
                      + re.escape(label + '_END') + r'\r?$', text, re.M | re.S)
    assert match, 'missing acknowledged section: ' + label
    return match.group(1)


class DebuggerOutput:
    def __init__(self, process, output):
        self.process = process
        self.bytes = bytearray()
        self.condition = threading.Condition()
        self.file = (output / 'debugger.stdout').open('wb')
        self.thread = threading.Thread(target=self.read, daemon=True)
        self.thread.start()

    def read(self):
        try:
            while data := self.process.stdout.read1(4096):
                self.file.write(data)
                self.file.flush()
                with self.condition:
                    self.bytes.extend(data)
                    self.condition.notify_all()
        finally:
            self.file.close()

    def text(self):
        with self.condition:
            return self.bytes.decode('utf-8', errors='replace')

    def await_marker(self, marker, deadline):
        with self.condition:
            while not re.search(r'^' + re.escape(marker) + r'\r?$',
                                self.bytes.decode('utf-8', errors='replace'), re.M):
                remaining = deadline - time.monotonic()
                assert remaining > 0 and self.process.poll() is None, 'debugger acknowledgement missing: ' + marker
                self.condition.wait(min(remaining, 0.2))


def main():
    recipe_path = Path(sys.argv[1]).resolve()
    recipe = json.loads(recipe_path.read_text())
    root = Path.cwd().resolve()
    output = root / 'build/atomic-windows-source-callers-cc7'
    output.mkdir(parents=True, exist_ok=False)
    report = {'passed': False, 'qualification': False, 'recipe_sha256': sha(recipe_path),
              'classification': 'Interrupted Source prefix; caller diagnostic only, never fixture acceptance.',
              'samples': []}
    process = reader = runner = pdb = None
    process_handle = job = None
    runner_identity_verified = False
    started = None
    k32 = None
    handler = None
    try:
        assert os.name == 'nt' and platform.machine().lower() in ('amd64', 'x86_64')
        git = lambda *args: subprocess.check_output(['git', *args], text=True, timeout=10).strip()
        assert git('rev-parse', 'HEAD') == recipe['source']
        assert git('rev-parse', 'HEAD^{tree}') == recipe['tree'] and not git('status', '--porcelain')
        assert os.environ['OMEGA_DEBUG_START_CACHE_HIT'] == 'true'
        assert os.environ['OMEGA_DEBUG_START_CACHE_KEY'] == recipe['cache']['key']
        assert os.environ['OMEGA_CORPUS_FIXTURE_SECS'] == '120'
        observer = os.environ['OMEGA_DEBUG_START_OBSERVER']
        assert observer == os.environ['GITHUB_SHA'] and observer != recipe['source']
        for path, expected in recipe['compiler_and_workflow_objects'].items():
            assert git('rev-parse', 'HEAD:' + path) == expected
        for path, expected in recipe['inputs'].items():
            assert sha(root / path) == expected, path
        for kind, endpoint in [('run', 'actions/runs/'), ('artifact', 'actions/artifacts/')]:
            identity = recipe['origin_run'] if kind == 'run' else recipe['origin_artifact']['id']
            value = json.loads(subprocess.check_output(
                ['gh', 'api', 'repos/CathedralOS/Omega/' + endpoint + str(identity)],
                timeout=15, text=True))
            (output / ('origin-' + kind + '.json')).write_text(json.dumps(value, indent=2) + '\n')
            if kind == 'run':
                assert value['status'] == 'completed' and value['head_sha'] == recipe['source']
            else:
                assert value['workflow_run']['id'] == recipe['origin_run'] and not value['expired']
                assert value['workflow_run']['head_sha'] == recipe['source']
                for field in ('id', 'name', 'size_in_bytes', 'digest'):
                    assert value[field] == recipe['origin_artifact'][field]
        report.update(source=recipe['source'], tree=recipe['tree'], observer=observer,
                      producer=recipe['producer'], host=platform.platform(), frozen_inputs=len(recipe['inputs']))

        # Bounded cached-output discovery, selected by complete original digest.
        # The authenticated original Cargo receipt identifies mbx's actual
        # output unit; the ordinary deps glob alone does not cover this layout.
        fingerprint = Path(recipe['runner_build_fingerprint']['path'])
        assert sha(fingerprint) == recipe['runner_build_fingerprint']['sha256'], 'cached runner fingerprint differs'
        unit_output = fingerprint.parent.parent / 'out'
        assert unit_output == Path(recipe['runner_output_directory'])
        candidates = sorted(set(Path('D:/ot/debug/deps').glob('corpus_runner*.exe'))
                            | set(unit_output.glob('corpus_runner*.exe'))
                            | {Path('D:/ot/debug/corpus_runner.exe')})
        candidates = [path for path in candidates if path.is_file()]
        report['runner_candidate_inventory'] = [{'path': str(path), 'bytes': path.stat().st_size}
                                                for path in candidates]
        assert 0 < len(candidates) <= 32, 'cached runner candidate count outside 1..32'
        assert sum(path.stat().st_size for path in candidates) <= 1024 ** 3, 'cached runner candidate bytes exceed 1 GiB'
        rows = [{'path': str(path), 'sha256': sha(path)} for path in candidates]
        matches = [Path(row['path']) for row in rows if row['sha256'] == recipe['producer']['runner_profile']['sha256']]
        assert matches, 'no exact original runner in the restored cache'
        # Cargo may retain byte-identical top-level and deps outputs; choose the
        # sole exact output with its own adjacent PDB, never a name-only output.
        paired = [(path, path.with_suffix('.pdb')) for path in matches if path.with_suffix('.pdb').is_file()]
        assert len(paired) == 1, 'one exact runner with an adjacent local PDB is required'
        cached_runner, pdb = paired[0]
        assert not cached_runner.is_symlink() and not pdb.is_symlink()
        pdb_before = sha(pdb)
        assert pdb_before == recipe['runner_pdb_sha256'], 'cached original runner PDB changed'
        runner = Path(recipe['producer']['runner'])
        assert root == runner.parents[3], 'the original runner command requires its original checkout root'
        runner.parent.mkdir(parents=True, exist_ok=False)
        shutil.copyfile(cached_runner, runner)
        assert sha(runner) == recipe['producer']['runner_profile']['sha256']
        report.update(cached_runner=str(cached_runner), runner_candidates=rows, runner=str(runner),
                      pdb=str(pdb), pdb_sha256=pdb_before, pdb_bytes=pdb.stat().st_size)
        keys = ('SYSTEMROOT', 'WINDIR', 'PATH', 'PATHEXT', 'TEMP', 'TMP',
                'USERPROFILE', 'LOCALAPPDATA', 'APPDATA', 'COMSPEC')
        child_env = {key: value for key, value in os.environ.items() if key.upper() in keys}
        profile = subprocess.run([str(runner), '--build-profile'], env=child_env,
                                 stdin=subprocess.DEVNULL, capture_output=True, timeout=10)
        (output / 'build-profile.stdout').write_bytes(profile.stdout)
        (output / 'build-profile.stderr').write_bytes(profile.stderr)
        assert profile.returncode == 0 and json.loads(profile.stdout) == recipe['producer']['runner_profile']['profile']
        source_env = {
            'OMEGA_CORPUS_ROOT': str(root), 'OMEGA_LIBRARY_ROOT': str(root / 'source/library'),
            'OMEGA_CORPUS_RECORDS': str(output / 'corpus-records.txt'),
            'OMEGA_CORPUS_FIXTURE_FILTER': recipe['fixture'], 'OMEGA_CORPUS_JOBS': '1',
            'OMEGA_CORPUS_FIXTURE_SECS': '120',
            'OMEGA_CORPUS_TIMINGS': str(root / 'tests/omega/corpus_outcomes.txt'),
            'OMEGA_CORPUS_SWEEP': '1', 'RUST_MIN_STACK': '67108864',
            'OMEGA_CORPUS_CAPTURE_ARTIFACTS': str(output / 'captures'),
        }
        child_env.update(source_env)
        report.update(source_command=[str(runner)], source_cwd=str(root), source_environment=source_env,
                      phase_profile=False, planned_snapshots_seconds=[60, 90], source_prefix_limit_seconds=115)
        # Create a console owned only by this observer before creating CDB.
        # A custom handler is process-local; the inheritable ignore-Ctrl+C flag
        # is never set. Never signal the Actions worker's inherited console.
        k32 = ctypes.WinDLL('kernel32', use_last_error=True)
        k32.FreeConsole.argtypes = []; k32.FreeConsole.restype = wintypes.BOOL
        k32.AllocConsole.argtypes = []; k32.AllocConsole.restype = wintypes.BOOL
        k32.GetConsoleProcessList.argtypes = [ctypes.POINTER(wintypes.DWORD), wintypes.DWORD]
        k32.GetConsoleProcessList.restype = wintypes.DWORD
        k32.GenerateConsoleCtrlEvent.argtypes = [wintypes.DWORD, wintypes.DWORD]
        k32.GenerateConsoleCtrlEvent.restype = wintypes.BOOL
        callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.DWORD)
        k32.SetConsoleCtrlHandler.argtypes = [callback_type, wintypes.BOOL]
        k32.SetConsoleCtrlHandler.restype = wintypes.BOOL
        handler = callback_type(lambda event: event == 0)
        k32.FreeConsole()
        assert k32.AllocConsole(), ctypes.get_last_error()
        assert k32.SetConsoleCtrlHandler(handler, True), ctypes.get_last_error()

        def console_members():
            values = (wintypes.DWORD * 16)()
            count = k32.GetConsoleProcessList(values, len(values))
            assert 0 < count <= len(values)
            return set(values[:count])

        assert console_members() == {os.getpid()}
        debugger = Path(recipe['debugger']['path'])
        assert sha(debugger) == recipe['debugger']['sha256']
        initial = ('.echo RUNNER_MODULES_BEGIN; lm; .echo RUNNER_MODULES_END; '
                   '.echo OWNED_PROCESS_BEGIN; |; .echo OWNED_PROCESS_END; .echo INITIAL_READY')
        init_file = output / 'empty-init.cdb'
        init_file.write_bytes(b'')
        report['init_file_sha256'] = sha(init_file)
        arguments = [str(debugger), '-sins', '-noshell', '-nosqm', '-pt', '5',
                     '-cf', str(init_file),
                     '-y', str(pdb.parent), '-c', initial, str(runner)]
        report.update(arguments=arguments, initial_commands=initial, debugger_sha256=sha(debugger))
        stderr = (output / 'debugger.stderr').open('wb')
        process = subprocess.Popen(arguments, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=stderr, cwd=root, env=child_env)
        reader = DebuggerOutput(process, output)
        report['debugger_pid'] = process.pid
        reader.await_marker('INITIAL_READY', time.monotonic() + 20)
        text = reader.text()
        # CDB retains the original linked image name after the executable is
        # copied to the original producer argv path. Resolve its live module
        # token through this stopped process's exact image/base correspondence.
        assert cached_runner.name == recipe['runner_debug_image']
        loads = re.findall(r'^ModLoad:\s+([0-9a-f`]+)\s+([0-9a-f`]+)\s+'
                           + re.escape(cached_runner.name) + r'\s*$', text, re.M | re.I)
        assert len(loads) == 1, 'one actual main-image load is required'
        main_base, main_end = loads[0]
        modules = re.findall(r'^\s*' + re.escape(main_base) + r'\s+' + re.escape(main_end)
                             + r'\s+(\S+)\s+[^\r\n]+$', section(text, 'RUNNER_MODULES'), re.M | re.I)
        assert len(modules) == 1
        module = modules[0]
        assert re.fullmatch(r'[A-Za-z0-9_-]+', module)
        owned = re.findall(r'^\s*[.# ]*\s*0\s+id:\s*([0-9a-f]+)\s+.*'
                           + re.escape(cached_runner.name) + r'\s*$',
                           section(text, 'OWNED_PROCESS'), re.M | re.I)
        assert len(owned) == 1
        pid = int(owned[0], 16)
        expected_console = {os.getpid(), process.pid, pid}
        assert len(expected_console) == 3 and console_members() == expected_console
        k32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        k32.OpenProcess.restype = wintypes.HANDLE
        k32.CloseHandle.argtypes = [wintypes.HANDLE]; k32.CloseHandle.restype = wintypes.BOOL
        k32.QueryFullProcessImageNameW.argtypes = [wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)]
        k32.QueryFullProcessImageNameW.restype = wintypes.BOOL
        k32.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(ctypes.c_uint64)] * 4
        k32.GetProcessTimes.restype = wintypes.BOOL
        # Only the created, stopped runner is opened. Job assignment needs
        # SET_QUOTA|TERMINATE; no all-access handle or token adjustment.
        process_handle = k32.OpenProcess(0x1000 | 0x100000 | 0x0100 | 0x0001, False, pid)
        assert process_handle, ctypes.get_last_error()

        def cpu():
            size = wintypes.DWORD(32768)
            image = ctypes.create_unicode_buffer(size.value)
            assert k32.QueryFullProcessImageNameW(process_handle, 0, image, ctypes.byref(size))
            assert Path(image.value).resolve() == runner.resolve()
            created, exited, kernel, user = [ctypes.c_uint64() for _ in range(4)]
            assert k32.GetProcessTimes(process_handle, *[ctypes.byref(value) for value in (created, exited, kernel, user)])
            assert exited.value == 0, 'the source runner already exited'
            return {'pid': pid, 'image': image.value, 'created_100ns': created.value,
                    'kernel_100ns': kernel.value, 'user_100ns': user.value}

        k32.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
        k32.WaitForSingleObject.restype = wintypes.DWORD
        k32.TerminateProcess.argtypes = [wintypes.HANDLE, wintypes.UINT]
        k32.TerminateProcess.restype = wintypes.BOOL
        baseline = cpu()
        runner_identity_verified = True
        def send(command):
            process.stdin.write((command + '\n').encode('ascii'))
            process.stdin.flush()

        pdb_commands = ('.reload /f ' + cached_runner.name + '; .echo RUNNER_LMI_BEGIN; !lmi '
                        + main_base + '; .echo RUNNER_LMI_END; .echo RUNNER_LMVM_BEGIN; lmv a '
                        + main_base + '; .echo RUNNER_LMVM_END; .echo RUNNER_PDB_READY')
        send(pdb_commands)
        reader.await_marker('RUNNER_PDB_READY', time.monotonic() + 20)
        lmi, lmvm = section(reader.text(), 'RUNNER_LMI'), section(reader.text(), 'RUNNER_LMVM')
        lmi_modules = re.findall(r'^\s*Module:\s+(\S+)\s*$', lmi, re.M | re.I)
        assert [name.lower() for name in lmi_modules] == [cached_runner.stem.lower()]
        # !lmi names the linked image stem; lm uses the debugger symbol token.
        # Keep both exact roles joined through the same actual module range.
        assert str(runner).lower().replace('\\', '/') in lmi.lower().replace('\\', '/')
        assert re.search(r'^\s*Symbol Type:\s+PDB\b', lmi, re.M | re.I)
        assert re.search(r'^\s*' + re.escape(main_base) + r'\s+' + re.escape(main_end)
                         + r'\s+' + re.escape(module) + r'\s+[^\r\n]*\((?:private )?pdb symbols\)', lmvm, re.M | re.I)
        normalized = lmvm.lower().replace('\\', '/')
        assert str(pdb).lower().replace('\\', '/') in normalized
        # The debugger image label may be the original linker name. The
        # authoritative full launched path is independently pinned by the held
        # OS process handle, its creation time and the complete runner digest.
        assert cpu()['created_100ns'] == baseline['created_100ns']
        report.update(runner_debug_image=cached_runner.name, runner_debug_module=module,
                      runner_lmi_image_stem=lmi_modules[0],
                      runner_module_base=main_base, runner_module_end=main_end,
                      pdb_commands=pdb_commands, matching_runner_pdb=True)
        # Strictly establish both memberships: CDB may have created its child
        # before a parent-only job assignment. Failure never silently yields.
        class BasicLimits(ctypes.Structure):
            _fields_ = [("PerProcessUserTimeLimit", ctypes.c_int64),
                        ("PerJobUserTimeLimit", ctypes.c_int64),
                        ("LimitFlags", ctypes.c_uint32),
                        ("MinimumWorkingSetSize", ctypes.c_size_t),
                        ("MaximumWorkingSetSize", ctypes.c_size_t),
                        ("ActiveProcessLimit", ctypes.c_uint32),
                        ("Affinity", ctypes.c_size_t),
                        ("PriorityClass", ctypes.c_uint32),
                        ("SchedulingClass", ctypes.c_uint32)]
        class IoCounters(ctypes.Structure):
            _fields_ = [(name, ctypes.c_uint64) for name in (
                "ReadOperationCount", "WriteOperationCount", "OtherOperationCount",
                "ReadTransferCount", "WriteTransferCount", "OtherTransferCount")]
        class ExtendedLimits(ctypes.Structure):
            _fields_ = [("BasicLimitInformation", BasicLimits), ("IoInfo", IoCounters),
                        ("ProcessMemoryLimit", ctypes.c_size_t), ("JobMemoryLimit", ctypes.c_size_t),
                        ("PeakProcessMemoryUsed", ctypes.c_size_t), ("PeakJobMemoryUsed", ctypes.c_size_t)]
        k32.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
        k32.CreateJobObjectW.restype = wintypes.HANDLE
        k32.SetInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]
        k32.SetInformationJobObject.restype = wintypes.BOOL
        k32.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
        k32.AssignProcessToJobObject.restype = wintypes.BOOL
        k32.IsProcessInJob.argtypes = [wintypes.HANDLE, wintypes.HANDLE, ctypes.POINTER(wintypes.BOOL)]
        k32.IsProcessInJob.restype = wintypes.BOOL
        job = k32.CreateJobObjectW(None, None)
        assert job, ctypes.get_last_error()
        limits = ExtendedLimits()
        limits.BasicLimitInformation.LimitFlags = 0x2000 | 0x200
        limits.JobMemoryLimit = 32 * 1024 ** 3
        assert k32.SetInformationJobObject(job, 9, ctypes.byref(limits), ctypes.sizeof(limits)), ctypes.get_last_error()
        for name, handle in [('debugger', int(process._handle)), ('runner', process_handle)]:
            assert k32.AssignProcessToJobObject(job, handle), (name, ctypes.get_last_error())
            member = wintypes.BOOL()
            assert k32.IsProcessInJob(handle, job, ctypes.byref(member)) and member.value
        report['verified_job_membership'] = {'debugger': process.pid, 'runner': pid,
                                             'kill_on_close': True, 'memory_bytes': limits.JobMemoryLimit}
        report.update(runner_pid=pid, initial_cpu=baseline, matching_runner_pdb=True,
                      private_console_members=sorted(expected_console))
        def channels():
            return {'stdout': reader.text(),
                    'stderr': (output / 'debugger.stderr').read_text(errors='replace')}

        def channel_offsets():
            return {name: len(text) for name, text in channels().items()}

        def reject_uncommanded_stop(start_offsets, label):
            current = channels()
            active = {name: text[start_offsets[name]:] for name, text in current.items()}
            for name, text in active.items():
                if re.search(r'(?:exception[^\r\n]*(?:code|chance)|Breakpoint[^\r\n]*hit|'
                             r'exited[^\r\n]*(?:code|process)|exit process)', text, re.I):
                    for channel, contents in active.items():
                        (output / ('unexpected-stop-' + label + '.' + channel)).write_text(contents)
                    report['unexpected_stop'] = {'start_characters': start_offsets,
                                                 'end_characters': {key: len(value) for key, value in current.items()},
                                                 'channel': name, 'interval': label}
                    raise AssertionError('uncommanded debugger stop before timed interrupt')
            return active

        def root_reports():
            # The original runner uses eprintln!: retain and read stderr as
            # well as CDB stdout, rather than assuming channel redirection.
            return [match for text in channels().values() for match in
                    re.findall(r'^corpus_runner: fixture-root-v1 ([0-9a-f]+)\r?\n', text, re.M)]

        started = time.monotonic()
        active_start = channel_offsets()
        send('g')  # No synchronous communicate(): observer schedules interrupts independently.
        # The original runner protocol proves g reached the real runner main,
        # rather than allowing two copies of the initial-break stack to pass.
        while not root_reports():
            assert time.monotonic() < started + 10 and process.poll() is None, 'runner main not acknowledged'
            reject_uncommanded_stop(active_start, 'initial-resume')
            time.sleep(0.05)
        roots = root_reports()
        assert len(roots) == 1 and Path(os.fsdecode(bytes.fromhex(roots[0]))).resolve() == root
        report['runner_root_acknowledged'] = roots[0]
        for offset in (60, 90):
            while time.monotonic() < started + offset:
                assert process.poll() is None, 'debugger exited before caller sample'
                reject_uncommanded_stop(active_start, str(offset))
                time.sleep(max(0.001, min(0.2, started + offset - time.monotonic())))
            assert time.monotonic() < started + 100
            assert console_members() == expected_console, 'refuse signal outside the private owned console'
            before = cpu()
            assert before['created_100ns'] == baseline['created_100ns']
            label = 'SAMPLE_' + str(offset)
            active = reject_uncommanded_stop(active_start, str(offset))
            for channel, contents in active.items():
                (output / ('running-interval-' + str(offset) + '.' + channel)).write_text(contents)
            interval_end = channel_offsets()
            assert k32.GenerateConsoleCtrlEvent(0, 0), ctypes.get_last_error()
            commands = ('.echo ' + label + '_BEGIN; .lastevent; |; .time; !runaway 7; '
                        '~* kc 0n32; .echo ' + label + '_END')
            send(commands)
            reader.await_marker(label + '_END', min(started + 105, time.monotonic() + 8))
            sample = section(reader.text(), label)
            (output / ('callers-' + str(offset) + '.txt')).write_text(sample)
            # Resume only our expected debugger interrupt. Never swallow an
            # unexpected application exception or a natural process exit.
            assert re.search(r'exception[^\r\n]*code\s+(?:80000003|40010005|40010008)\b', sample, re.I), 'unrecognized stop reason'
            assert re.search(r'\b' + re.escape(module) + '!', sample), 'no symbolized exact runner caller frames'
            assert re.search(r'User Mode Time', sample, re.I), 'missing per-thread CPU report'
            assert not re.search(r'(?:exited|exit code|access violation|c0000005)', sample, re.I)
            after = cpu()
            assert after['created_100ns'] == baseline['created_100ns']
            assert console_members() == expected_console
            report['samples'].append({'offset_seconds': offset, 'observed_wall_seconds': round(time.monotonic() - started, 3),
                                      'before': before, 'after': after, 'commands': commands,
                                      'running_interval_start_characters': active_start,
                                      'running_interval_end_characters': interval_end})
            if offset == 60:
                active_start = channel_offsets()
                send('gh')
        send('q')
        process.wait(timeout=min(5, max(0.1, started + 110 - time.monotonic())))
        assert process.returncode == 0
        assert k32.WaitForSingleObject(process_handle, 2000) == 0, 'runner remains alive after q'
        assert time.monotonic() - started < 115
        report.update(passed=True, diagnostic_complete=True, debugger_exit=process.returncode,
                      source_prefix_seconds=round(time.monotonic() - started, 3))
        stderr.close()
    except Exception as error:
        report['error'] = repr(error)
        report['error_traceback'] = traceback.format_exc()
    finally:
        # Close the owned kill-on-close job even if CDB exited first. A pinned
        # identity-verified runner is independently reaped on setup failures.
        if job and k32:
            k32.CloseHandle(job)
            job = None
        if process_handle and k32 and runner_identity_verified:
            try:
                if k32.WaitForSingleObject(process_handle, 0) != 0:
                    assert k32.TerminateProcess(process_handle, 1), ctypes.get_last_error()
            except Exception as error:
                report['runner_termination_request_error'] = repr(error)
        if process is not None and process.poll() is None:
            try:
                killed = subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'],
                                        stdin=subprocess.DEVNULL, capture_output=True, timeout=5)
                (output / 'cleanup.stdout').write_bytes(killed.stdout)
                (output / 'cleanup.stderr').write_bytes(killed.stderr)
                report['cleanup_exit'] = killed.returncode
            except Exception as error:
                report['cleanup_error'] = repr(error)
            if process.poll() is None:
                process.kill()
            try:
                process.wait(timeout=3)
            except Exception as error:
                report['cleanup_wait_error'] = repr(error)
        # A stopped debuggee can keep its exit event pending until CDB is
        # reaped. Confirm the held runner only after debugger cleanup, rather
        # than treating that pending event as a permanent termination failure.
        if process_handle and k32 and runner_identity_verified:
            try:
                wait_result = k32.WaitForSingleObject(process_handle, 3000)
                report['runner_final_wait_result'] = wait_result
                assert wait_result == 0, ('runner still alive after debugger cleanup', wait_result)
                report['owned_runner_exited'] = True
            except Exception as error:
                report.update(passed=False, runner_cleanup_error=repr(error))
        if reader is not None:
            reader.thread.join(timeout=1)
        if 'stderr' in locals():
            stderr.close()
        if process_handle and k32:
            k32.CloseHandle(process_handle)
        if started is not None:
            report['source_prefix_with_cleanup_seconds'] = round(time.monotonic() - started, 3)
            if report['source_prefix_with_cleanup_seconds'] >= 120:
                report.update(passed=False, cleanup_budget_exceeded=True)
        try:
            if runner is not None:
                assert sha(runner) == recipe['producer']['runner_profile']['sha256']
                assert sha(Path(report['cached_runner'])) == recipe['producer']['runner_profile']['sha256']
                assert sha(pdb) == report['pdb_sha256']
            assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True, timeout=10).strip() == recipe['source']
            assert not subprocess.check_output(['git', 'status', '--porcelain'], text=True, timeout=10).strip()
            for path, expected in recipe['inputs'].items():
                assert sha(root / path) == expected
            report['postchecks_passed'] = True
        except Exception as error:
            report.update(passed=False, postcheck_error=repr(error))
        report['saved'] = {str(path.relative_to(output)): sha(path) for path in output.rglob('*') if path.is_file()}
        (output / 'observation.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
