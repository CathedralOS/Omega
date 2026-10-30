from pathlib import Path
import collections,gzip,importlib.util,json,statistics,sys
ROOT=next(p for p in Path(__file__).resolve().parents if (p/'AGENTS.md').is_file())
WORK=ROOT/'build/proof-profile/source-checking-cpu'
def stats(values):
    values=list(values);return dict(median=statistics.median(values),min=min(values),max=max(values),all=values)
def outside_coverage(run):
    windows=run['checking']['outside']
    roots=[w for w in windows if not any(p['id']!=w['id'] and p['start_ns']<=w['start_ns'] and p['end_ns']>=w['end_ns'] for p in windows)]
    roots.sort(key=lambda w:w['start_ns'])
    assert all(a['end_ns']<=b['start_ns'] for a,b in zip(roots,roots[1:]))
    check_roots=[n for n in run['checking']['nodes'] if n['name']=='05']
    assert all(w['end_ns']<=r['start_ns'] or w['start_ns']>=r['end_ns'] for w in windows for r in check_roots)
    return dict(process_cpu=sum(w['process_cpu'] for w in roots),wall=sum(w['wall'] for w in roots),windows=len(windows),outer_windows=len(roots))
def analyze(records):
    groups={key:[r for r in records if r['probe_enabled'] and ('warm' in r['label'])==warm] for key,warm in [('cold',False),('warm',True)]}
    report={}
    for group,runs in groups.items():
        assert runs
        rows=[]
        for run in runs:
            byid={n['id']:n for n in run['checking']['nodes']}
            buckets=collections.defaultdict(lambda:collections.defaultdict(float))
            for n in byid.values():
                parent=byid[n['parent']]['name'] if n['parent'] is not None else '<root>'
                b=buckets[(parent,n['name'])]
                for field in ['process_cpu','thread_cpu','wall','self_process_cpu','self_thread_cpu','self_wall']:b[field]+=n[field]
                b['count']+=1
            rows.append(buckets)
        pairs=sorted(set().union(*(set(r) for r in rows)))
        buckets=[dict(parent=p,name=n,**{f:stats(r[(p,n)][f] for r in rows) for f in ['process_cpu','thread_cpu','wall','self_process_cpu','self_thread_cpu','self_wall','count']}) for p,n in pairs]
        report[group]=dict(samples=len(runs),compilation_wall=stats(r['wall_seconds'] for r in runs),compilation_cpu=stats(r['cpu_seconds'] for r in runs),
            checking_cpu=stats(r['checking']['process_cpu'] for r in runs),checking_thread_cpu=stats(r['checking']['thread_cpu'] for r in runs),checking_wall=stats(r['checking']['wall'] for r in runs),
            checking_percent_of_compile_cpu=stats(100*r['checking']['process_cpu']/r['cpu_seconds'] for r in runs),
            thread_percent_of_check_cpu=stats(100*r['checking']['thread_cpu']/r['checking']['process_cpu'] for r in runs),
            roots=[r['checking']['roots'] for r in runs],external_capacity=stats(r['external_busy_capacity_percent'] for r in runs),buckets=buckets)
        report[group]['outside_selected_helper_coverage']={field:stats(outside_coverage(r)[field] for r in runs) for field in ['process_cpu','wall','windows','outer_windows']}
    controls=[r for r in records if not r['probe_enabled'] and 'cold' in r['label']]
    report['controls']=dict(samples=len(controls),wall=stats(r['wall_seconds'] for r in controls),cpu=stats(r['cpu_seconds'] for r in controls),external_capacity=stats(r['external_busy_capacity_percent'] for r in controls))
    report['observations']=[{k:r[k] for k in ['label','probe_enabled','cache_mode','wall_seconds','cpu_seconds','external_busy_capacity_percent']} for r in records]
    return report
def main():
    if len(sys.argv)>1:folder=Path(sys.argv[1])
    else:folder=WORK/'observations'
    paths=sorted(folder.glob('*.json')) or sorted(folder.glob('*/result.json.gz'))
    records=[json.loads(gzip.decompress(p.read_bytes()).decode('utf-8') if p.suffix=='.gz' else p.read_text(encoding='utf-8')) for p in paths]
    assert len(records)==9
    report=analyze(records)
    (folder.parent/'summary.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    for group in ['cold','warm']:
        r=report[group];print(group,'CPU',r['checking_cpu'],'wall',r['checking_wall'],'compileCPU%',r['checking_percent_of_compile_cpu'])
        print('Top source-check children:')
        for b in sorted([b for b in r['buckets'] if b['parent']=='05'],key=lambda b:b['process_cpu']['median'],reverse=True):print(b['name'],b['process_cpu'])
        print('Largest exclusive owners:')
        for b in sorted(r['buckets'],key=lambda b:b['self_process_cpu']['median'],reverse=True)[:12]:print(b['name'],b['self_process_cpu'])
    print('CONTROLS',report['controls'])
if __name__=='__main__':main()
