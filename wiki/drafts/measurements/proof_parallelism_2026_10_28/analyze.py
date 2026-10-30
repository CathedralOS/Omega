"""Summarize observer intervals without adding overlapping machine durations."""
import collections, gzip, json, pathlib, sys

def occupancy(start, end, intervals):
    events = collections.Counter({start: 0, end: 0})
    for left, right in intervals:
        assert start <= left <= right <= end
        events[left] += 1
        events[right] -= 1
    active = maximum = 0
    previous = start
    buckets = [0, 0, 0]
    for instant, change in sorted(events.items()):
        buckets[min(active, 2)] += instant - previous
        active += change
        maximum = max(maximum, active)
        previous = instant
    assert active == 0 and sum(buckets) == end - start
    return dict(idle_ns=buckets[0], one_active_ns=buckets[1], overlapping_ns=buckets[2],
                maximum_active=maximum, summed_machine_ns=sum(right-left for left,right in intervals))

assert occupancy(0, 12, [(0, 10), (2, 6)]) == dict(
    idle_ns=2, one_active_ns=6, overlapping_ns=4, maximum_active=2, summed_machine_ns=14)
assert occupancy(0, 5, [(0, 2), (2, 5)])['one_active_ns'] == 5

def summarize(folder):
    result = json.loads((folder / 'result.json').read_text(encoding='utf-8'))
    jobs = []
    for path in sorted((folder / 'events').glob('check-*.tsv*')):
        contents = gzip.decompress(path.read_bytes()).decode('utf-8') if path.suffix == '.gz' else path.read_text(encoding='utf-8')
        lines = [line.split('\t') for line in contents.splitlines()]
        header, *rows = lines
        assert header[0] == 'J'
        identity, mode, total, start, end, merge, workers = header[1:]
        total, start, end, merge, workers = map(int, (total, start, end, merge, workers))
        machines = [row for row in rows if row[0] == 'M']
        expressions = [row for row in rows if row[0] == 'E']
        visits = sum(int(row[6]) for row in machines)
        contexts = sum(int(row[8]) for row in machines)
        distinct = sum(int(row[7]) for row in machines)
        assert visits == sum(int(row[3]) for row in expressions)
        assert contexts == sum(int(row[4]) for row in expressions)
        names = {row[1]: row[2] for row in machines}
        jobs.append(dict(identity=identity, mode=mode, validation_ns=total,
            pre_batch_ns=start, batch_ns=end-start, merge_ns=merge-end,
            post_merge_ns=total-merge, workers=workers, machine_count=len(machines),
            **occupancy(start, end, [(int(row[3]), int(row[4])) for row in machines]),
            expression_visits=visits, distinct_expression_body_pairs=distinct,
            distinct_context_pairs=contexts, repeated_context_visits=visits-contexts,
            longest_machines=[dict(name=row[2], wall_ns=int(row[4])-int(row[3]))
                for row in sorted(machines, key=lambda row:int(row[4])-int(row[3]), reverse=True)[:8]],
            most_repeated=[dict(machine=names[row[1]], expression=row[2], visits=int(row[3]),
                contexts=int(row[4]), repeats=int(row[5]), description=row[6])
                for row in sorted(expressions, key=lambda row:int(row[5]), reverse=True)[:8]]))
    result['checking_jobs'] = jobs
    return result

if __name__ == '__main__':
    root = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).resolve().parent
    results = {folder.name:summarize(folder) for folder in root.iterdir()
               if folder.is_dir() and (folder / 'result.json').exists()
               and 'compiler_sha256' in json.loads((folder / 'result.json').read_text())}
    (root / 'summary.json').write_text(json.dumps(results, indent=2), encoding='utf-8')
    for name, result in results.items():
        print(name, round(result['wall_seconds'], 4), 'seconds',
              [(job['machine_count'], round(job['batch_ns']/1e9,4), job['expression_visits'],
                job['repeated_context_visits']) for job in result['checking_jobs']])
