"""One frozen compiler and accepted lock, two graph-initialization modes."""
from pathlib import Path
import importlib.util,json,os,platform,random,re,shutil,subprocess,sys
ROOT=next(p for p in Path(__file__).resolve().parents if (p/'AGENTS.md').is_file())
LEDGER=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28'
spec=importlib.util.spec_from_file_location('cold',LEDGER/'cold_variation/run.py')
cold=importlib.util.module_from_spec(spec);spec.loader.exec_module(cold)
runner=cold.p.runner;OUT=runner.OUT;WORK=OUT/'same-binary-graph';PRODUCER='same-binary-graph'
IMAGE='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'

def schedule():
    orders=['ELLE']*3+['LEEL']*3;random.Random(20261031).shuffle(orders);return orders

def graph_counts(text):
    ready=[];builds=[]
    for line in text.splitlines():
        f=line.split('\t')
        if f[0]=='SAME_GRAPH_READY':assert len(f)==3;ready.append([f[1],int(f[2])])
        elif f[0]=='SAME_GRAPH_BUILD':assert len(f)==3;builds.append([int(f[1]),int(f[2])])
    return dict(ready=ready,builds=builds)

def run(mode,label,warm_from=None,counts=False,comparison=False):
    assert mode in ['eager','lazy']
    env_saved=os.environ.copy()
    try:
        for flag in list(os.environ):
            if flag.startswith('OMEGA_') and any(x in flag for x in ['PROBE','PROFILE','COUNTS','MEASUREMENT','EXPERIMENT','EDGE_CACHE']):os.environ.pop(flag,None)
        os.environ['OMEGA_REFERENCE_GRAPH_EXPERIMENT_MODE']=mode
        if counts:os.environ['OMEGA_REFERENCE_GRAPH_EXPERIMENT_COUNTS']='1'
        if warm_from:os.environ['LOCALAPPDATA']=str(OUT/'private-cache'/warm_from)
        runner.verify_inputs();runner.measure(PRODUCER,label,'off',0,comparison=comparison,phase_profile=False,cold_cache=not warm_from and not comparison);runner.verify_inputs()
    finally:
        os.environ.clear();os.environ.update(env_saved)
    folder=OUT/label;record=json.loads((folder/'result.json').read_text());stderr=(folder/'stderr.txt').read_text()
    assert not any(line.startswith(('RECONCILE\t','REFERENCE_','SOURCE_QUERY_','FULL\t','LAZY_REFERENCE_')) for line in stderr.splitlines())
    counted=graph_counts(stderr)
    if counts:
        assert counted==dict(ready=[[mode,202]]*4,builds=[[202,202]]*3 if mode=='eager' else []),(label,counted)
    else:assert counted==dict(ready=[],builds=[])
    expected=(LEDGER/'lazy_reference_graph/raw/lazy-original-cold-0/runtime/stdout.txt').read_bytes()
    if comparison:
        kept=Path(re.search(r'build dir kept: (.+)',stderr)[1].strip());stdout=(folder/'stdout.txt').read_bytes()
        assert stdout==(kept/'terminal.stdout').read_bytes()==expected and runner.digest(kept/'omega-program.exe')==IMAGE
        shutil.copyfile(kept/'terminal.stdout',folder/'terminal.stdout');record.update(terminal_stdout_agrees=True,retained_image_sha256=IMAGE)
    else:
        assert record['image_sha256']==IMAGE and (folder/'runtime/stdout.txt').read_bytes()==expected
        record.update(**cold.phases(stderr))
    record.update(graph_mode=mode,graph_counts=counted,counts_enabled=counts,warm_cache_from=warm_from,comparison=comparison)
    if warm_from:record['cache_mode']='warm reuse of own mode cold private cache'
    (folder/'result.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n')
    print('SAME',label,'wall',round(record['wall_seconds'],3),'cpu',round(record['cpu_seconds'],3),'checking',round(record.get('checking_seconds',0),3),'lowering',round(record.get('lowering_seconds',0),3),'host%',round(record['external_busy_capacity_percent'],1),flush=True)
    return record

def main():
    cold.self_check();assert 0<os.cpu_count()<=64
    identity=json.loads((WORK/'producer.json').read_text());folder=OUT/'producers'/PRODUCER
    assert runner.digest(folder/'omega.exe')==identity['compiler_sha256'] and runner.digest(folder/'omega.lock')==identity['accepted_lock_sha256']
    assert json.loads(subprocess.check_output([str(folder/'omega.exe'),'--build-profile']))==identity['build_profile']
    metadata=dict(base=json.loads((WORK/'source.json').read_text())['base'],logical_processors=os.cpu_count(),platform=platform.platform(),python=sys.version,
        cold_schedule=schedule(),warm_orders=['EL' if b in [0,3,4] else 'LE' for b in range(6)],producer=identity,input_manifest_sha256=runner.digest(OUT/'input-manifest.json'),sampling_interval_seconds=.25)
    (WORK/'identity.json').write_text(json.dumps(metadata,indent=2)+'\n',encoding='utf-8',newline='\n');shutil.copyfile(OUT/'input-manifest.json',WORK/'input-manifest.json')
    runner.invoke=cold.invoke
    for mode in ['eager','lazy']:
        label=f'sbg-count-{mode}-cold';run(mode,label,counts=True);run(mode,f'sbg-count-{mode}-warm',warm_from=label,counts=True)
    for block,order in enumerate(schedule()):
        seeds={}
        for position,symbol in enumerate(order):
            mode='eager' if symbol=='E' else 'lazy';label=f'sbg-cold-{block}-{position}-{mode}'
            r=run(mode,label);r.update(block=block,position=position,order=order,series='cold');seeds[mode]=label
            (OUT/label/'result.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8',newline='\n')
        for position,symbol in enumerate(metadata['warm_orders'][block]):
            mode='eager' if symbol=='E' else 'lazy';label=f'sbg-warm-{block}-{mode}'
            r=run(mode,label,warm_from=seeds[mode]);r.update(block=block,position=position,order=metadata['warm_orders'][block],series='warm')
            (OUT/label/'result.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8',newline='\n')
    for mode in ['eager','lazy']:run(mode,f'sbg-both-{mode}',comparison=True)

def self_check():
    cold.self_check();assert schedule().count('ELLE')==schedule().count('LEEL')==3
    assert all(sum(i for i,c in enumerate(o) if c=='E')==sum(i for i,c in enumerate(o) if c=='L')==3 for o in schedule())
    assert graph_counts('SAME_GRAPH_READY\teager\t202\nSAME_GRAPH_BUILD\t202\t202')==dict(ready=[['eager',202]],builds=[[202,202]])
    assert graph_counts('ordinary timing')==dict(ready=[],builds=[])

if __name__=='__main__':
    if sys.argv[1:]==['--self-check']:self_check();print('Same-binary schedule, graph counters and telemetry self-checks passed.')
    else:main()
