"""Disposable Linux observer: exact seed, wall/CPU, and read-only Gamma cursor."""
import ctypes
import datetime
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import threading
import time
from measure_heap import HeapPhases

LIBC = ctypes.CDLL(None, use_errno=True)
LIBC.ptrace.restype = ctypes.c_long
ARENA = 0x20400000

def trace(request, pid=0, address=0, data=0):
    result = LIBC.ptrace(ctypes.c_uint(request), ctypes.c_uint(pid),
                         ctypes.c_void_p(address), ctypes.c_void_p(data))
    if result == -1:
        raise OSError(ctypes.get_errno(), 'ptrace')
    return result

def identity(content):
    return {'bytes': len(content), 'sha256': hashlib.sha256(content).hexdigest()}

def cursor(pid):
    with open(f'/proc/{pid}/mem', 'rb', buffering=0) as memory:
        memory.seek(0x402500)
        current, limit = struct.unpack('<QQ', memory.read(16))
    if not ARENA <= current <= limit == 0x2000000000:
        raise ValueError(f'unexpected Gamma cursor/limit: {current:x}/{limit:x}')
    return {'heap_cursor': current, 'heap_limit': limit,
            'cumulative_pairs': (current - ARENA) // 40}

def observe(directory, label, request, timeout=14400, phase_stdin=None, cursor_points=None, output_points=None):
    directory = Path(directory)
    if (directory / f'{label}.json').exists():
        raise RuntimeError(f"{label}: observation exists; use a new directory")
    (directory / f'{label}.input').write_bytes(request)
    started = time.monotonic()
    observer_cpu_start = time.process_time()
    phases = None
    started_utc = datetime.datetime.now(datetime.timezone.utc).isoformat()
    print(f'{label}: start', flush=True)
    with subprocess.Popen([str(directory / 'evaluator.exe')],
                          stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE,
                          preexec_fn=lambda: trace(0)) as process:
        _, status, _ = os.wait4(process.pid, os.WUNTRACED)
        if not os.WIFSTOPPED(status):
            raise RuntimeError('child did not stop at exec')
        trace(0x4200, process.pid, 0, 0x40)  # SETOPTIONS, TRACEEXIT
        trace(7, process.pid)  # CONT
        captured = {}
        def write_input():
            try:
                process.stdin.write(request)
            except BrokenPipeError:
                pass
            finally:
                process.stdin.close()
        def read_output(name, stream):
            captured[name] = stream.read()
        workers = [threading.Thread(target=write_input),
                   threading.Thread(target=read_output, args=('output', process.stdout)),
                   threading.Thread(target=read_output, args=('stderr', process.stderr))]
        for worker in workers:
            worker.start()
        phases = HeapPhases(process.pid, phase_stdin, cursor_points or {}, output_points or {}, started) if phase_stdin is not None else None
        last_scan = 0.0
        last_report = started
        final_cursor = None
        timed_out = False
        exiting = False
        while True:
            waited, status, usage = os.wait4(process.pid, os.WNOHANG | os.WUNTRACED)
            if waited:
                if os.WIFEXITED(status) or os.WIFSIGNALED(status):
                    process.returncode = os.waitstatus_to_exitcode(status)
                    break
                if os.WIFSTOPPED(status):
                    stop_signal = os.WSTOPSIG(status)
                    if status >> 16 == 6:  # exit event, memory still readable
                        if phases is not None:
                            phases.scan()
                        final_cursor = cursor(process.pid)
                        exiting = True
                        trace(7, process.pid)
                    else:
                        trace(7, process.pid, 0, stop_signal)
            now = time.monotonic()
            if not exiting and phases is not None and now-last_scan >= 0.1:
                phases.scan()
                last_scan = now
            if now - started > timeout and not timed_out:
                timed_out = True
                process.kill()
            if not exiting and now - last_report >= 60:
                progress = {'elapsed_seconds': now-started, **cursor(process.pid)}
                if phases is not None:
                    progress.update(phases.result())
                    (directory / f'{label}.progress.json').write_text(json.dumps(progress, indent=2)+'\n')
                print(f'{label}: {progress["elapsed_seconds"]:.1f}s, pairs={progress["cumulative_pairs"]}, input_cursor={phases.last_input_cursor if phases else None}, last_phase={phases.events[-1]["name"] if phases and phases.events else None}', flush=True)
                last_report = now
            time.sleep(0.02)
        for worker in workers:
            worker.join()
    (directory / f'{label}.output').write_bytes(captured['output'])
    (directory / f'{label}.stderr').write_bytes(captured['stderr'])
    projection_valid = None
    if phases is not None:
        projection_valid = (captured['output'][:5] == bytes(5)
                            and bytes(phases.output) == captured['output'][5:]
                            and phases.last_input_cursor == len(phase_stdin))
        projection = phases.result()
        projection['validated_against_public_output'] = projection_valid
        (directory / f'{label}.phases.json').write_text(json.dumps(projection, indent=2)+'\n')
    record = {'label': label, 'status': process.returncode,
              'started_utc': started_utc,
              'observer': identity(Path(__file__).read_bytes()),
              'heap_projection': identity(Path(__file__).with_name('measure_heap.py').read_bytes()),
              'transport': 'PIPE stdin/stdout/stderr',
              'phase_projection_valid': projection_valid,
              'wall_seconds': time.monotonic() - started,
              'observer_cpu_seconds': time.process_time() - observer_cpu_start,
              'user_seconds': usage.ru_utime, 'system_seconds': usage.ru_stime,
              'max_rss_kib': usage.ru_maxrss, 'timed_out': timed_out,
              'input': identity(request), **(final_cursor or {})}
    content = (directory / f'{label}.output').read_bytes()
    record['output'] = identity(content)
    record['output_prefix'] = content[:64].hex()
    record['stderr'] = (directory / f'{label}.stderr').read_text()
    (directory / f'{label}.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record), flush=True)
    if record['timed_out'] or record['status'] != 0 or record['stderr']:
        raise RuntimeError(f'{label}: execution did not complete cleanly')
    if phases is not None and not projection_valid:
        raise RuntimeError(f'{label}: heap projection not validated by public output')
    return content
