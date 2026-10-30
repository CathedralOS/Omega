"""Bounded cold-only diagnosis; frozen compilers, fresh caches, same-binary controls."""
from pathlib import Path
import ctypes,gzip,hashlib,importlib.util,json,os,platform,random,re,statistics,subprocess,sys,time
ROOT=next(p for p in Path(__file__).resolve().parents if (p/'AGENTS.md').is_file())
LEDGER=ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28'
spec=importlib.util.spec_from_file_location('lazy',LEDGER/'lazy_reference_graph/lazy_reference_run.py')
p=importlib.util.module_from_spec(spec);spec.loader.exec_module(p)
WORK=p.OUT/'cold-variation'
K=ctypes.WinDLL('kernel32',use_last_error=True);PS=ctypes.WinDLL('psapi',use_last_error=True)
U64=ctypes.c_uint64;HANDLE=ctypes.c_void_p
class IO(ctypes.Structure):
    _fields_=[(n,U64) for n in ['read_operations','write_operations','other_operations','read_bytes','write_bytes','other_bytes']]
K.GetSystemTimes.argtypes=[ctypes.POINTER(U64)]*3
K.GetProcessTimes.argtypes=[HANDLE]+[ctypes.POINTER(U64)]*4
K.GetProcessIoCounters.argtypes=[HANDLE,ctypes.POINTER(IO)]
PS.GetProcessMemoryInfo.argtypes=[HANDLE,ctypes.POINTER(p.h.MemoryCounters),ctypes.c_uint32]

def system():
    idle,kernel,user=(U64() for _ in range(3))
    assert K.GetSystemTimes(ctypes.byref(idle),ctypes.byref(kernel),ctypes.byref(user)),ctypes.get_last_error()
    return dict(idle=idle.value/1e7,kernel=kernel.value/1e7,user=user.value/1e7)

def sample(handle,origin):
    begin=time.perf_counter();created,exited,kernel,user=(U64() for _ in range(4))
    assert K.GetProcessTimes(handle,ctypes.byref(created),ctypes.byref(exited),ctypes.byref(kernel),ctypes.byref(user)),ctypes.get_last_error()
    io=IO();assert K.GetProcessIoCounters(handle,ctypes.byref(io)),ctypes.get_last_error()
    memory=p.h.MemoryCounters();memory.cb=ctypes.sizeof(memory)
    memory_ok=bool(PS.GetProcessMemoryInfo(handle,ctypes.byref(memory),ctypes.sizeof(memory)))
    result=dict(t=time.perf_counter()-origin,system=system(),kernel_seconds=kernel.value/1e7,user_seconds=user.value/1e7,
        io={n:getattr(io,n) for n,_ in IO._fields_},memory=None)
    if memory_ok:result['memory']={n:getattr(memory,n) for n in ['faults','peak_working_set','peak_pagefile']}
    result['sampling_seconds']=time.perf_counter()-begin
    return result

def invoke(command,folder,environment,timeout=600):
    folder.mkdir(parents=True,exist_ok=False);samples=[]
    system_before=system();started=time.perf_counter()
    proc=subprocess.Popen([str(x) for x in command],cwd=ROOT,env=environment,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        while True:
            samples.append(sample(HANDLE(int(proc._handle)),started))
            remaining=timeout-(time.perf_counter()-started)
            if remaining<=0:raise subprocess.TimeoutExpired(command,timeout)
            try:
                stdout,stderr=proc.communicate(timeout=min(.25,remaining));break
            except subprocess.TimeoutExpired:pass
        wall=time.perf_counter()-started
        final=sample(HANDLE(int(proc._handle)),started);samples.append(final)
    except BaseException:
        if proc.poll() is None:proc.kill()
        proc.communicate();raise
    cpu=final['kernel_seconds']+final['user_seconds']
    delta={k:final['system'][k]-system_before[k] for k in system_before}
    total=delta['kernel']+delta['user'];busy=total-delta['idle'];external=busy-cpu
    assert total>0 and external>-.1 and all(v>=0 for v in delta.values())
    memory=[s['memory'] for s in samples if s['memory'] is not None];assert memory
    record=dict(command=[str(x) for x in command],wall_seconds=wall,cpu_seconds=cpu,exit_code=proc.returncode,
        kernel_seconds=final['kernel_seconds'],user_seconds=final['user_seconds'],io=final['io'],
        peak_working_set_bytes=max(m['peak_working_set'] for m in memory),peak_commit_bytes=max(m['peak_pagefile'] for m in memory),
        page_faults_observed=max(m['faults'] for m in memory),memory_final_available=final['memory'] is not None,
        system_cpu_capacity_seconds=total,system_busy_seconds=busy,external_busy_seconds=external,
        external_busy_capacity_percent=100*external/total,sampling_seconds=sum(s['sampling_seconds'] for s in samples))
    (folder/'stdout.txt').write_bytes(stdout);(folder/'stderr.txt').write_bytes(stderr)
    (folder/'telemetry.json').write_text(json.dumps(dict(system_before=system_before,samples=samples),indent=2)+'\n',encoding='utf-8',newline='\n')
    (folder/'result.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n')
    if proc.returncode!=0:raise RuntimeError(stderr.decode('utf-8','replace')[-6000:])
    return subprocess.CompletedProcess(command,proc.returncode,stdout,stderr),record

def phases(text):
    rows={name:float(ms)/1000 for ms,name in re.findall(r'^\s*([0-9.]+) ms  (.+)$',text,re.M)}
    return dict(checking_seconds=rows['Stage 05: TypedTrees -> CheckedTrees'],lowering_seconds=rows['terminal-production/lowering: CheckedTrees -> LoweredPsi'])

def schedule():
    orders=['ABBA']*3+['BAAB']*3;random.Random(20261030).shuffle(orders)
    return orders

def self_check():
    assert ctypes.sizeof(p.h.MemoryCounters)==72 and ctypes.sizeof(IO)==48
    assert schedule().count('ABBA')==schedule().count('BAAB')==3
    assert all(sum(i for i,c in enumerate(order) if c=='A')==sum(i for i,c in enumerate(order) if c=='B')==3 for order in schedule())
    a=sample(HANDLE(-1),time.perf_counter());b=sample(HANDLE(-1),time.perf_counter())
    assert b['kernel_seconds']>=a['kernel_seconds'] and b['user_seconds']>=a['user_seconds'] and b['memory']['faults']>=a['memory']['faults']
    for k in a['io']:assert b['io'][k]>=a['io'][k]
    assert phases(' 1200.000 ms  Stage 05: TypedTrees -> CheckedTrees\n 300.000 ms  terminal-production/lowering: CheckedTrees -> LoweredPsi')==dict(checking_seconds=1.2,lowering_seconds=.3)

def main():
    WORK.mkdir(exist_ok=False);self_check()
    assert 0<os.cpu_count()<=64
    identities=json.loads((LEDGER/'lazy_reference_graph/identities.json').read_text())
    for identity in identities:
        folder=p.OUT/'producers'/identity['producer']
        assert p.runner.digest(folder/'omega.exe')==identity['compiler_sha256']
        assert p.runner.digest(folder/'omega.lock')==identity['accepted_lock_sha256']
        assert json.loads(subprocess.check_output([str(folder/'omega.exe'),'--build-profile']))==identity['build_profile']
    metadata=dict(base=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),logical_processors=os.cpu_count(),platform=platform.platform(),python=sys.version,
        schedule=schedule(),identities=identities,input_manifest_sha256=p.runner.digest(p.OUT/'input-manifest.json'),sampling_interval_seconds=.25)
    (WORK/'identity.json').write_text(json.dumps(metadata,indent=2)+'\n',encoding='utf-8',newline='\n')
    p.runner.invoke=invoke
    for block,order in enumerate(schedule()):
        for position,symbol in enumerate(order):
            role='original' if symbol=='A' else 'candidate';label=f'cold-noise-{block}-{position}-{role}'
            p.run(role,label)
            folder=p.OUT/label;record=json.loads((folder/'result.json').read_text());record.update(block=block,position=position,order=order,**phases((folder/'stderr.txt').read_text()))
            (folder/'result.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n')
            print('COLD',block,position,role,'wall',round(record['wall_seconds'],3),'cpu',round(record['cpu_seconds'],3),'check',round(record['checking_seconds'],3),'lower',round(record['lowering_seconds'],3),'other host busy%',round(record['external_busy_capacity_percent'],1),flush=True)

if __name__=='__main__':
    if sys.argv[1:]==['--self-check']:self_check();print('Win32 counters, layouts, monotonicity, phase parser and balanced centered schedule checked.')
    else:main()
