"""Capture a fixed native workload with existing benchmark measurement primitives.

Run from the measured checkout, after building and freezing both producers.
Raw process output and nested probe events are retained; instrumented durations
are diagnostic attribution, never an uninstrumented speedup estimate.
"""
from collections import defaultdict
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
try:
    import resource
except ImportError:
    resource = None

sys.path.insert(0, str(Path('tools/benchmark').resolve()))
from benchmark import capture_compile, parse_timings, require_optimized, run_measured

output = Path('build/resolution-measurement').resolve()
subjects = [
    'samples/cli/facts/length/local/main.omg',
    'samples/cli/facts/dyn_dispatch_selector_payload_bound/local/main.omg',
    'samples/cli/facts/nested_nine_level_record_field_bound/local/main.omg',
]

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def traces(text):
    stacks = defaultdict(list)
    events = []
    for line in text.splitlines():
        if not line.startswith('RESMEASURE\t'):
            continue
        _, thread, kind, label, value = line.split('\t', 4)
        stack = stacks[thread]
        if kind == 'begin':
            fields = {key: int(number) for key, number in
                      (pair.split('=') for pair in value.split())}
            stack.append(dict(label=label, fields=fields, child_ns=0,
                              parents=[entry['label'] for entry in stack]))
        else:
            assert kind == 'end' and stack and stack[-1]['label'] == label, line
            entry = stack.pop()
            entry['elapsed_ns'] = int(value)
            entry['exclusive_ns'] = int(value) - entry.pop('child_ns')
            assert entry['exclusive_ns'] >= 0, entry
            entry['thread'] = thread
            events.append(entry)
            if stack:
                stack[-1]['child_ns'] += int(value)
    assert all(not stack for stack in stacks.values()), stacks
    summary = {}
    for entry in events:
        # Keep pre-resolution work distinct from the final/deferred resolver.
        domain = 'pre_resolution' if 'pre_resolution' in entry['parents'] else 'later'
        key = domain + '/' + entry['label']
        row = summary.setdefault(key, dict(calls=0, elapsed_ns=0, exclusive_ns=0, fields={}))
        row['calls'] += 1
        for metric in ('elapsed_ns', 'exclusive_ns'):
            row[metric] += entry[metric]
        for field, number in entry['fields'].items():
            row['fields'][field] = row['fields'].get(field, 0) + number
    return dict(events=events, summary=summary)

def main():
    producers = {name: output / ('omega-' + name) for name in ('baseline', 'instrumented')}
    for binary in producers.values():
        require_optimized(binary)
    assert not subprocess.check_output(['git','diff','HEAD','--','omega-rust','source/library','samples'])
    input_paths = sorted({path for subject in subjects for path in Path(subject).parent.rglob('*.omg')})
    inputs = {str(path): digest(path) for path in input_paths}
    metadata = dict(
        started_utc=datetime.now(timezone.utc).isoformat(),
        base=(output/'base.txt').read_text().strip(),
        library_tree=subprocess.check_output(['git','rev-parse','HEAD:source/library'], text=True).strip(),
        rustc=subprocess.check_output(['rustc','--version'], text=True).strip(),
        host=platform.platform(), inputs=inputs,
        instrumentation_sha256=digest(output/'instrumentation.patch'),
        producers={name: dict(path=str(binary), sha256=digest(binary),
            profile=json.loads(subprocess.check_output([str(binary),'--build-profile'],text=True)))
            for name,binary in producers.items()},
    )
    records = []
    def save():
        (output/'observations.json').write_text(json.dumps(dict(metadata=metadata,records=records),indent=2)+'\n')
    for round_number in range(2):
        # Heavy shared-host contention: repeat counts, not a timing campaign.
        order = ([('baseline', False), ('instrumented', False), ('instrumented', True)]
                 if round_number == 0 else [('instrumented', True)])
        for subject in subjects:
            for producer, enabled in order:
                with tempfile.TemporaryDirectory(prefix='omega-resolution-measure-') as temporary:
                    command = [str(producers[producer]), '--timings', '--target', 'macos_arm64',
                               '--build-dir', temporary, str(Path(subject).resolve())]
                    before = os.environ.get('OMEGA_RESOLUTION_MEASURE')
                    if enabled:
                        os.environ['OMEGA_RESOLUTION_MEASURE'] = '1'
                    else:
                        os.environ.pop('OMEGA_RESOLUTION_MEASURE', None)
                    before_cpu = resource.getrusage(resource.RUSAGE_CHILDREN) if resource else None
                    try:
                        code, wall_ms, rss, text = capture_compile(command, Path.cwd())
                    finally:
                        if before is None:
                            os.environ.pop('OMEGA_RESOLUTION_MEASURE', None)
                        else:
                            os.environ['OMEGA_RESOLUTION_MEASURE'] = before
                    after_cpu = resource.getrusage(resource.RUSAGE_CHILDREN) if resource else None
                    stages, published = parse_timings(text)
                    row = dict(subject=subject, producer=producer, traced=enabled,
                        round=round_number, warmup=False, command=command,
                        exit=code, wall_ms=wall_ms, peak_rss_bytes=rss, timings_ms=stages,
                        user_cpu_ms=1000*(after_cpu.ru_utime-before_cpu.ru_utime) if resource else None,
                        system_cpu_ms=1000*(after_cpu.ru_stime-before_cpu.ru_stime) if resource else None,
                        load_average=os.getloadavg() if hasattr(os,'getloadavg') else None, output=text)
                    records.append(row)
                    save()
                    assert code == 0 and published, text
                    artifact = Path(published)
                    row.update(native_sha256=digest(artifact), native_bytes=artifact.stat().st_size)
                    native_exit, _, _ = run_measured([str(artifact)], Path.cwd())
                    row['native_exit'] = native_exit
                    if enabled:
                        row['trace'] = traces(text)
                        labels = {event['label'] for event in row['trace']['events']}
                        assert {'driver.prepare', 'pre_resolution', 'resolver.lower_items',
                                'resolved.expressions.clone'} <= labels, labels
                    save()
                    assert native_exit == 0, row
                print(subject, producer, enabled, round_number, round(wall_ms, 3), flush=True)
    for subject in subjects:
        rows = [row for row in records if row['subject']==subject]
        assert len({row['native_sha256'] for row in rows}) == 1, subject
        traced = [row['trace']['summary'] for row in rows if row['traced']]
        counts = [{label:(entry['calls'],entry['fields']) for label,entry in trace.items()} for trace in traced]
        assert len(counts) == 2 and counts[0] == counts[1], subject
    assert inputs == {str(path):digest(path) for path in input_paths}, 'workload changed'
    assert all(metadata['producers'][name]['sha256']==digest(binary) for name,binary in producers.items())
    metadata['completed_utc'] = datetime.now(timezone.utc).isoformat()
    metadata['checks'] = dict(native_images_identical=True, native_exits_zero=True,
                              trace_counts_repeat=True, inputs_unchanged=True, producers_unchanged=True)
    save()

if __name__ == '__main__':
    main()
