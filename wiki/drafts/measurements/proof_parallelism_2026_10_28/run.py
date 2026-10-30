"""Fixed-workload compiler measurements; all child failures remain failures."""
import argparse, ctypes, hashlib, json, os, pathlib, subprocess, time

ROOT = next(parent for parent in pathlib.Path(__file__).resolve().parents
            if (parent / 'Cargo.toml').is_file() and (parent / 'AGENTS.md').is_file())
OUT = ROOT / 'build/proof-profile'
PROGRAM = ROOT / 'samples/cli/games/dungeon_crawler_cli/main.omg'
ORACLE = b'A bottomless dark room near the dungeon heart.'

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def verify_inputs():
    manifest = json.loads((OUT / 'input-manifest.json').read_text())['inputs']
    assert all(digest(ROOT / path) == expected for path, expected in manifest.items()
               if not path.endswith('/omega.lock')), 'input drift'

def invoke(command, folder, environment, timeout=600):
    folder.mkdir(parents=True, exist_ok=False)
    started = time.perf_counter()
    process = subprocess.Popen([str(item) for item in command], cwd=ROOT, env=environment,
                               stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        process.kill()
        process.communicate()
        raise
    result = subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
    elapsed = time.perf_counter() - started
    created, exited, kernel, user = (ctypes.c_uint64() for _ in range(4))
    assert ctypes.windll.kernel32.GetProcessTimes(ctypes.c_void_p(int(process._handle)),
        ctypes.byref(created), ctypes.byref(exited), ctypes.byref(kernel), ctypes.byref(user))
    (folder / 'stdout.txt').write_bytes(result.stdout)
    (folder / 'stderr.txt').write_bytes(result.stderr)
    record = dict(command=[str(item) for item in command], wall_seconds=elapsed,
                  exit_code=result.returncode, cpu_seconds=(kernel.value+user.value)/1e7)
    (folder / 'result.json').write_text(json.dumps(record, indent=2), encoding='utf-8')
    print(json.dumps(record), flush=True)
    if result.returncode != 0:
        raise RuntimeError(result.stderr.decode('utf-8', 'replace')[-6000:])
    return result, record

def measure(producer, label, mode, workers, comparison=False, phase_profile=False, cold_cache=False):
    executable = OUT / 'producers' / producer / 'omega.exe'
    accepted_lock = (OUT / 'producers' / producer / 'omega.lock').read_bytes()
    (PROGRAM.parent / 'omega.lock').write_bytes(accepted_lock)
    environment = os.environ.copy()
    environment.update(OMEGA_PHASE_PROFILE='1' if phase_profile else '0', OMEGA_CHECK_MEASUREMENT=mode,
                       OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'),
                       OMEGA_CHECK_MEASUREMENT_WORKERS=str(workers),
                       OMEGA_CHECK_MEASUREMENT_DIR=str(OUT / label / 'events'))
    if cold_cache:
        cache = OUT / 'private-cache' / label
        cache.mkdir(parents=True, exist_ok=False)
        environment['LOCALAPPDATA'] = str(cache)
    if comparison:
        result, record = invoke([executable, 'run', '--both', '--keep', PROGRAM], OUT / label, environment)
        assert ORACLE in result.stdout, 'missing authored runtime output'
        assert b'DECLINED' not in result.stdout + result.stderr, 'comparison declined'
        assert b'interp exit: 0' in result.stderr, 'comparison did not execute'
    else:
        result, record = invoke([executable, '--timings', '--target', 'windows_x86_64',
                                 '--build-dir', OUT / label / 'native', PROGRAM], OUT / label, environment)
        images = list((OUT / label / 'native').rglob('*.exe'))
        assert len(images) == 1, images
        observed, runtime = invoke([images[0]], OUT / label / 'runtime', environment, 30)
        assert ORACLE in observed.stdout, 'missing authored runtime output'
        record['image_sha256'] = digest(images[0])
        record['runtime_wall_seconds'] = runtime['wall_seconds']
    record.update(producer=producer, compiler_sha256=digest(executable), mode=mode,
                  workers=workers, phase_profile=phase_profile, program_sha256=digest(PROGRAM),
                  accepted_lock_sha256=hashlib.sha256(accepted_lock).hexdigest())
    record['cache_mode'] = 'fresh private cache' if cold_cache else 'shared user cache'
    if mode != 'off':
        assert list((OUT / label / 'events').glob('check-*.tsv')), 'missing observer output'
        assert b'checking_measurement: output failed' not in result.stderr
    (OUT / label / 'result.json').write_text(json.dumps(record, indent=2), encoding='utf-8')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('producer')
    parser.add_argument('label')
    parser.add_argument('--mode', default='off', choices=['off', 'timing', 'counts'])
    parser.add_argument('--workers', type=int, default=0)
    parser.add_argument('--comparison', action='store_true')
    parser.add_argument('--phase-profile', action='store_true')
    parser.add_argument('--cold-cache', action='store_true')
    options = parser.parse_args()
    measure(options.producer, options.label, options.mode, options.workers, options.comparison,
            options.phase_profile, options.cold_cache)
