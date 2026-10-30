from pathlib import Path
from collections import Counter
import hashlib,importlib.util,json,os,re,subprocess
ROOT=next(p for p in Path(__file__).resolve().parents if (p/'AGENTS.md').is_file())
RECORD=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28'
spec=importlib.util.spec_from_file_location('cache',RECORD/'cache_diagnostics/cache_diagnostics_measure.py')
h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
runner=h.parent.runner;OUT=runner.OUT;WORK=OUT/'terminal-breakdown'
IMAGE='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'

def parse(text):
    active={};windows=Counter();details={k:Counter() for k in ['SOURCE','REFERENCE','unscoped']}
    graphs=[];demands=[];cache=Counter()
    for line in text.splitlines():
        f=line.split('\t')
        if f[0] in ['SOURCE_QUERY_BEGIN','REFERENCE_QUERY_BEGIN']:
            kind=f[0].split('_')[0];assert f[1] not in active
            active[f[1]]=kind;windows[kind]+=1
        elif f[0] in ['SOURCE_QUERY_END','REFERENCE_QUERY_END']:
            assert active.pop(f[1])==f[0].split('_')[0]
        elif f[0]=='REFERENCE_GRAPH':
            assert len(f)==5
            graphs.append(dict(scope=active.get(f[1],'unscoped'),module_machines=int(f[2]),graph_machines=int(f[3]),ns=int(f[4])))
        elif f[0]=='REFERENCE_DEMAND':
            assert len(f)==5
            demands.append(dict(scope=active.get(f[1],'unscoped'),mappings=int(f[2]),shared_leaves=int(f[3]),projected_candidates=int(f[4])))
        elif f[0]=='SOURCE_GRAPH_CACHE':cache[f[1]]+=1
        elif f[0]=='REPLAY_DETAIL':
            c=details[active.get(f[1],'unscoped')];c['queries']+=1
            for item in f[2:]:
                key,value=item.split('=');c[key]+=int(value)
        elif f[0] in ['ENTRY_INIT','ENTRY_PREVIOUS_DROP']:
            c=details[active.get(f[1],'unscoped')];c[f[0]+'_calls']+=1;c[f[0]+'_ns']+=int(f[2])
    assert not active
    rows=h.parent.parse_probe(text)
    if rows:
        rosters=[[r['machines'] for r in rows if r['operation']==kind] for kind in ['places','residences','memberships']]
        assert rosters[0]==rosters[1]==rosters[2]
    totals=Counter();parts=Counter()
    for row in rows:
        totals[row['operation']]+=row['wall_ns']
        if row['operation']=='memberships':
            for k,v in row['metrics'].items():parts[k]+=v
            assert sum(v for k,v in row['metrics'].items() if k.endswith('_ns'))<=row['wall_ns']
    if rows:
        parts['residual_ns']=totals['memberships']-sum(v for k,v in parts.items() if k.endswith('_ns'))
        graph_ns=sum(g['ns'] for g in graphs if g['scope']=='REFERENCE')
        assert graph_ns<=parts['reference_queries_ns']
        parts['reference_graph_ns']=graph_ns
        parts['reference_other_ns']=parts['reference_queries_ns']-graph_ns
        for kind,parent in [('SOURCE','source_contract_queries_ns'),('REFERENCE','reference_other_ns')]:
            c=details[kind]
            if c:
                disjoint=sum(c[k] for k in ['setup','clone','observe','retire','transfer','canonical','edges_inclusive','ENTRY_INIT_ns','ENTRY_PREVIOUS_DROP_ns'])
                c['disjoint_remainder_ns']=parts[parent]-disjoint
                assert c['disjoint_remainder_ns']>=0,(kind,c,parts[parent])
    result=dict(rows=rows,totals_ns=totals,membership_parts=parts,windows=windows,graphs=graphs,cache=cache,replay_details=details)
    if demands:result['demands']=demands
    return result

def self_check():
    t='REFERENCE_QUERY_BEGIN\tt\nREFERENCE_GRAPH\tt\t3\t3\t10\nREFERENCE_QUERY_END\tt\nSOURCE_GRAPH_CACHE\tloaded\n'
    r=parse(t);assert r['graphs']==[dict(scope='REFERENCE',module_machines=3,graph_machines=3,ns=10)] and r['cache']=={'loaded':1}
    try:parse('SOURCE_QUERY_BEGIN\tt\nREFERENCE_QUERY_END\tt\n')
    except AssertionError:pass
    else:raise AssertionError('mismatched window accepted')
    assert not parse('ordinary stdout')['graphs']
    assert parse('REFERENCE_QUERY_BEGIN\tt\nREFERENCE_DEMAND\tt\t2\t0\t0\nREFERENCE_QUERY_END\tt')['demands']==[dict(scope='REFERENCE',mappings=2,shared_leaves=0,projected_candidates=0)]
self_check()

def run(producer,label,mode='off',detail=False,warm_from=None,comparison=False):
    for flag in list(os.environ):
        if flag.startswith('OMEGA_') and any(k in flag for k in ['PROBE','PROFILE','COUNTS','MEASUREMENT','EDGE_CACHE']):os.environ.pop(flag,None)
    os.environ['OMEGA_RECONCILE_PROBE']=mode
    if detail:os.environ['OMEGA_PAYLOAD_ENTRY_PROFILE']='1'
    saved=os.environ.get('LOCALAPPDATA')
    if warm_from:os.environ['LOCALAPPDATA']=str(OUT/'private-cache'/warm_from)
    try:
        runner.verify_inputs();runner.measure(producer,label,'off',0,comparison=comparison,cold_cache=not warm_from and not comparison);runner.verify_inputs()
    finally:
        if saved is None:os.environ.pop('LOCALAPPDATA',None)
        else:os.environ['LOCALAPPDATA']=saved
    return finish(label,mode,detail,warm_from,comparison)

def finish(label,mode,detail=False,warm_from=None,comparison=False):
    folder=OUT/label;r=json.loads((folder/'result.json').read_text());stderr=(folder/'stderr.txt').read_text();p=parse(stderr)
    r.update(reconciliation_mode=mode,entry_detail=detail,attribution=p)
    if mode=='off':assert not p['rows'] and not p['cache'] and not p['graphs']
    else:
        assert p['rows'] and p['windows'].get('SOURCE',0)==(418 if detail else 0) and p['windows']['REFERENCE']==418
        assert p['cache']==({'loaded':1,'prepared_hit':2} if warm_from else {'cold':1})
    if detail:
        assert p['replay_details']['SOURCE']['queries']==173
        assert p['replay_details']['SOURCE']['steps']==15219
        assert p['replay_details']['SOURCE']['edges']==17444
    if comparison:
        kept=Path(re.search(r'build dir kept: (.+)',stderr)[1].strip());stdout=(folder/'stdout.txt').read_bytes()
        assert stdout==(kept/'terminal.stdout').read_bytes() and len(stdout)==463
        assert hashlib.sha256((kept/'omega-program.exe').read_bytes()).hexdigest()==IMAGE
        (folder/'terminal.stdout').write_bytes(stdout);r.update(terminal_stdout_agrees=True,retained_image_sha256=IMAGE)
    else:assert r['image_sha256']==IMAGE
    if warm_from:r.update(cache_mode='warm reuse of own private cache',warm_cache_from=warm_from)
    (folder/'result.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8',newline='\n')
    print('BREAKDOWN',label,round(r['wall_seconds'],3),p['totals_ns'],p['membership_parts'],p['graphs'],p['replay_details']['REFERENCE'],flush=True)
    return r

if __name__=='__main__':
    identities=[]
    for name in ['baseline','terminal-breakdown']:
        p=OUT/'producers'/name;profile=json.loads(subprocess.check_output([str(p/'omega.exe'),'--build-profile']));assert int(profile['opt_level'])>0
        identities.append(dict(producer=name,compiler_sha256=runner.digest(p/'omega.exe'),accepted_lock_sha256=runner.digest(p/'omega.lock'),build_profile=profile))
    expected=json.loads((RECORD/'identity.json').read_text())['producers']['baseline'];assert identities[0]['compiler_sha256']==expected['sha256'] and identities[0]['build_profile']==expected['profile']
    (WORK/'identities.json').write_text(json.dumps(identities,indent=2)+'\n',encoding='utf-8',newline='\n')
    run('baseline','term-original-a')
    run('terminal-breakdown','term-off-a')
    run('terminal-breakdown','term-cold-a','timing')
    run('terminal-breakdown','term-warm-a','timing',warm_from='term-cold-a')
    for letter in 'bc':
        run('terminal-breakdown','term-cold-'+letter,'timing',True)
        run('terminal-breakdown','term-warm-'+letter,'timing',True,warm_from='term-cold-'+letter)
        if letter=='b':run('baseline','term-original-b');run('terminal-breakdown','term-off-b')
    run('terminal-breakdown','term-counts-cold','counts')
    run('terminal-breakdown','term-counts-warm','counts',warm_from='term-counts-cold')
    run('terminal-breakdown','term-verified',comparison=True)
