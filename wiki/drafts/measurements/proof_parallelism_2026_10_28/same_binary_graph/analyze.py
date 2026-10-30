"""Exact replay of a one-producer, two-mode comparison and its repeat controls."""
from pathlib import Path
import gzip,hashlib,importlib.util,json,statistics,sys
HERE=Path(__file__).resolve().parent
ROOT=next(p for p in HERE.parents if (p/'AGENTS.md').is_file())
LEDGER=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28'
METRICS=['wall_seconds','cpu_seconds','checking_seconds','lowering_seconds']

def read(path):return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(path.suffix+'.gz').read_bytes())

def module(path):
    spec=importlib.util.spec_from_file_location('experiment',path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m

def effect(eager,lazy):return dict(eager=eager,lazy=lazy,reduction=eager-lazy,reduction_percent=100*(eager-lazy)/eager)

def observed(report):return [{k:r[k] for k in ['fixture','tier','status','expected_satisfied','facts','diagnostics']} for r in report['observations']]

def analyze(folder,raw,run_script):
    script=module(run_script);identity=json.loads((folder/'identity.json').read_text());producer=json.loads((folder/'producer.json').read_text())
    assert identity['producer']==producer and int(producer['build_profile']['opt_level'])>0 and int(producer['corpus_profile']['opt_level'])>0
    assert identity['cold_schedule']==script.schedule() and len(identity['cold_schedule'])==6
    assert identity['warm_orders']==['EL' if b in [0,3,4] else 'LE' for b in range(6)]
    source=json.loads((folder/'source.json').read_text());assert hashlib.sha256((folder/'diagnostic.patch').read_bytes()).hexdigest()==source['patch_sha256']
    checks=json.loads((folder/'patch-checks.json').read_text());assert checks['applies'] and checks['base']==source['base'] and checks['patched_source_sha256']==source['source_sha256']
    inputs=read(folder/'input-manifest.json');assert hashlib.sha256(inputs).hexdigest()==identity['input_manifest_sha256']
    assert json.loads(inputs)['inputs']==json.loads((LEDGER/'input-manifest.json').read_text())['inputs']
    expected=(LEDGER/'lazy_reference_graph/raw/lazy-original-cold-0/runtime/stdout.txt').read_bytes()
    rows={};all_samples=[]
    for case in sorted(raw.glob('sbg-*')):
        r=json.loads((case/'result.json').read_text());assert r['exit_code']==0 and r['producer']==producer['producer']
        for key in ['compiler_sha256','accepted_lock_sha256']:assert r[key]==producer[key]
        assert r['program_sha256']=='b98c24db839027800ada039e525f355e80f4cccaf22079bfb68d8a6b48105077'
        mode=r['graph_mode'];assert mode in ['eager','lazy']
        stderr=read(case/'stderr.txt').decode('utf-8');assert script.graph_counts(stderr)==r['graph_counts']
        assert not any(line.startswith(('RECONCILE\t','REFERENCE_','SOURCE_QUERY_','FULL\t','LAZY_REFERENCE_')) for line in stderr.splitlines())
        counts=dict(ready=[[mode,202]]*4,builds=[[202,202]]*3 if mode=='eager' else []) if r['counts_enabled'] else dict(ready=[],builds=[])
        assert r['graph_counts']==counts
        if r['comparison']:
            assert not r['counts_enabled'] and r['terminal_stdout_agrees'] and r['retained_image_sha256']==script.IMAGE
            assert read(case/'stdout.txt')==read(case/'terminal.stdout')==expected
        else:
            assert r['image_sha256']==script.IMAGE and read(case/'runtime/stdout.txt')==expected
            runtime=json.loads((case/'runtime/result.json').read_text());assert runtime['exit_code']==0
            assert script.cold.phases(stderr)=={k:r[k] for k in ['checking_seconds','lowering_seconds']}
        telemetry=json.loads(read(case/'telemetry.json'));samples=telemetry['samples'];end=samples[-1];start=telemetry['system_before']
        cpu=end['user_seconds']+end['kernel_seconds'];assert cpu==r['cpu_seconds']
        d={k:end['system'][k]-start[k] for k in start};capacity=d['kernel']+d['user'];busy=capacity-d['idle'];external=busy-cpu
        assert capacity==r['system_cpu_capacity_seconds'] and busy==r['system_busy_seconds'] and external==r['external_busy_seconds']
        assert 100*external/capacity==r['external_busy_capacity_percent']
        assert sum(s['sampling_seconds'] for s in samples)==r['sampling_seconds']
        for a,b in zip(samples,samples[1:]):
            assert a['t']<=b['t'] and a['user_seconds']<=b['user_seconds'] and a['kernel_seconds']<=b['kernel_seconds']
            assert all(a['io'][k]<=b['io'][k] for k in a['io'])
        memory=[s['memory'] for s in samples if s['memory'] is not None]
        for out,key in [('peak_working_set_bytes','peak_working_set'),('peak_commit_bytes','peak_pagefile'),('page_faults_observed','faults')]:assert r[out]==max(s[key] for s in memory)
        if r['warm_cache_from']:
            seed=json.loads((raw/r['warm_cache_from']/'result.json').read_text());assert seed['graph_mode']==mode and seed['compiler_sha256']==r['compiler_sha256'] and seed['accepted_lock_sha256']==r['accepted_lock_sha256']
            assert r['cache_mode']=='warm reuse of own mode cold private cache'
        elif not r['comparison']:assert r['cache_mode']=='fresh private cache'
        rows[case.name]=r;all_samples.append(r)
    assert len(rows)==42 and sum(r['comparison'] for r in rows.values())==2 and sum(r['counts_enabled'] for r in rows.values())==4
    cold=[];warm=[];repeats=[]
    for block,order in enumerate(identity['cold_schedule']):
        group={mode:[] for mode in ['eager','lazy']};seeds={}
        for position,symbol in enumerate(order):
            mode='eager' if symbol=='E' else 'lazy';label=f'sbg-cold-{block}-{position}-{mode}';r=rows[label]
            assert not r['counts_enabled'] and not r['comparison'] and r['series']=='cold' and r['block']==block and r['position']==position and r['order']==order
            group[mode].append(r);seeds[mode]=label
        cold.append(dict(block=block,order=order,metrics={k:effect(statistics.mean(r[k] for r in group['eager']),statistics.mean(r[k] for r in group['lazy'])) for k in METRICS}))
        for mode,data in group.items():
            repeats.append(dict(block=block,mode=mode,distance=data[1]['position']-data[0]['position'],metrics={k:100*abs(data[1][k]-data[0][k])/statistics.mean(r[k] for r in data) for k in METRICS}))
        pair={mode:rows[f'sbg-warm-{block}-{mode}'] for mode in group}
        for position,symbol in enumerate(identity['warm_orders'][block]):
            mode='eager' if symbol=='E' else 'lazy';r=pair[mode]
            assert r['warm_cache_from']==seeds[mode] and r['series']=='warm' and r['block']==block and r['position']==position and r['order']==identity['warm_orders'][block] and not r['counts_enabled']
        warm.append(dict(block=block,order=identity['warm_orders'][block],metrics={k:effect(pair['eager'][k],pair['lazy'][k]) for k in METRICS}))
    def summary(blocks):
        return {k:dict(lazy_faster=sum(b['metrics'][k]['reduction']>0 for b in blocks),median_reduction_percent=statistics.median(b['metrics'][k]['reduction_percent'] for b in blocks),
            reduction_percent_range=[min(b['metrics'][k]['reduction_percent'] for b in blocks),max(b['metrics'][k]['reduction_percent'] for b in blocks)]) for k in METRICS}
    quality=json.loads((folder/'quality-summary.json').read_text());assert not quality['golden_recorded'] and len(quality['legs'])==4
    for mode in ['eager','lazy']:
        for leg,count in [('positive',4),('negative',3)]:
            report=json.loads((folder/(mode+'-'+leg+'.json')).read_text());baseline=json.loads((LEDGER/'lazy_reference_graph'/('baseline-quality-'+leg+'.json')).read_text())
            assert report['exit_code']==1 and not report['oracle_failures'] and not report['infrastructure_error'] and len(report['observations'])==count
            assert observed(report)==observed(baseline)
    timed=[r for r in all_samples if r.get('series') in ['cold','warm']]
    resources={series:{mode:{k:[min(r[k] for r in timed if r['series']==series and r['graph_mode']==mode),max(r[k] for r in timed if r['series']==series and r['graph_mode']==mode)] for k in ['external_busy_capacity_percent','peak_commit_bytes','page_faults_observed']} for mode in ['eager','lazy']} for series in ['cold','warm']}
    return dict(cold_blocks=cold,warm_pairs=warm,cold_summary=summary(cold),warm_summary=summary(warm),same_mode_repeats=repeats,resources=resources,
        same_mode_variation={k:dict(median_percent=statistics.median(r['metrics'][k] for r in repeats),maximum_percent=max(r['metrics'][k] for r in repeats)) for k in METRICS},
        host_capacity_percent_range=[min(r['external_busy_capacity_percent'] for r in all_samples),max(r['external_busy_capacity_percent'] for r in all_samples)],
        maximum_sampling_fraction_percent=max(100*r['sampling_seconds']/r['wall_seconds'] for r in all_samples),observations=42,timed_cold=24,timed_warm=12)

if __name__=='__main__':
    if len(sys.argv)>1:
        work=Path(sys.argv[1]);raw=work.parent;script=work.parent/'same_binary_run.py'
        result=analyze(work,raw,script);(work/'summary.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
    else:
        manifest=json.loads((HERE/'manifest.json').read_text());actual={p.relative_to(HERE).as_posix() for p in HERE.rglob('*') if p.is_file() and p.name!='manifest.json' and '__pycache__' not in p.parts};assert set(manifest)==actual
        for name,digest in manifest.items():assert hashlib.sha256((HERE/name).read_bytes()).hexdigest()==digest,name
        result=analyze(HERE,HERE/'raw',HERE/'run.py');assert result==json.loads((HERE/'summary.json').read_text())
    print('COLD',result['cold_summary']);print('WARM',result['warm_summary']);print('SAME MODE',result['same_mode_variation'])
    print('Verified one compiler+lock identity,42 observations,3->0 graph builds,40 native executions,2 engine comparisons,balanced cold blocks/warm pairs and exact baseline-equivalent quality outcomes.')
