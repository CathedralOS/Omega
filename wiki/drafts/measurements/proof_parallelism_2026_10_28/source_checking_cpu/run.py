"""Bounded source-checking attribution, preserving every observation."""
from pathlib import Path
import gzip,hashlib,importlib.util,json,os,platform,subprocess,sys
ROOT=next(p for p in Path(__file__).resolve().parents if (p/'AGENTS.md').is_file())
LEDGER=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28'
spec=importlib.util.spec_from_file_location('endpoint',LEDGER/'graph_phase_cpu/run.py')
endpoint=importlib.util.module_from_spec(spec);spec.loader.exec_module(endpoint)
OUT=ROOT/'build/proof-profile';WORK=OUT/'source-checking-cpu';PRODUCER=OUT/'producers/source-checking-cpu'
PROGRAM=ROOT/'samples/cli/games/dungeon_crawler_cli/main.omg'
ORACLE=b'A bottomless dark room near the dungeon heart.'
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,data):p.write_text(json.dumps(data,indent=2)+'\n',encoding='utf-8',newline='\n')
def cache_state(path):
    files=[p for p in path.rglob('*') if p.is_file()]
    return dict(files=len(files),bytes=sum(p.stat().st_size for p in files))
def manifest():
    names=subprocess.check_output(['git','ls-files','--','samples/cli/games/dungeon_crawler_cli','source/library'],cwd=ROOT,text=True).splitlines()
    return {n:digest(ROOT/n) for n in names}
def parse(text):
    windows=[]
    for line in text.splitlines():
        f=line.split('\t')
        if f[0]!='CHECK_CPU':continue
        assert len(f)==14,f
        name,thread=f[1:3];id,a,b,pk0,pu0,tk0,tu0,pk1,pu1,tk1,tu1=map(int,f[3:])
        assert a<=b and all(x<=y for x,y in [(pk0,pk1),(pu0,pu1),(tk0,tk1),(tu0,tu1)])
        windows.append(dict(name=name,thread=thread,id=id,start_ns=a,end_ns=b,p0=pk0+pu0,p1=pk1+pu1,t0=tk0+tu0,t1=tk1+tu1,
            process_cpu=(pk1+pu1-pk0-pu0)/1e7,thread_cpu=(tk1+tu1-tk0-tu0)/1e7,kernel_cpu=(pk1-pk0)/1e7,wall=(b-a)/1e9))
    return windows
def partition(windows):
    roots=[w for w in windows if w['name']=='05'];assert roots
    for a,b in zip(sorted(roots,key=lambda w:w['start_ns']),sorted(roots,key=lambda w:w['start_ns'])[1:]):assert a['end_ns']<=b['start_ns']
    nodes=[];outside=[]
    for w in windows:
        containing=[r for r in roots if r['start_ns']<=w['start_ns'] and r['end_ns']>=w['end_ns']]
        if not containing:outside.append(w);continue
        assert len(containing)==1
        # Any cross-thread selected overlap would make process-CPU subtraction ambiguous.
        assert w['thread']==containing[0]['thread'],w
        parents=[p for p in windows if p['thread']==w['thread'] and p['start_ns']<=w['start_ns'] and p['end_ns']>=w['end_ns'] and p['id']!=w['id']]
        parent=min(parents,key=lambda p:p['end_ns']-p['start_ns']) if parents else None
        nodes.append(dict(w,parent=parent['id'] if parent else None))
    for n in nodes:
        children=sorted([w for w in nodes if w['parent']==n['id']],key=lambda w:w['start_ns'])
        assert all(a['end_ns']<=b['start_ns'] for a,b in zip(children,children[1:])),children
        for field in ['process_cpu','thread_cpu','wall']:
            residual=n[field]-sum(c[field] for c in children)
            assert residual>=-1e-7,(n,field,residual)
            n['self_'+field]=residual
    for field in ['process_cpu','thread_cpu','wall']:
        assert abs(sum(n['self_'+field] for n in nodes)-sum(r[field] for r in roots))<1e-6
    return dict(roots=len(roots),process_cpu=sum(r['process_cpu'] for r in roots),thread_cpu=sum(r['thread_cpu'] for r in roots),wall=sum(r['wall'] for r in roots),nodes=nodes,outside=outside)
def self_check():
    def row(name,id,a,b,p0,p1):return dict(name=name,thread='t',id=id,start_ns=a,end_ns=b,p0=p0,p1=p1,process_cpu=p1-p0,thread_cpu=p1-p0,wall=b-a)
    p=partition([row('05',0,0,10,0,5),row('a',1,1,4,1,3),row('b',2,5,8,3,4)])
    assert p['process_cpu']==5 and p['nodes'][0]['self_process_cpu']==2
    try:partition([row('05',0,0,10,0,5),row('a',1,1,6,1,3),row('b',2,5,8,3,4)])
    except AssertionError:pass
    else:raise AssertionError('overlap accepted')
    assert parse('normal output')==[]
def run(label,on,cold=True,warm_from=None,both=False):
    assert manifest()==json.loads((WORK/'input-manifest.json').read_text())['inputs'],'source drift'
    exe=PRODUCER/'omega.exe';cache=OUT/'private-cache'/(warm_from or label)
    if cold:cache.mkdir(parents=True,exist_ok=False)
    assert cache.is_dir()
    before=cache_state(cache)
    if cold:assert before['files']==0
    env=os.environ.copy();env.update(OMEGA_SOURCE_CHECK_CPU='1' if on else '0',OMEGA_PHASE_PROFILE='0',OMEGA_LIBRARY_ROOT=str(ROOT/'source/library'),LOCALAPPDATA=str(cache),OMEGA_CHECK_MEASUREMENT='off')
    (PROGRAM.parent/'omega.lock').write_bytes((PRODUCER/'omega.lock').read_bytes())
    folder=OUT/label
    command=[exe,'run','--both','--keep',PROGRAM] if both else [exe,'--timings','--target','windows_x86_64','--build-dir',folder/'native',PROGRAM]
    completed,result=endpoint.invoke(command,folder,env)
    result.update(label=label,probe_enabled=on,cache_mode='fresh private cache' if cold else 'reused own private cache',cache_source=warm_from,compiler_sha256=digest(exe),accepted_lock_sha256=digest(PRODUCER/'omega.lock'),program_sha256=digest(PROGRAM))
    result.update(cache_before=before,cache_after=cache_state(cache))
    if both:
        assert ORACLE in completed.stdout and b'DECLINED' not in completed.stdout+completed.stderr and b'interp exit: 0' in completed.stderr
    else:
        images=list((folder/'native').rglob('*.exe'));assert len(images)==1,images
        native,runtime=endpoint.invoke([images[0]],folder/'runtime',env,30)
        assert ORACLE in native.stdout
        result.update(image_sha256=digest(images[0]),stdout_sha256=hashlib.sha256(native.stdout).hexdigest(),stdout_bytes=len(native.stdout),runtime=runtime)
        prior=[json.loads(p.read_text()) for p in WORK.glob('observations/*.json') if json.loads(p.read_text()).get('image_sha256')]
        if prior:assert all(r['image_sha256']==result['image_sha256'] and r['stdout_sha256']==result['stdout_sha256'] for r in prior)
    windows=parse(completed.stderr.decode('utf-8','strict'))
    if on:result['checking']=partition(windows)
    else:assert not windows
    save(folder/'result.json',result);save(WORK/'observations'/f'{label}.json',result)
    print(json.dumps({k:result[k] for k in ['label','probe_enabled','cache_mode','wall_seconds','cpu_seconds']}),flush=True)
    if on:print('CHECK', {k:v for k,v in result['checking'].items() if k not in ['nodes','outside']},flush=True)
def main():
    self_check();(WORK/'observations').mkdir(exist_ok=False)
    save(WORK/'input-manifest.json',dict(inputs=manifest(),scope='All tracked Dungeon and bundled library files; compiler base plus diagnostic patch recorded separately'))
    profile=json.loads(subprocess.check_output([str(PRODUCER/'omega.exe'),'--build-profile']));assert int(profile['opt_level'])>0
    save(WORK/'producer.json',dict(executable_sha256=digest(PRODUCER/'omega.exe'),lock_sha256=digest(PRODUCER/'omega.lock'),build_profile=profile,platform=platform.platform(),logical_processors=os.cpu_count()))
    for i,on in enumerate([False,True,True,False,True,True]):run(f'checkcpu-cold-{i}',on)
    for i in [1,5]:run(f'checkcpu-warm-{i}',True,cold=False,warm_from=f'checkcpu-cold-{i}')
    run('checkcpu-both',False,both=True)
if __name__=='__main__':
    if sys.argv[1:]==['--self-check']:self_check();print('Disjoint partition closes; overlap refused.')
    else:main()
