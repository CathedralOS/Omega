"""LLDB-owned child launch and bounded function/source/PC observations only."""
import hashlib
import json
import os
from pathlib import Path
import time
import lldb

TAG = 'installed-entry-early-callers-v1'


def emit(kind, **fields):
    print(json.dumps(dict(protocol=TAG, kind=kind, **fields)), flush=True)


def checked_command(debugger, command):
    result = lldb.SBCommandReturnObject()
    debugger.GetCommandInterpreter().HandleCommand(command, result)
    if not result.Succeeded():
        raise RuntimeError('LLDB setting refused: ' + command + ': ' + (result.GetError() or ''))


def process_stat(pid):
    tail = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    return dict(start_ticks=int(tail[19]), ppid=int(tail[1]),
                session=int(tail[3]), pgrp=int(tail[2]),
                utime_ticks=int(tail[11]), stime_ticks=int(tail[12]), rss_pages=int(tail[21]))


def resume(process):
    previous_stop = process.GetStopID()
    assert process.Continue().Success()
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        state = process.GetState()
        if state == lldb.eStateRunning:
            return
        assert state == lldb.eStateStopped and process.GetStopID() == previous_stop, 'Unexpected resume transition'
        time.sleep(.01)
    raise RuntimeError('No running acknowledgement after Continue')


def run(debugger):
    root = Path.cwd()
    config = json.loads((root / 'build/installed-entry-linux_arm64/early-callers-runtime.json').read_text())
    recipe = config['recipe']
    process = None
    began = None
    errors = []
    try:
        # These are debugger-local controls, not host permission or ASLR policy.
        for command in ('settings set target.disable-aslr false',
                        'settings set target.load-script-from-symbol-file false',
                        'settings set target.load-cwd-lldbinit false',
                        'settings set symbols.enable-external-lookup false',
                        'settings set frame-format "frame #${frame.index}: ${frame.pc} ${function.name-without-args}"',
                        'settings set thread-format "thread ${thread.id}"',
                        'settings set thread-stop-format "thread ${thread.id}: ${thread.stop-reason}"',
                        'settings set stop-line-count-before 0',
                        'settings set stop-line-count-after 0'):
            checked_command(debugger, command)
        debugger.SetAsync(True)
        target = debugger.CreateTarget(config['runner'])
        assert target.IsValid() and target.GetNumModules() > 0
        assert target.GetModuleAtIndex(0).GetNumSymbols() > 0, 'No local executable symbols'
        assert Path(target.GetExecutable().fullpath).resolve() == Path(config['runner']).resolve()
        error = lldb.SBError()
        # Explicit flags0 preserve normal ASLR. No argument, no inherited
        # environment, no terminal, no attach, no inferior expression evaluation.
        process = target.Launch(
            debugger.GetListener(), None, config['inferior_environment'], '/dev/null',
            config['inferior_stdout'], config['inferior_stderr'], str(root), 0, True, error)
        if not error.Success() or not process.IsValid():
            raise RuntimeError('Normal-user child launch refused: ' + str(error))
        deadline = time.monotonic() + 5
        while process.GetState() not in (lldb.eStateStopped, lldb.eStateExited, lldb.eStateCrashed) and time.monotonic() < deadline:
            time.sleep(.02)
        assert process.GetState() == lldb.eStateStopped, 'No initial entry stop'
        pid = process.GetProcessID()
        stat = process_stat(pid)
        actual = Path(os.readlink(f'/proc/{pid}/exe')).resolve()
        assert actual == Path(config['runner']).resolve()
        assert hashlib.sha256(actual.read_bytes()).hexdigest() == recipe['runner_sha256']
        emit('inferior_identity', pid=pid, executable=str(actual), **stat,
             observer=config['observer'], compiler_source=recipe['compiler_source'])
        resume(process)
        began = time.monotonic()
        emit('initial_resume', pid=pid, elapsed=0)
        for scheduled in (15, 25, 35):
            while time.monotonic() - began < scheduled:
                if process.GetState() != lldb.eStateRunning:
                    raise RuntimeError('Uncommanded inferior stop/exit before scheduled snapshot')
                time.sleep(min(.05, max(0, scheduled - (time.monotonic() - began))))
            root_rows = [line.split('fixture-root-v1 ', 1)[1] for line in Path(config['inferior_stderr']).read_text().splitlines() if line.startswith('corpus_runner: fixture-root-v1 ')]
            assert len(root_rows) == 1 and bytes.fromhex(root_rows[0]).decode() == str(root), 'Missing or conflicting source root acknowledgement'
            emit('source_root', source_root=str(root), scheduled=scheduled)
            # The callback captures the debugger-owned process only. The actual
            # stop and sampling duration are retained; no timing is projected.
            assert process.GetState() == lldb.eStateRunning
            assert process.Stop().Success()
            deadline = time.monotonic() + 3
            while process.GetState() == lldb.eStateRunning and time.monotonic() < deadline:
                time.sleep(.01)
            assert process.GetState() == lldb.eStateStopped
            count = process.GetNumThreads()
            assert 0 < count <= 64, 'Thread roster exceeds the bounded snapshot'
            threads = []
            for index in range(count):
                thread = process.GetThreadAtIndex(index)
                reason = thread.GetStopReason()
                data = [thread.GetStopReasonDataAtIndex(i) for i in range(thread.GetStopReasonDataCount())]
                if reason not in (lldb.eStopReasonNone, lldb.eStopReasonInvalid):
                    assert reason == lldb.eStopReasonSignal and data and data[0] in (2, 19), 'Unexpected stop cause'
                frames = []
                # Do not ask GetNumFrames to unwind an arbitrarily deep stack.
                for ordinal in range(32):
                    frame = thread.GetFrameAtIndex(ordinal)
                    if not frame.IsValid():
                        break
                    line = frame.GetLineEntry()
                    frames.append(dict(ordinal=ordinal, pc=frame.GetPC(),
                                       function=frame.GetFunctionName(),
                                       source=(line.GetFileSpec().fullpath if line.IsValid() else None),
                                       line=(line.GetLine() if line.IsValid() else None)))
                threads.append(dict(tid=thread.GetThreadID(), name=thread.GetName(),
                                    stop_reason=reason, stop_data=data, frames=frames))
            emit('snapshot', scheduled=scheduled, elapsed=round(time.monotonic() - began, 3),
                 pid=pid, resources=process_stat(pid), threads=threads,
                 meaning='Active frames only; no allocation, CPU-share, OOM or qualification inference.')
            if scheduled < 35:
                resume(process)
        emit('samples_complete', elapsed=round(time.monotonic() - began, 3))
    except BaseException as error:
        errors.append(dict(type=type(error).__name__, message=str(error)))
        emit('inner_error', error=errors[-1], elapsed=None if began is None else round(time.monotonic() - began, 3))
    finally:
        if process is not None and process.IsValid():
            result = process.Kill()
            deadline = time.monotonic() + 3
            while process.GetState() != lldb.eStateExited and time.monotonic() < deadline:
                time.sleep(.02)
            emit('inner_cleanup', success=result.Success(), state=process.GetState(),
                 exit_status=process.GetExitStatus(), elapsed=None if began is None else round(time.monotonic() - began, 3))
            if not result.Success() or process.GetState() != lldb.eStateExited:
                errors.append(dict(type='CleanupFailure', message=str(result)))
        emit('inner_final', failed=bool(errors), errors=errors, diagnostic_only=True)
