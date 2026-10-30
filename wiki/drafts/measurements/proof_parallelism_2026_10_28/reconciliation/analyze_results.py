"""Summarize disjoint reconciliation owners, retaining all timing observations."""
import json, pathlib, statistics, sys

def summarize(root):
    runs = {}
    for path in sorted(root.glob('recon-*/result.json')):
        result = json.loads(path.read_text())
        if 'reconciliation' not in result:
            continue
        for row in result['reconciliation']:
            assert sum(value for key,value in row['metrics'].items() if key.endswith('_ns')) <= row['wall_ns']
            metrics = row['metrics']
            if row['operation']=='memberships' and 'candidate_machines' in metrics:
                assert metrics['fixed_point_rounds']==metrics['candidate_machines']+metrics.get('changing_rounds',0)
                assert metrics['reference_context_constructions']==metrics['fixed_point_rounds']
                assert metrics['reference_queries']==metrics['eligible_operations']
        runs[path.parent.name] = result
    timings = [result for result in runs.values() if result['reconciliation_mode']=='timing']
    totals = {name:[sum(row['wall_ns'] for row in result['reconciliation'] if row['operation']==name)/1e6
                    for result in timings] for name in ['places','residences','memberships']}
    counts = [result for result in runs.values() if result['reconciliation_mode']=='counts']
    if len(counts)>1:
        work = lambda result:[(row['operation'],row['machines'],{key:value for key,value in row['metrics'].items()
                              if not key.endswith('_ns')}) for row in result['reconciliation']]
        assert all(work(result)==work(counts[0]) for result in counts[1:]), 'work counts changed'
    return dict(runs=runs, timings_ms=totals,
                median_ms={name:statistics.median(values) for name,values in totals.items() if values})

if __name__=='__main__':
    root = pathlib.Path(sys.argv[1]) if len(sys.argv)>1 else pathlib.Path(__file__).resolve().parent/'raw'
    summary = summarize(root)
    (root/'reconciliation-summary.json').write_text(json.dumps(summary,indent=2),encoding='utf-8')
    print(json.dumps({key:value for key,value in summary.items() if key!='runs'},indent=2))
