from pathlib import Path
import hashlib,importlib.util,json,os,re,statistics,subprocess
ROOT=next(p for p in Path(__file__).resolve().parents if (p/'AGENTS.md').is_file())
RECORD=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28'
spec=importlib.util.spec_from_file_location('cache',RECORD/'cache_diagnostics/cache_diagnostics_measure.py')
h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
runner=h.parent.runner;OUT=runner.OUT;WORK=OUT/'lazy-reference-graph'
IMAGE='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
PRODUCERS={'original':'baseline','candidate':'lazy-reference'}

def run(role,label,warm_from=None,comparison=False):
    for flag in list(os.environ):
        if flag.startswith('OMEGA_') and any(x in flag for x in ['PROBE','PROFILE','COUNTS','MEASUREMENT','EDGE_CACHE']):os.environ.pop(flag,None)
    saved=os.environ.get('LOCALAPPDATA')
    if warm_from:os.environ['LOCALAPPDATA']=str(OUT/'private-cache'/warm_from)
    try:
        runner.verify_inputs();runner.measure(PRODUCERS[role],label,'off',0,comparison=comparison,phase_profile=False,cold_cache=not warm_from and not comparison);runner.verify_inputs()
    finally:
        if saved is None:os.environ.pop('LOCALAPPDATA',None)
        else:os.environ['LOCALAPPDATA']=saved
    folder=OUT/label;r=json.loads((folder/'result.json').read_text());stderr=(folder/'stderr.txt').read_text()
    assert not any(x.startswith(('RECONCILE\t','REFERENCE_','SOURCE_QUERY_','FULL\t')) for x in stderr.splitlines()),'instrumented producer in pure timings'
    if comparison:
        kept=Path(re.search(r'build dir kept: (.+)',stderr)[1].strip());stdout=(folder/'stdout.txt').read_bytes()
        assert stdout==(kept/'terminal.stdout').read_bytes() and len(stdout)==463
        assert runner.digest(kept/'omega-program.exe')==IMAGE
        (folder/'terminal.stdout').write_bytes(stdout);r.update(terminal_stdout_agrees=True,retained_image_sha256=IMAGE)
    else:assert r['image_sha256']==IMAGE
    r.update(role=role,warm_cache_from=warm_from,cache_mode='warm reuse of own cold private cache' if warm_from else r['cache_mode'])
    (folder/'result.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8',newline='\n')
    print('PURE',label,round(r['wall_seconds'],4),round(r['cpu_seconds'],4),flush=True)
    return r

def summarize(root=OUT):
    summary={}
    for mode in ['cold','warm']:
        pairs=[]
        for round_number in range(6):
            rows={role:json.loads((root/f'lazy-{role}-{mode}-{round_number}'/'result.json').read_text()) for role in PRODUCERS}
            a,b=rows['original'],rows['candidate']
            pairs.append(dict(round=round_number,original_wall=a['wall_seconds'],candidate_wall=b['wall_seconds'],wall_gain_percent=100*(a['wall_seconds']-b['wall_seconds'])/a['wall_seconds'],original_cpu=a['cpu_seconds'],candidate_cpu=b['cpu_seconds'],cpu_gain_percent=100*(a['cpu_seconds']-b['cpu_seconds'])/a['cpu_seconds']))
        wins=sum(p['wall_gain_percent']>0 for p in pairs);cpu_wins=sum(p['cpu_gain_percent']>0 for p in pairs)
        wall_median=statistics.median(p['wall_gain_percent'] for p in pairs);cpu_median=statistics.median(p['cpu_gain_percent'] for p in pairs)
        summary[mode]=dict(pairs=pairs,wall_wins=wins,cpu_wins=cpu_wins,median_wall_gain_percent=wall_median,median_cpu_gain_percent=cpu_median,supported=wins>=5 and cpu_wins>=4 and wall_median>0 and cpu_median>0)
    summary['promotion_supported']=all(summary[m]['supported'] for m in ['cold','warm'])
    return summary

def self_check():
    assert sum(1 for r in range(6) if r%2==0)==3
    assert sum(1 for r in range(6) if r in [0,3,4])==3
    assert statistics.median([3,-1,5,2,-2,4])==2.5
self_check()

if __name__=='__main__':
    identities=[]
    for role,name in PRODUCERS.items():
        p=OUT/'producers'/name;profile=json.loads(subprocess.check_output([str(p/'omega.exe'),'--build-profile']));assert int(profile['opt_level'])>0
        identities.append(dict(role=role,producer=name,compiler_sha256=runner.digest(p/'omega.exe'),accepted_lock_sha256=runner.digest(p/'omega.lock'),build_profile=profile))
    expected=json.loads((RECORD/'identity.json').read_text())['producers']['baseline'];assert identities[0]['compiler_sha256']==expected['sha256'] and identities[0]['build_profile']==expected['profile']
    (WORK/'identities.json').write_text(json.dumps(identities,indent=2)+'\n',encoding='utf-8',newline='\n')
    for round_number in range(6):
        cold_order=['original','candidate'] if round_number%2==0 else ['candidate','original']
        warm_order=['original','candidate'] if round_number in [0,3,4] else ['candidate','original']
        for role in cold_order:run(role,f'lazy-{role}-cold-{round_number}')
        for role in warm_order:run(role,f'lazy-{role}-warm-{round_number}',warm_from=f'lazy-{role}-cold-{round_number}')
    for role in PRODUCERS:run(role,'lazy-'+role+'-verified',comparison=True)
    summary=summarize();(WORK/'summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8',newline='\n');print(json.dumps(summary,indent=2),flush=True)
