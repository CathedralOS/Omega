"""Replay all blocks, including slow same-binary controls; never trim observations."""
from pathlib import Path
import gzip,hashlib,importlib.util,json,statistics
HERE=Path(__file__).resolve().parent
ROOT=next(p for p in HERE.parents if (p/'AGENTS.md').is_file())
LEDGER=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28'
METRICS=['wall_seconds','cpu_seconds','checking_seconds','lowering_seconds','user_seconds','kernel_seconds',
    'peak_working_set_bytes','peak_commit_bytes','page_faults_observed','external_busy_capacity_percent']

def read(path):
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(path.suffix+'.gz').read_bytes())

def phases(text):
    import re
    r={name:float(ms)/1000 for ms,name in re.findall(r'^\s*([0-9.]+) ms  (.+)$',text,re.M)}
    return dict(checking_seconds=r['Stage 05: TypedTrees -> CheckedTrees'],lowering_seconds=r['terminal-production/lowering: CheckedTrees -> LoweredPsi'])

def analyze(folder,raw):
    identity=json.loads((folder/'identity.json').read_text());expected=json.loads((LEDGER/'lazy_reference_graph/identities.json').read_text())
    manifest=read(folder/'input-manifest.json')
    assert hashlib.sha256(manifest).hexdigest()==identity['input_manifest_sha256']
    assert json.loads(manifest)['inputs']==json.loads((LEDGER/'input-manifest.json').read_text())['inputs']
    assert identity['identities']==expected and len(identity['schedule'])==6 and identity['schedule'].count('ABBA')==identity['schedule'].count('BAAB')==3
    identities={x['role']:x for x in expected};rows=[];blocks=[];repeats=[];output_hashes=set()
    for block,order in enumerate(identity['schedule']):
        group={role:[] for role in identities}
        for position,symbol in enumerate(order):
            role='original' if symbol=='A' else 'candidate';case=raw/f'cold-noise-{block}-{position}-{role}'
            r=json.loads((case/'result.json').read_text());runtime=json.loads((case/'runtime/result.json').read_text())
            assert r['block']==block and r['position']==position and r['order']==order and r['role']==role
            assert r['producer']==identities[role]['producer'] and r['exit_code']==runtime['exit_code']==0 and r['cache_mode']=='fresh private cache' and r['warm_cache_from'] is None
            for key in ['compiler_sha256','accepted_lock_sha256']:assert r[key]==identities[role][key]
            assert r['program_sha256']=='b98c24db839027800ada039e525f355e80f4cccaf22079bfb68d8a6b48105077'
            assert r['image_sha256']=='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
            stdout=read(case/'runtime/stdout.txt');assert len(stdout)==463 and b'A bottomless dark room near the dungeon heart.' in stdout
            output_hashes.add(hashlib.sha256(stdout).hexdigest())
            stderr=read(case/'stderr.txt').decode('utf-8');assert phases(stderr)=={k:r[k] for k in ['checking_seconds','lowering_seconds']}
            assert not any(line.startswith(('RECONCILE\t','REFERENCE_','SOURCE_QUERY_','FULL\t','LAZY_REFERENCE_')) for line in stderr.splitlines())
            t=json.loads(read(case/'telemetry.json'));samples=t['samples'];last=samples[-1];start=t['system_before']
            assert abs(r['cpu_seconds']-last['user_seconds']-last['kernel_seconds'])<1e-8
            d={k:last['system'][k]-start[k] for k in start};capacity=d['kernel']+d['user'];busy=capacity-d['idle'];external=busy-r['cpu_seconds']
            assert r['system_cpu_capacity_seconds']==capacity and r['system_busy_seconds']==busy and r['external_busy_seconds']==external
            assert r['external_busy_capacity_percent']==100*external/capacity
            assert abs(sum(s['sampling_seconds'] for s in samples)-r['sampling_seconds'])<1e-8 and r['sampling_seconds']>=0
            for a,b in zip(samples,samples[1:]):
                assert a['t']<=b['t'] and a['user_seconds']<=b['user_seconds'] and a['kernel_seconds']<=b['kernel_seconds']
                assert all(a['io'][k]<=b['io'][k] for k in a['io'])
            memory=[s['memory'] for s in samples if s['memory'] is not None]
            for out,key in [('peak_working_set_bytes','peak_working_set'),('peak_commit_bytes','peak_pagefile'),('page_faults_observed','faults')]:assert r[out]==max(s[key] for s in memory)
            group[role].append(r);rows.append(r)
        result=dict(block=block,order=order,metrics={})
        for metric in METRICS:
            a=statistics.mean(x[metric] for x in group['original']);b=statistics.mean(x[metric] for x in group['candidate'])
            result['metrics'][metric]=dict(original=a,candidate=b,reduction=a-b,reduction_percent=100*(a-b)/a if a else None)
        blocks.append(result)
        for role,data in group.items():
            repeat=dict(block=block,role=role,distance=data[1]['position']-data[0]['position'],metrics={})
            for metric in METRICS:
                a,b=(r[metric] for r in data);mean=(a+b)/2
                repeat['metrics'][metric]=dict(first=a,second=b,absolute_difference=abs(a-b),absolute_difference_percent=100*abs(a-b)/mean if mean else None)
            repeats.append(repeat)
    old_stdout=(LEDGER/'lazy_reference_graph/raw/lazy-original-cold-0/runtime/stdout.txt').read_bytes()
    assert len(rows)==24 and output_hashes=={hashlib.sha256(old_stdout).hexdigest()}
    metrics={}
    for metric in METRICS:
        effects=[b['metrics'][metric] for b in blocks];control=[r['metrics'][metric] for r in repeats]
        metrics[metric]=dict(block_reduction_count=sum(e['reduction']>0 for e in effects),median_block_reduction_percent=statistics.median(e['reduction_percent'] for e in effects),
            block_reduction_percent_range=[min(e['reduction_percent'] for e in effects),max(e['reduction_percent'] for e in effects)],
            median_same_binary_absolute_difference_percent=statistics.median(e['absolute_difference_percent'] for e in control),
            maximum_same_binary_absolute_difference_percent=max(e['absolute_difference_percent'] for e in control))
    per_role={role:{metric:[min(r[metric] for r in rows if r['role']==role),max(r[metric] for r in rows if r['role']==role)] for metric in METRICS} for role in identities}
    correlations={role:{metric:statistics.correlation([r[metric] for r in rows if r['role']==role],[r['external_busy_capacity_percent'] for r in rows if r['role']==role]) for metric in ['wall_seconds','cpu_seconds','checking_seconds','lowering_seconds']} for role in identities}
    old_pairs=json.loads((LEDGER/'lazy_reference_graph/summary.json').read_text())['cold']['pairs']
    old_phases=json.loads((LEDGER/'lazy_reference_graph/reported-phase-observations.json').read_text())['cold']
    attribution=[]
    for old,phase in zip(old_pairs,old_phases):
        assert old['round']==phase['round']
        wall=old['candidate_wall']-old['original_wall']
        check=(phase['candidate']['reported_checking_ms'][0]-phase['original']['reported_checking_ms'][0])/1000
        lower=(phase['candidate']['reported_lowering_ms'][0]-phase['original']['reported_lowering_ms'][0])/1000
        attribution.append(dict(round=old['round'],additional_wall_seconds=wall,additional_checking_seconds=check,additional_lowering_seconds=lower,
            remaining_additional_wall_seconds=wall-check-lower,checking_fraction_of_additional_wall_percent=100*check/wall if wall>0 else None))
    io_ranges={role:{k:[min(r['io'][k] for r in rows if r['role']==role),max(r['io'][k] for r in rows if r['role']==role)] for k in rows[0]['io']} for role in identities}
    return dict(blocks=blocks,same_binary_repeats=repeats,metrics=metrics,ranges=per_role,io_ranges=io_ranges,
        descriptive_host_correlations=correlations,prior_cold_attribution=attribution,
        max_sampling_seconds=max(r['sampling_seconds'] for r in rows),median_sampling_seconds=statistics.median(r['sampling_seconds'] for r in rows),
        maximum_sampling_fraction_percent=max(100*r['sampling_seconds']/r['wall_seconds'] for r in rows),observations=24)

if __name__=='__main__':
    import sys
    if len(sys.argv)>1:
        work=Path(sys.argv[1]);raw=work.parent
        summary=analyze(work,raw);(work/'summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8',newline='\n')
    else:
        manifest=json.loads((HERE/'manifest.json').read_text());actual={p.relative_to(HERE).as_posix() for p in HERE.rglob('*') if p.is_file() and p.name!='manifest.json' and '__pycache__' not in p.parts}
        assert set(manifest)==actual
        for name,digest in manifest.items():assert hashlib.sha256((HERE/name).read_bytes()).hexdigest()==digest,name
        summary=analyze(HERE,HERE/'raw');assert summary==json.loads((HERE/'summary.json').read_text())
    for key in ['wall_seconds','cpu_seconds','checking_seconds','lowering_seconds']:print(key,summary['metrics'][key])
    print('All24 cold observations,12 identical-binary repeat controls,centered blocks,telemetry accounting and exact native outcomes verified.')
