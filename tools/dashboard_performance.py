"""Project-size context and historical measurements, never inferred speedups.

The archived benchmark schema has no imported-source inventory. Project SLOC
counts tracked .omg files below the subject directory at its recorded revision;
it is deliberately distinct from the compiler's full source closure.
"""
import hashlib
import importlib.util
import json
import math
from pathlib import Path, PurePosixPath
import re
import statistics
import subprocess
from datetime import datetime, timezone
from functools import lru_cache


def git(repository, *arguments, binary=False):
    result = subprocess.run(['git', '-C', str(repository), *arguments],
                            capture_output=True, timeout=30)
    if result.returncode:
        raise ValueError(result.stderr.decode('utf-8', errors='replace').strip() or 'Git object unavailable')
    return result.stdout if binary else result.stdout.decode('utf-8')


def source_lines(text):
    """Count code-bearing physical lines; comment markers in literals are code."""
    block, quote, escaped, code, count, index = 0, None, False, False, 0, 0
    while index < len(text):
        char, pair = text[index], text[index:index + 2]
        if char == '\n':
            count += code
            code = False
        elif block:
            if pair == '/*':
                block += 1
                index += 1
            elif pair == '*/':
                block -= 1
                index += 1
        elif quote:
            code |= not char.isspace()
            if escaped:
                escaped = False
            elif char == '\\':
                escaped = True
            elif char == quote:
                quote = None
        elif pair == '//':
            end = text.find('\n', index)
            index = len(text) if end < 0 else end - 1
        elif pair == '/*':
            block = 1
            index += 1
        elif char in ('"', "'"):
            quote, code = char, True
        else:
            code |= not char.isspace()
        index += 1
    return int(count + code)


@lru_cache(maxsize=256)
def project_inventory(repository, revision, root):
    path = PurePosixPath(root)
    if (not re.fullmatch(r'[0-9a-f]{40}', revision) or path.is_absolute()
            or '..' in path.parts or ':' in root or path.suffix != '.omg'
            or str(path.parent) == '.'):
        raise ValueError('No repository-relative project source identity')
    names = git(repository, 'ls-tree', '-r', '--name-only', revision, '--',
                str(path.parent) + '/').splitlines()
    names = sorted(name for name in names if name.endswith('.omg'))
    if root not in names:
        raise ValueError('Recorded root source is unavailable')
    files = []
    for name in names:
        content = git(repository, 'show', f'{revision}:{name}', binary=True)
        files.append({'path': name, 'sha256': hashlib.sha256(content).hexdigest(),
                      'sloc': source_lines(content.decode('utf-8-sig')), 'bytes': len(content)})
    digest = hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()
    return {'scope': 'tracked-project-omg', 'sloc': sum(f['sloc'] for f in files),
            'bytes': sum(f['bytes'] for f in files), 'sha256': digest, 'files': files}


def duration(metric):
    if metric.get('status') != 'measured':
        return None
    samples = metric.get('samples')
    if not isinstance(samples, list) or not samples or any(
            isinstance(n, bool) or not isinstance(n, (int, float))
            or not math.isfinite(n) or n <= 0 for n in samples):
        raise ValueError('Measured timings need finite positive samples')
    return {'median': statistics.median(samples), 'min': min(samples),
            'max': max(samples), 'samples': samples}


def flatten(record, identity, path, revision, project):
    metrics, host = record['metrics'], record['host']
    compile_time, runtime = duration(metrics['compile_time_ms']), duration(metrics['runtime_ms'])
    runtime_leg = metrics['runtime_ms']
    expected, exits = runtime_leg.get('exit_code_expected'), runtime_leg.get('exit_codes', [])
    oracle = (runtime is not None and type(expected) is int
              and runtime_leg.get('exit_code_match') is True
              and len(exits) == len(runtime['samples']) and all(code == expected for code in exits))
    context = {key: host.get(key) for key in ('os', 'machine', 'cpu', 'logical_cpus', 'rustc', 'omega_profile')}
    context['target'] = record['key']['target']
    method = record.get('measurement') or {}
    context['machine_id'] = method.get('machine_id')
    context['effective_profile'] = method.get('compiler_profile')
    context_id = hashlib.sha256(json.dumps(context, sort_keys=True).encode()).hexdigest()[:16]
    memory, size = metrics['peak_memory_bytes'], metrics['code_size_bytes']
    def nonnegative(value):
        return value if type(value) in (int, float) and math.isfinite(value) and value >= 0 else None
    return {'id': identity, 'path': path, 'record_revision': revision,
            'recorded': datetime.fromisoformat(record['recorded_utc'].replace('Z', '+00:00')).astimezone(timezone.utc).isoformat(), 'subject': record['subject'],
            'selection': record['key']['selection'], 'host': host, 'context': context,
            'context_id': context_id, 'project': project,
            'compile': compile_time, 'runtime': runtime, 'runtime_oracle': oracle,
            'expected_exit': expected, 'applicability': record.get('applicability'),
            'compile_reason': metrics['compile_time_ms'].get('reason'),
            'runtime_status': runtime_leg['status'], 'runtime_reason': runtime_leg.get('reason'),
            'compile_ms_kloc': compile_time['median'] * 1000 / project['sloc']
            if compile_time and (project.get('sloc') or 0) > 0 else None,
            'compile_memory': nonnegative(memory.get('compile_max_rss')) if memory['status'] == 'measured' else None,
            'run_memory': nonnegative(memory.get('run_max_rss')) if memory['status'] == 'measured' else None,
            'code_bytes': nonnegative(size.get('value')) if size['status'] == 'measured' else None,
            'code_stable': size.get('stable'), 'notes': record.get('notes', []),
            'observation_warnings': record.get('observation_warnings', []),
            'measurement': record.get('measurement'), 'baseline_ratio': None, 'baseline_id': None}


def pair_baselines(rows):
    """Only explicit comparable measurement contexts can produce median ratios."""
    groups = {}
    for row in sorted(rows, key=lambda r: (r['recorded'], r['id'])):
        method = row.get('measurement')
        profile = method.get('compiler_profile') if isinstance(method, dict) else None
        if (not row['runtime_oracle'] or not isinstance(method, dict)
                or method.get('source_clean') is not True
                or method.get('dependency_lock_stable') is not True
                or not re.fullmatch(r'[0-9a-f]{64}', str(method.get('input_sha256', '')))
                or not method.get('protocol') or not method.get('machine_id')
                or not re.fullmatch(r'[0-9a-f]{64}', str(method.get('dependency_lock_sha256', '')))
                or not isinstance(profile, dict) or profile.get('opt_level') not in ('1', '2', '3', 's', 'z')
                or not re.fullmatch(r'[0-9a-f]{64}', str(row['host'].get('omega_binary_sha256', '')))
                or not row['project'].get('sha256')
                or not all(row['context'].get(key) for key in ('os', 'machine', 'cpu'))):
            continue
        # A fixed repository revision also freezes imported tracked source.
        key = (row['context_id'], row['subject']['root'], row['subject']['source_revision'],
               method['input_sha256'], method['protocol'], method['machine_id'],
               method.get('work_units'), row['expected_exit'], json.dumps(profile, sort_keys=True))
        key += (method['dependency_lock_sha256'],)
        baseline = groups.setdefault(key, row)
        row['baseline_id'] = baseline['id']
        if baseline['id'] != row['id']:
            row['baseline_ratio'] = baseline['runtime']['median'] / row['runtime']['median']


def collect_performance(repository, warnings):
    directory = Path(repository) / 'tools/benchmark/records'
    spec = importlib.util.spec_from_file_location('dashboard_benchmark', Path(__file__).parent / 'benchmark/benchmark.py')
    benchmark = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(benchmark)
    candidates, errors, seen, inventories, rows = [], [], set(), {}, []
    for file in sorted(directory.glob('*.json')):
        try:
            path = file.relative_to(repository).as_posix()
            # A retained Git blob with identical JSON supplies the commit link.
            # A new/changed local record stays unbound, even during a write race.
            candidates.append((path, None, file.read_text(encoding='utf-8')))
        except (OSError, ValueError) as error:
            errors.append(f'{file.name}: {error}')
    try:
        commits = git(repository, 'log', '--format=%H', '--', 'tools/benchmark/records').splitlines()
        for commit in commits:
            paths = git(repository, 'diff-tree', '--root', '--no-commit-id', '--name-only', '--diff-filter=AM', '-r', commit,
                        '--', 'tools/benchmark/records').splitlines()
            for path in paths:
                if path.startswith('tools/benchmark/records/') and path.endswith('.json'):
                    try:
                        candidates.append((path, commit, git(repository, 'show', f'{commit}:{path}')))
                    except ValueError as error:
                        errors.append(f'{path}@{commit[:11]}: {error}')
    except (ValueError, subprocess.TimeoutExpired) as error:
        errors.append('Benchmark history unavailable: ' + str(error))
    for path, revision, text in sorted(candidates, key=lambda candidate: candidate[1] is None):
        try:
            record = json.loads(text)
            identity = hashlib.sha256(json.dumps(record, sort_keys=True).encode()).hexdigest()
            if identity in seen:
                continue
            seen.add(identity)
            problems = benchmark.validate_record(record, path)
            if problems:
                raise ValueError('; '.join(problems))
            if datetime.fromisoformat(record['recorded_utc'].replace('Z', '+00:00')).utcoffset() is None:
                raise ValueError('Benchmark timestamp needs a timezone')
            subject = record['subject']
            key = (subject['source_revision'], subject['root'])
            if key not in inventories:
                try:
                    inventories[key] = project_inventory(repository, *key)
                except (ValueError, subprocess.TimeoutExpired) as error:
                    inventories[key] = {'scope': 'tracked-project-omg', 'sloc': None, 'error': str(error)}
            project = inventories[key]
            if isinstance(record.get('measurement'), dict) and record['measurement'].get('source_clean') is False:
                project = {'scope': 'tracked-project-omg', 'sloc': None,
                           'error': 'Measured source was modified; recorded revision cannot bind LOC'}
            rows.append(flatten(record, identity, path, revision, project))
        except (ValueError, TypeError, KeyError, AttributeError, OverflowError) as error:
            errors.append(f'{path}@{revision[:11] if revision else "working-tree"}: {error}')
    pair_baselines(rows)
    if errors:
        warnings.append(f'{len(errors)} benchmark inputs could not be read; see Performance evidence')
    return {'records': sorted(rows, key=lambda r: (r['recorded'], r['id'])),
            'errors': errors, 'scope': 'tracked-project-omg'}
