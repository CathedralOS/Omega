"""Process CPU at existing phase boundaries; endpoint-only external accounting."""
from pathlib import Path
import ctypes,gzip,hashlib,importlib.util,json,os,platform,random,shutil,subprocess,sys,time
ROOT=next(p for p in Path(__file__).resolve().parents if (p/'AGENTS.md').is_file())
LEDGER=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28'
spec=importlib.util.spec_from_file_location('same',LEDGER/'same_binary_graph/run.py')
same=importlib.util.module_from_spec(spec);spec.loader.exec_module(same)
runner=same.runner;OUT=runner.OUT;WORK=OUT/'graph-phase-cpu';PRODUCER='graph-phase-cpu'

def invoke(command,folder,environment,timeout=600):
    folder.mkdir(parents=True,exist_ok=False);before=same.cold.system();started=time.perf_counter()
    process=subprocess.Popen([str(x) for x in command],cwd=ROOT,env=environment,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:stdout,stderr=process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:process.kill();process.communicate();raise
    elapsed=time.perf_counter()-started;created,exited,kernel,user=(ctypes.c_uint64() for _ in range(4))
    assert same.cold.K.GetProcessTimes(ctypes.c_void_p(int(process._handle)),ctypes.byref(created),ctypes.byref(exited),ctypes.byref(kernel),ctypes.byref(user))
    after=same.cold.system();cpu=(kernel.value+user.value)/1e7
    d={k:after[k]-before[k] for k in before};capacity=d['kernel']+d['user'];busy=capacity-d['idle'];external=busy-cpu
    record=dict(command=[str(x) for x in command],wall_seconds=elapsed,cpu_seconds=cpu,kernel_seconds=kernel.value/1e7,user_seconds=user.value/1e7,exit_code=process.returncode,
        system_before=before,system_after=after,system_cpu_capacity_seconds=capacity,system_busy_seconds=busy,external_busy_seconds=external,
        external_busy_capacity_percent=100*external/capacity if capacity else None,observer='endpoints only')
    (folder/'stdout.txt').write_bytes(stdout);(folder/'stderr.txt').write_bytes(stderr)
    (folder/'result.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n')
    if process.returncode:raise RuntimeError(stderr.decode('utf-8','replace')[-5000:])
    return subprocess.CompletedProcess(command,process.returncode,stdout,stderr),record

def parse(text):
    windows=[]
    for line in text.splitlines():
        fields=line.split('\t')
        if fields[0]!='GRAPH_CPU':continue
        assert len(fields)==9,fields
        stage,thread=fields[1:3];a,b,k0,u0,k1,u1=map(int,fields[3:])
        assert b>=a and k1>=k0 and u1>=u0
        windows.append(dict(stage=stage,thread=thread,start_ns=a,end_ns=b,start_cpu_ticks=k0+u0,end_cpu_ticks=k1+u1,
            kernel_ticks=k1-k0,user_ticks=u1-u0,cpu_seconds=(k1+u1-k0-u0)/1e7,wall_seconds=(b-a)/1e9))
    return windows

def partition(windows,total):
    grouped={name:[w for w in windows if w['stage']==name] for name in ['Stage 05','terminal-production','terminal-production/lowering']}
    assert not grouped['terminal-production'] and len(grouped['terminal-production/lowering'])==1,grouped
    lower=grouped['terminal-production/lowering'][0]
    checks=sorted(grouped['Stage 05'],key=lambda w:w['start_ns']);assert checks
    assert all(w['end_ns']<=lower['start_ns'] for w in checks)
    assert all(a['end_ns']<=b['start_ns'] and a['end_cpu_ticks']<=b['start_cpu_ticks'] for a,b in zip(checks,checks[1:]))
    checking=sum(w['cpu_seconds'] for w in checks);before=lower['start_cpu_ticks']/1e7;after=total-lower['end_cpu_ticks']/1e7
    assert before>=checking and after>=0
    regions=dict(before_lowering_cpu=before,lowering_cpu=lower['cpu_seconds'],after_lowering_cpu=after,
        checking_cpu=checking,other_before_lowering_cpu=before-checking)
    assert abs(regions['before_lowering_cpu']+regions['lowering_cpu']+regions['after_lowering_cpu']-total)<1e-8
    return regions

def schedule():
    orders=['ELLE']*3+['LEEL']*3;random.Random(20261101).shuffle(orders);return orders

def run(mode,label,counts=False,comparison=False):
    if comparison:os.environ.pop('OMEGA_GRAPH_PHASE_CPU',None)
    else:os.environ['OMEGA_GRAPH_PHASE_CPU']='1'
    r=same.run(mode,label,counts=counts,comparison=comparison);folder=OUT/label;windows=parse((folder/'stderr.txt').read_text())
    if comparison:assert not windows
    else:r['cpu_partition']=partition(windows,r['cpu_seconds'])
    r['cpu_windows']=windows;(folder/'result.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8',newline='\n')
    if not comparison:print('PARTITION',label,r['cpu_partition'],flush=True)
    return r

def self_check():
    assert schedule().count('ELLE')==schedule().count('LEEL')==3
    assert all(sum(i for i,c in enumerate(o) if c=='E')==sum(i for i,c in enumerate(o) if c=='L')==3 for o in schedule())
    text='GRAPH_CPU\tStage 05\tt\t1\t3\t1\t0\t3\t0\nGRAPH_CPU\tterminal-production/lowering\tt\t5\t7\t5\t0\t7\t0'
    value=partition(parse(text),10/1e7);assert value['checking_cpu']==2/1e7 and value['lowering_cpu']==2/1e7
    assert parse('ordinary output')==[]

def main():
    self_check();folder=OUT/'producers'/PRODUCER
    identity=dict(producer=PRODUCER,compiler_sha256=runner.digest(folder/'omega.exe'),accepted_lock_sha256=runner.digest(folder/'omega.lock'),build_profile=json.loads(subprocess.check_output([str(folder/'omega.exe'),'--build-profile'])))
    assert int(identity['build_profile']['opt_level'])>0
    (WORK/'producer.json').write_text(json.dumps(identity,indent=2)+'\n',encoding='utf-8',newline='\n')
    metadata=dict(base=json.loads((WORK/'source.json').read_text())['base'],producer=identity,schedule=schedule(),logical_processors=os.cpu_count(),platform=platform.platform(),input_manifest_sha256=runner.digest(OUT/'input-manifest.json'),observer='endpoints only')
    (WORK/'identity.json').write_text(json.dumps(metadata,indent=2)+'\n',encoding='utf-8',newline='\n');shutil.copyfile(OUT/'input-manifest.json',WORK/'input-manifest.json')
    same.PRODUCER=PRODUCER;runner.invoke=invoke
    for mode in ['eager','lazy']:
        label=f'gpc-count-{mode}'
        if mode=='eager' and (OUT/label/'result.json').is_file():
            r=json.loads((OUT/label/'result.json').read_text());text=(OUT/label/'stderr.txt').read_text()
            assert r['exit_code']==0 and r['counts_enabled'] and r['graph_counts']==dict(ready=[['eager',202]]*4,builds=[[202,202]]*3)
            r['cpu_windows']=parse(text);r['cpu_partition']=partition(r['cpu_windows'],r['cpu_seconds'])
            (OUT/label/'result.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8',newline='\n')
            print('Recovered successful eager preflight after correcting outer-timer assumption; no compiler rerun.',flush=True)
        else:run(mode,label,counts=True)
    for block,order in enumerate(schedule()):
        for position,symbol in enumerate(order):
            mode='eager' if symbol=='E' else 'lazy';label=f'gpc-{block}-{position}-{mode}';r=run(mode,label)
            r.update(block=block,position=position,order=order);(OUT/label/'result.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8',newline='\n')
    for mode in ['eager','lazy']:run(mode,f'gpc-both-{mode}',comparison=True)

if __name__=='__main__':
    if sys.argv[1:]==['--self-check']:self_check();print('CPU boundary parser, nested/disjoint partition closure and balanced schedule checked.')
    else:main()
