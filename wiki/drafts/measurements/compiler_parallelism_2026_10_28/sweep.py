"""One fixed Windows Rayon sweep; run after build/parallelism-chain.py verifies the producer.

Reuse the repository's profile, timing and Windows job accounting helpers.
PeakJobMemoryUsed measures private committed job memory, not resident memory.
The experiment changes only RAYON_NUM_THREADS, in a fresh compiler process.
"""
import ctypes
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
COLD = '--cold' in sys.argv
EVIDENCE = ROOT / 'build/parallelism-first' / ('sweep-cold' if COLD else 'sweep')
sys.path.insert(0, str(ROOT / 'tools'))
from compiler_profile import require_optimized
from compiler_profile_images import executable_image

spec = importlib.util.spec_from_file_location('benchmark', ROOT / 'tools/benchmark/benchmark.py')
benchmark = importlib.util.module_from_spec(spec)
spec.loader.exec_module(benchmark)
WORKERS = [1, 2, 4, 8, 16, 32]
SUBJECTS = {
    'length': 'samples/cli/facts/length/local/main.omg',
    'dispatch': 'samples/cli/facts/dynamic_trait_dispatch/local/main.omg',
    'stack32': 'samples/cli/facts/thirtytwo_arg_stack_calling_plan/other_machine/main.omg',
}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def system_cpu():
    values = [ctypes.c_uint64() for _ in range(3)]
    function = ctypes.windll.kernel32.GetSystemTimes
    function.argtypes = [ctypes.c_void_p] * 3
    function.restype = ctypes.c_int
    if not function(*[ctypes.byref(value) for value in values]):
        return None
    idle, kernel, user = (value.value for value in values)
    return (kernel + user - idle) / 10000


def capture(command, label, environment):
    output_path = EVIDENCE / (label + '.log')
    if COLD and label != 'self-test':
        cache = EVIDENCE / (label + '-appdata')
        cache.mkdir()
        environment = dict(environment, LOCALAPPDATA=str(cache))
    busy_before = system_cpu()
    started = time.perf_counter()
    with output_path.open('wb') as output:
        process = subprocess.Popen(command, cwd=ROOT, env=environment,
                                   stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT)
        job = benchmark._windows_job_assign(process)
        try:
            process.wait(timeout=180)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            raise
        wall_ms = (time.perf_counter() - started) * 1000
        busy_after = system_cpu()
        times = [ctypes.c_uint64() for _ in range(4)]
        get_times = ctypes.windll.kernel32.GetProcessTimes
        get_times.argtypes = [ctypes.c_void_p] * 5
        get_times.restype = ctypes.c_int
        obtained = get_times(int(process._handle), *[ctypes.byref(value) for value in times])
        cpu = {'kernel_ms': times[2].value / 10000, 'user_ms': times[3].value / 10000} if obtained else None
        peak_commit = benchmark._windows_job_peak_bytes(job)
    return {'exit': process.returncode, 'wall_ms': wall_ms, 'root_cpu': cpu,
            'host_busy_cpu_ms': busy_after - busy_before if busy_after is not None and busy_before is not None else None,
            'peak_job_private_commit_bytes': peak_commit,
            'log': str(output_path.relative_to(ROOT))}, output_path.read_text(encoding='utf-8', errors='replace')


def compiled_artifact(observation, text, directory):
    if observation['exit'] != 0 or re.search(r'^error(?:\[|:|\s)', text, re.MULTILINE):
        raise ValueError('Compiler failed: ' + observation['log'])
    stages, published = benchmark.parse_timings(text)
    if not published:
        raise ValueError('No published native artifact: ' + observation['log'])
    artifact = Path(published)
    if not artifact.is_absolute():
        artifact = next((base / artifact for base in (directory, ROOT) if (base / artifact).is_file()), artifact)
    if not artifact.is_file() or not executable_image(artifact.read_bytes()):
        raise ValueError('Missing or malformed artifact: ' + str(artifact))
    observation.update(stages_ms=stages, artifact=str(artifact),
                       artifact_sha256=digest(artifact), artifact_bytes=artifact.stat().st_size)
    return artifact


def self_test():
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    observation, text = capture([sys.executable, '-c',
        'import time; a=bytearray(32*1024*1024); print("captured"); time.sleep(0.1); raise SystemExit(7)'],
        'self-test', dict(os.environ))
    assert observation['exit'] == 7 and 'captured' in text
    assert observation['root_cpu'] is not None and observation['peak_job_private_commit_bytes'] > 32*1024*1024
    assert observation['host_busy_cpu_ms'] is not None and observation['host_busy_cpu_ms'] >= 0
    for broken in (observation, dict(observation, exit=0)):
        try:
            compiled_artifact(broken, text, EVIDENCE)
        except ValueError:
            pass
        else:
            raise AssertionError('Failed compiler or missing artifact was accepted')
    assert benchmark.parse_timings('  12.5 ms  stage\npublished native output to x.exe')[0] == {'stage': 12.5}
    valid = dict(exit=0, log='self-test')
    artifact = compiled_artifact(valid, 'published native output to ' + sys.executable, EVIDENCE)
    assert artifact == Path(sys.executable) and valid['artifact_sha256'] == digest(sys.executable)
    print('Measurement self-test passed: nonzero exit, missing artifact, CPU and committed-memory accounting', flush=True)


def main(binary):
    assert sys.platform == 'win32', 'This archived experiment accounts Windows processes only'
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    profile = require_optimized(binary)
    producer = json.loads((ROOT / 'build/parallelism-first/producer.json').read_text(encoding='utf-8'))
    assert producer['profile']['sha256'] == profile['sha256']
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    assert producer['revision'] == revision
    subprocess.run(['git', 'diff', '--exit-code', '--', 'omega-rust', 'source/library', 'Cargo.toml', 'Cargo.lock'], cwd=ROOT, check=True)
    environment = dict(os.environ, OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'))
    environment.pop('OMEGA_PHASE_PROFILE', None)
    records = {'revision': revision, 'producer': profile, 'subjects': SUBJECTS, 'workers': WORKERS,
               'validation': [], 'warmup': [], 'samples': [],
               'cache_regime': 'empty private compiler cache per invocation; warm OS caches; pool startup included' if COLD else 'fresh compiler process and fresh output directory; shared OS/package caches warmed; pool startup included',
               'cpu_scope': 'root compiler process only; job commit includes descendants',
               'unmeasured': ['peak RSS', 'stack virtual reservation', 'sampled thread count', 'batch throughput']}
    fingerprints = {}
    for subject, relative in SUBJECTS.items():
        source = ROOT / relative
        fingerprints[relative] = digest(source)
        for companion in ('build.omg', 'expected_native_exit.txt'):
            path = source.parent / companion
            if path.is_file():
                fingerprints[str(path.relative_to(ROOT))] = digest(path)
        for workers in WORKERS:
            environment['RAYON_NUM_THREADS'] = str(workers)
            label = f'validate-{subject}-{workers}'
            observation, text = capture([binary, 'run', '--both', str(source)], label, environment)
            observation.update(subject=subject, workers=workers, kind='both')
            records['validation'].append(observation)
            (EVIDENCE / 'results.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')
            if observation['exit'] != 0 or 'interp exit: 0' not in text or 'native exit 0' not in text:
                raise ValueError('Terminal/native validation did not agree: ' + observation['log'])
            invalid = source.parent / 'invalid/main.omg'
            fingerprints[str(invalid.relative_to(ROOT))] = digest(invalid)
            observation, text = capture([binary, '--check', str(invalid)], label + '-reject', environment)
            observation.update(subject=subject, workers=workers, kind='reject')
            observation['diagnostic_sha256'] = hashlib.sha256(text.encode('utf-8')).hexdigest()
            prior = next((row for row in records['validation'] if row['subject'] == subject and row['kind'] == 'reject'), None)
            if prior and observation['diagnostic_sha256'] != prior['diagnostic_sha256']:
                raise ValueError('Reject diagnostic changed with worker count: ' + observation['log'])
            records['validation'].append(observation)
            if observation['exit'] != 1 or not text.strip():
                raise ValueError('Invalid control did not reject: ' + observation['log'])
            print(f'Validated {subject}, workers={workers}: both engines exit 0; invalid rejects', flush=True)
    records['source_sha256'] = fingerprints
    for round_number in range(4 if COLD else 6):
        order = WORKERS if round_number % 2 == 0 else list(reversed(WORKERS))
        for workers in order:
            environment['RAYON_NUM_THREADS'] = str(workers)
            for subject, relative in SUBJECTS.items():
                label = f'round-{round_number}-{subject}-{workers}'
                directory = EVIDENCE / label
                directory.mkdir()
                observation, text = capture([binary, '--timings', '--build-dir', str(directory),
                                             '--target', 'windows_x86_64', str(ROOT / relative)], label, environment)
                artifact = compiled_artifact(observation, text, directory)
                observation.update(subject=subject, workers=workers, round=round_number)
                prior = next((row for row in records['samples'] + records['warmup'] if row['subject'] == subject), None)
                if prior and observation['artifact_sha256'] != prior['artifact_sha256']:
                    raise ValueError('Native bytes changed with worker count: ' + observation['log'])
                runtime = subprocess.run([str(artifact)], stdin=subprocess.DEVNULL, capture_output=True, timeout=20)
                observation['native_exit'] = runtime.returncode
                if runtime.returncode != 0:
                    raise ValueError('Native oracle failed: ' + observation['log'])
                records['warmup' if round_number == 0 else 'samples'].append(observation)
                (EVIDENCE / 'results.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')
                print(f'{label}: {observation["wall_ms"]:.1f} ms; native exit 0', flush=True)
    assert require_optimized(binary)['sha256'] == profile['sha256']
    assert all(digest(ROOT / path) == value for path, value in fingerprints.items())
    for subject in SUBJECTS:
        for workers in WORKERS:
            rows = [row for row in records['samples'] if row['subject'] == subject and row['workers'] == workers]
            print(subject, workers, 'median', round(statistics.median(row['wall_ms'] for row in rows), 1), flush=True)


if __name__ == '__main__':
    self_test() if sys.argv[-1] == '--self-test' else main(str(Path(sys.argv[-1]).resolve()))
