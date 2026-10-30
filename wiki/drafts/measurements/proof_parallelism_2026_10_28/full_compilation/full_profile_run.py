from pathlib import Path
import hashlib,importlib.util,json,os,re,subprocess
ROOT=next(p for p in Path(__file__).resolve().parents if (p/'AGENTS.md').is_file())
spec=importlib.util.spec_from_file_location('cache_harness',ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/cache_diagnostics/cache_diagnostics_measure.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
runner=m.parent.runner;OUT=runner.OUT;WORK=OUT/'full-compilation'
IMAGE='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
SCHEDULE=[('baseline','full-original-c',False),('full-profile','full-off-a',False),('full-profile','full-on-a',True),
    ('full-profile','full-off-b',False),('full-profile','full-on-b',True),('baseline','full-original-d',False),
    ('full-profile','full-on-c',True),('baseline','full-original-e',False),('full-profile','full-off-c',False)]

def parse(text):
    events=[];counts=[]
    for line in text.splitlines():
        f=line.split('\t')
        if f[0]=='FULL':
            assert len(f)==6,f
            _,thread,name,start,end,end_thread=f
            start=int(start.removeprefix('start_ns='));end=int(end.removeprefix('end_ns='));end_thread=end_thread.removeprefix('end_thread=')
            assert start<=end and thread==end_thread,(name,thread,end_thread)
            events.append(dict(thread=thread,name=name,start=start,end=end))
        elif f[0]=='FULL_COUNT':
            assert len(f)==4;counts.append(dict(thread=f[1],name=f[2],amount=int(f[3].removeprefix('amount='))))
    return events,counts

def category(name):
    if name=='cli.compile':return 'other CLI work'
    if name in ['app.build','app.requested_product','app.package_product_lifetime']:return 'package orchestration and cleanup'
    if name=='app.source_discovery':return 'source discovery and Build execution'
    if name=='app.prepare_project':return 'project preparation'
    if name=='app.publication':return 'native publication'
    if name=='cache.source_graph.store':return 'retained graph persistence'
    if name=='cache.source_graph.restore_request':return 'retained graph restore requests'
    if name=='06.0.e_produce_native_product':return 'native production'
    if name=='psi.source.prepare':return 'source and frontend preparation'
    if name=='05' or name.startswith(('05.c.','05.d.')):return 'typed preparation and checking'
    if name=='05.lower_terminal_selection' or name.startswith(('05.01','05.10')):return 'Terminal lowering'
    return None

def partition(events):
    roots=[e for e in events if e['name']=='cli.compile'];assert len(roots)==1
    root=roots[0];a,b=root['start'],root['end']
    assert all(a<=e['start']<=e['end']<=b for e in events), 'event escaped CLI root'
    anchors=[(e,category(e['name'])) for e in events if category(e['name'])]
    points=sorted({a,b}|{e['start'] for e,_ in anchors}|{e['end'] for e,_ in anchors})
    totals={};cells=[]
    for left,right in zip(points,points[1:]):
        if left==right:continue
        active=[(e,c) for e,c in anchors if e['start']<=left and right<=e['end']]
        leaves=[(e,c) for e,c in active if not any(e['start']<=o['start'] and o['end']<=e['end'] and (e['start'],e['end'])!=(o['start'],o['end']) for o,_ in active)]
        cats=sorted({c for _,c in leaves});assert cats
        key=cats[0] if len(cats)==1 else 'overlap: '+' + '.join(cats)
        totals[key]=totals.get(key,0)+right-left;cells.append(dict(start=left,end=right,category=key))
    assert sum(totals.values())==b-a
    return dict(cli_ns=b-a,disjoint_ns=totals,cells=cells)

def self_check():
    def event(name,a,b):return dict(thread='t',name=name,start=a,end=b)
    p=partition([event('cli.compile',0,100),event('psi.source.prepare',10,70),event('05',20,50),event('06.0.e_produce_native_product',70,100)])
    assert p['disjoint_ns']=={'other CLI work':10,'source and frontend preparation':30,'typed preparation and checking':30,'native production':30}
    p=partition([event('cli.compile',0,100),event('05',10,60),event('06.0.e_produce_native_product',40,80)])
    assert p['disjoint_ns']=={'other CLI work':30,'typed preparation and checking':30,'overlap: native production + typed preparation and checking':20,'native production':20}
    assert parse('ordinary output')==([],[])
self_check()

def legacy(text):
    return [dict(ms=float(x[1]),calls=int(x[2]),name=x[3]) for x in re.finditer(r'^\s*([0-9.]+) ms\s+(\d+) x\s+(.+)$',text,re.M)]

def run(producer,label,on=False,comparison=False,warm_from=None):
    for flag in list(os.environ):
        if flag.startswith('OMEGA_') and any(k in flag for k in ['PROBE','PROFILE','COUNTS','MEASUREMENT','EDGE_CACHE']):os.environ.pop(flag,None)
    if on:os.environ['OMEGA_FULL_COMPILE_PROFILE']='1'
    saved_appdata=os.environ.get('LOCALAPPDATA')
    if warm_from:os.environ['LOCALAPPDATA']=str(OUT/'private-cache'/warm_from)
    try:
        runner.verify_inputs();runner.measure(producer,label,'off',0,comparison=comparison,phase_profile=False,cold_cache=not comparison and not warm_from);runner.verify_inputs()
    finally:
        if saved_appdata is None:os.environ.pop('LOCALAPPDATA',None)
        else:os.environ['LOCALAPPDATA']=saved_appdata
    folder=OUT/label;r=json.loads((folder/'result.json').read_text());text=(folder/'stderr.txt').read_text()
    events,counts=parse(text);r.update(full_profile=on,full_events=events,full_counts=counts)
    if on:
        assert events and not legacy(text)
        r['full_partition']=partition(events);assert r['full_partition']['cli_ns']/1e9<=r['wall_seconds']
    else:assert not events and not counts
    if comparison:
        kept=Path(re.search(r'build dir kept: (.+)',text)[1].strip());stdout=(folder/'stdout.txt').read_bytes()
        assert stdout==(kept/'terminal.stdout').read_bytes() and len(stdout)==463
        assert hashlib.sha256((kept/'omega-program.exe').read_bytes()).hexdigest()==IMAGE
        (folder/'terminal.stdout').write_bytes(stdout);r.update(terminal_stdout_agrees=True,retained_image_sha256=IMAGE,guest_stdout_sha256=hashlib.sha256(stdout).hexdigest())
    else:assert r['image_sha256']==IMAGE
    if warm_from:
        r.update(cache_mode='warm reuse of prior private cache',warm_cache_from=warm_from)
        r['warm_graph_loaded']=any(c['name']=='source.graph.loaded' and c['amount']>0 for c in counts)
    (folder/'result.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8',newline='\n')
    print('FULL_SUMMARY',label,r['wall_seconds'],r['cpu_seconds'],r.get('full_partition',{}).get('disjoint_ns'),counts,flush=True)
    return r

if __name__=='__main__':
    identities=[]
    for producer in ['baseline','full-profile']:
        p=OUT/'producers'/producer;profile=json.loads(subprocess.check_output([str(p/'omega.exe'),'--build-profile']));assert int(profile['opt_level'])>0
        identities.append(dict(producer=producer,compiler_sha256=hashlib.sha256((p/'omega.exe').read_bytes()).hexdigest(),accepted_lock_sha256=hashlib.sha256((p/'omega.lock').read_bytes()).hexdigest(),build_profile=profile))
    expected=json.loads((ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/identity.json').read_text())['producers']['baseline']
    assert identities[0]['compiler_sha256']==expected['sha256'] and identities[0]['build_profile']==expected['profile']
    (WORK/'identities.json').write_text(json.dumps(identities,indent=2)+'\n',encoding='utf-8',newline='\n')
    for case in SCHEDULE:run(*case)
    run('full-profile','full-warm-a',True,warm_from='full-on-a')
    run('full-profile','full-verified',comparison=True)
    summary={}
    for label in ['full-original-a','full-detail-a','full-original-b']+[c[1] for c in SCHEDULE]+['full-warm-a','full-verified']:
        v=json.loads((OUT/label/'result.json').read_text());summary[label]={k:v[k] for k in ['producer','wall_seconds','cpu_seconds','peak_working_set_bytes','peak_commit_bytes','cache_mode','phase_profile']}
        if 'full_partition' in v:summary[label]['cli_ns']=v['full_partition']['cli_ns'];summary[label]['disjoint_ns']=v['full_partition']['disjoint_ns']
        if 'warm_graph_loaded' in v:summary[label]['warm_graph_loaded']=v['warm_graph_loaded']
    (WORK/'summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8',newline='\n')
