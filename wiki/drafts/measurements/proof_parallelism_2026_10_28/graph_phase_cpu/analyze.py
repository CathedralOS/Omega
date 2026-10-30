from pathlib import Path
import gzip,hashlib,importlib.util,json,statistics,sys
HERE=Path(__file__).resolve().parent
ROOT=next(p for p in HERE.parents if (p/'AGENTS.md').is_file())
LEDGER=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28'
FIELDS=['cpu_seconds','wall_seconds','before_lowering_cpu','checking_cpu','other_before_lowering_cpu','lowering_cpu','after_lowering_cpu','from_lowering_cpu']

def read(path):return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(path.suffix+'.gz').read_bytes())
def module(path):
    spec=importlib.util.spec_from_file_location('cpu_experiment',path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m

def analyze(folder,raw,script_path):
    script=module(script_path);identity=json.loads((folder/'identity.json').read_text());producer=json.loads((folder/'producer.json').read_text());source=json.loads((folder/'source.json').read_text())
    assert producer==identity['producer'] and int(producer['build_profile']['opt_level'])>0 and identity['schedule']==script.schedule()
    assert hashlib.sha256((folder/'diagnostic.patch').read_bytes()).hexdigest()==source['patch_sha256']
    member=json.loads((LEDGER/'same_binary_graph/source.json').read_text());assert source['files'][member['file']]==member['source_sha256']==source['membership_unchanged_sha256']
    checks=json.loads((folder/'patch-checks.json').read_text());assert checks['applies'] and checks['base']==source['base'] and checks['files']==source['files']
    inputs=read(folder/'input-manifest.json');assert hashlib.sha256(inputs).hexdigest()==identity['input_manifest_sha256']
    assert json.loads(inputs)['inputs']==json.loads((LEDGER/'input-manifest.json').read_text())['inputs']
    expected=(LEDGER/'lazy_reference_graph/raw/lazy-original-cold-0/runtime/stdout.txt').read_bytes();rows={}
    for case in sorted(raw.glob('gpc-*')):
        r=json.loads((case/'result.json').read_text());assert r['exit_code']==0 and r['producer']==producer['producer'] and r['observer']=='endpoints only'
        for k in ['compiler_sha256','accepted_lock_sha256']:assert r[k]==producer[k]
        assert r['program_sha256']=='b98c24db839027800ada039e525f355e80f4cccaf22079bfb68d8a6b48105077'
        assert r['cpu_seconds']==r['user_seconds']+r['kernel_seconds']
        d={k:r['system_after'][k]-r['system_before'][k] for k in r['system_before']};capacity=d['kernel']+d['user'];busy=capacity-d['idle'];external=busy-r['cpu_seconds']
        assert r['system_cpu_capacity_seconds']==capacity and r['system_busy_seconds']==busy and r['external_busy_seconds']==external and r['external_busy_capacity_percent']==100*external/capacity
        stderr=read(case/'stderr.txt').decode('utf-8');windows=script.parse(stderr);assert windows==r['cpu_windows']
        assert not any(line.startswith(('RECONCILE\t','REFERENCE_','SOURCE_QUERY_','FULL\t','LAZY_REFERENCE_')) for line in stderr.splitlines())
        counted=script.same.graph_counts(stderr);assert counted==r['graph_counts']
        assert counted==(dict(ready=[[r['graph_mode'],202]]*4,builds=[[202,202]]*3 if r['graph_mode']=='eager' else []) if r['counts_enabled'] else dict(ready=[],builds=[]))
        if r['comparison']:
            assert not windows and not r['counts_enabled'] and r['terminal_stdout_agrees'] and r['retained_image_sha256']==script.same.IMAGE
            assert read(case/'stdout.txt')==read(case/'terminal.stdout')==expected
        else:
            assert r['cpu_partition']==script.partition(windows,r['cpu_seconds'])
            assert r['image_sha256']==script.same.IMAGE and read(case/'runtime/stdout.txt')==expected
            assert json.loads((case/'runtime/result.json').read_text())['exit_code']==0
            assert script.same.cold.phases(stderr)=={k:r[k] for k in ['checking_seconds','lowering_seconds']}
            assert r['cache_mode']=='fresh private cache' and r['warm_cache_from'] is None
        rows[case.name]=r
    assert len(rows)==28 and sum(r['comparison'] for r in rows.values())==2 and sum(r['counts_enabled'] for r in rows.values())==2
    retained=json.loads((folder/'retained-implementation.json').read_text());pure_source=json.loads((LEDGER/'lazy_reference_graph/candidate-source.json').read_text());pure_identity=json.loads((LEDGER/'lazy_reference_graph/identities.json').read_text())[1]
    assert retained['file']==pure_source['file'] and retained['source_sha256']==pure_source['source_sha256'] and retained['producer']==pure_identity
    assert not retained['golden_recorded'] and not retained['diagnostic_flags_retained']
    def observed(report):return [{k:o[k] for k in ['fixture','tier','status','expected_satisfied','facts','diagnostics']} for o in report['observations']]
    for leg,count in [('positive',4),('negative',3)]:
        report=json.loads((folder/('clean-'+leg+'.json')).read_text());baseline=json.loads((LEDGER/'lazy_reference_graph'/('baseline-quality-'+leg+'.json')).read_text())
        assert report['exit_code']==1 and not report['oracle_failures'] and not report['infrastructure_error'] and len(report['observations'])==count and observed(report)==observed(baseline)
    clean=folder/'clean-main' if (folder/'clean-main').exists() else folder.parent/'lazy-clean-final-both'
    clean_result=json.loads((clean/'result.json').read_text());assert clean_result['exit_code']==0 and clean_result['terminal_stdout_agrees'] and clean_result['retained_image_sha256']==script.same.IMAGE
    for key in ['compiler_sha256','accepted_lock_sha256']:assert clean_result[key]==pure_identity[key]
    assert read(clean/'stdout.txt')==read(clean/'terminal.stdout')==expected and not script.parse(read(clean/'stderr.txt').decode('utf-8'))
    blocks=[];repeats=[];timed=[]
    for block,order in enumerate(identity['schedule']):
        group={mode:[] for mode in ['eager','lazy']}
        for position,symbol in enumerate(order):
            mode='eager' if symbol=='E' else 'lazy';r=rows[f'gpc-{block}-{position}-{mode}']
            assert r['block']==block and r['position']==position and r['order']==order and r['graph_mode']==mode and not r['counts_enabled'] and not r['comparison']
            data=dict(r,**r['cpu_partition']);data['from_lowering_cpu']=data['lowering_cpu']+data['after_lowering_cpu'];group[mode].append(data);timed.append(data)
        metrics={}
        for name in FIELDS:
            e=statistics.mean(r[name] for r in group['eager']);l=statistics.mean(r[name] for r in group['lazy'])
            metrics[name]=dict(eager=e,lazy=l,lazy_additional=l-e,lazy_additional_percent=100*(l-e)/e if e else None)
        blocks.append(dict(block=block,order=order,metrics=metrics))
        for mode,data in group.items():
            repeats.append(dict(block=block,mode=mode,metrics={name:100*abs(data[0][name]-data[1][name])/statistics.mean(r[name] for r in data) if statistics.mean(r[name] for r in data) else 0 for name in FIELDS}))
    summary={name:dict(lazy_more=sum(b['metrics'][name]['lazy_additional']>0 for b in blocks),mean_additional_seconds=statistics.mean(b['metrics'][name]['lazy_additional'] for b in blocks),median_additional_seconds=statistics.median(b['metrics'][name]['lazy_additional'] for b in blocks),
        median_additional_percent=statistics.median(b['metrics'][name]['lazy_additional_percent'] for b in blocks),additional_seconds_range=[min(b['metrics'][name]['lazy_additional'] for b in blocks),max(b['metrics'][name]['lazy_additional'] for b in blocks)]) for name in FIELDS}
    # Signed mean differences close across disjoint regions. Medians need not add.
    for b in blocks:
        m=b['metrics'];assert abs(m['cpu_seconds']['lazy_additional']-sum(m[k]['lazy_additional'] for k in ['before_lowering_cpu','lowering_cpu','after_lowering_cpu']))<1e-8
    assert abs(summary['cpu_seconds']['mean_additional_seconds']-sum(summary[k]['mean_additional_seconds'] for k in ['before_lowering_cpu','lowering_cpu','after_lowering_cpu']))<1e-8
    return dict(blocks=blocks,summary=summary,same_mode_repeats=repeats,
        slower_total_blocks=[dict(block=b['block'],before=b['metrics']['before_lowering_cpu']['lazy_additional'],lowering=b['metrics']['lowering_cpu']['lazy_additional'],after=b['metrics']['after_lowering_cpu']['lazy_additional'],total=b['metrics']['cpu_seconds']['lazy_additional']) for b in blocks if b['metrics']['cpu_seconds']['lazy_additional']>0],
        repeat_median_percent={name:statistics.median(r['metrics'][name] for r in repeats) for name in FIELDS},
        timed_host_capacity_percent_range=[min(r['external_busy_capacity_percent'] for r in timed),max(r['external_busy_capacity_percent'] for r in timed)],observations=28,timings=24)

if __name__=='__main__':
    if len(sys.argv)>1:
        folder=Path(sys.argv[1]);result=analyze(folder,folder.parent,folder.parent/'graph_phase_run.py');(folder/'summary.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
    else:
        manifest=json.loads((HERE/'manifest.json').read_text());actual={p.relative_to(HERE).as_posix() for p in HERE.rglob('*') if p.is_file() and p.name!='manifest.json' and '__pycache__' not in p.parts};assert set(manifest)==actual
        for name,digest in manifest.items():assert hashlib.sha256((HERE/name).read_bytes()).hexdigest()==digest,name
        result=analyze(HERE,HERE/'raw',HERE/'run.py');assert result==json.loads((HERE/'summary.json').read_text())
    for name in FIELDS:print(name,result['summary'][name])
    print('Verified28 diagnostic observations,one diagnostic producer/lock,CPU partition closure,balanced24 cold timings,12 same-mode controls,3->0 graph counts,and clean-source identity plus fresh production fixture/Dungeon both-engine qualification.')
