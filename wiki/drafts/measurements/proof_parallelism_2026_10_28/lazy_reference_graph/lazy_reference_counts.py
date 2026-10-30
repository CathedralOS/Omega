from pathlib import Path
import importlib.util,json,os,re,subprocess
spec=importlib.util.spec_from_file_location('pure',Path(__file__).with_name('lazy_reference_run.py'))
p=importlib.util.module_from_spec(spec);spec.loader.exec_module(p)

def parse(text):
    ready=[];graphs=[]
    for line in text.splitlines():
        f=line.split('\t')
        if f[0]=='LAZY_REFERENCE_READY':
            assert len(f)==3;ready.append(int(f[2]))
        elif f[0]=='LAZY_REFERENCE_GRAPH_BUILD':
            assert len(f)==4;graphs.append([int(f[2]),int(f[3])])
        elif f[0]=='REFERENCE_GRAPH':
            assert len(f)==5;graphs.append([int(f[2]),int(f[3])])
    return dict(prepared_calls=ready,graph_builds=graphs)

def self_check():
    assert parse('LAZY_REFERENCE_READY\tt\t202\nLAZY_REFERENCE_GRAPH_BUILD\tt\t202\t202')==dict(prepared_calls=[202],graph_builds=[[202,202]])
    assert parse('REFERENCE_GRAPH\tt\t202\t202\t100')==dict(prepared_calls=[],graph_builds=[[202,202]])
    assert parse('ordinary output')==dict(prepared_calls=[],graph_builds=[])
self_check()

def run(producer,label,warm_from=None,comparison=False):
    for k in list(os.environ):
        if k.startswith('OMEGA_') and any(x in k for x in ['PROBE','PROFILE','COUNTS','MEASUREMENT','EDGE_CACHE']):os.environ.pop(k,None)
    if producer=='terminal-breakdown':os.environ['OMEGA_RECONCILE_PROBE']='counts'
    elif not comparison:os.environ['OMEGA_LAZY_REFERENCE_GRAPH_COUNTS']='1'
    saved=os.environ.get('LOCALAPPDATA')
    if warm_from:os.environ['LOCALAPPDATA']=str(p.OUT/'private-cache'/warm_from)
    try:
        p.runner.verify_inputs();p.runner.measure(producer,label,'off',0,comparison=comparison,phase_profile=False,cold_cache=not warm_from and not comparison);p.runner.verify_inputs()
    finally:
        if saved is None:os.environ.pop('LOCALAPPDATA',None)
        else:os.environ['LOCALAPPDATA']=saved
    folder=p.OUT/label;r=json.loads((folder/'result.json').read_text());stderr=(folder/'stderr.txt').read_text();counts=parse(stderr)
    if comparison:
        kept=Path(re.search(r'build dir kept: (.+)',stderr)[1].strip());stdout=(folder/'stdout.txt').read_bytes()
        assert stdout==(kept/'terminal.stdout').read_bytes() and len(stdout)==463 and p.runner.digest(kept/'omega-program.exe')==p.IMAGE
        (folder/'terminal.stdout').write_bytes(stdout);r.update(terminal_stdout_agrees=True,retained_image_sha256=p.IMAGE)
    else:
        assert r['image_sha256']==p.IMAGE
        if producer=='terminal-breakdown':assert counts['graph_builds']==[[202,202]]*3
        else:
            assert counts['prepared_calls'] and all(x==202 for x in counts['prepared_calls'])
            assert all(g==[202,202] for g in counts['graph_builds'])
            assert len(counts['graph_builds'])<=len(counts['prepared_calls'])
    r.update(graph_counts=counts,cache_mode='warm reuse of own cold private cache' if warm_from else r['cache_mode'],warm_cache_from=warm_from)
    (folder/'result.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8',newline='\n');print(label,counts,flush=True)

if __name__=='__main__':
    identities=[]
    for name in ['terminal-breakdown','lazy-reference-probe']:
        d=p.OUT/'producers'/name;profile=json.loads(subprocess.check_output([str(d/'omega.exe'),'--build-profile']));assert int(profile['opt_level'])>0
        identities.append(dict(producer=name,compiler_sha256=p.runner.digest(d/'omega.exe'),accepted_lock_sha256=p.runner.digest(d/'omega.lock'),build_profile=profile))
    expected=json.loads((p.RECORD/'terminal_reconciliation/identities.json').read_text())[1]
    assert identities[0]==expected
    (p.WORK/'counter-identities.json').write_text(json.dumps(identities,indent=2)+'\n',encoding='utf-8',newline='\n')
    for role,producer in [('original','terminal-breakdown'),('candidate','lazy-reference-probe')]:
        label='lazy-count-'+role+'-cold';run(producer,label)
        run(producer,'lazy-count-'+role+'-warm',warm_from=label)
    run('lazy-reference-probe','lazy-count-verified',comparison=True)
