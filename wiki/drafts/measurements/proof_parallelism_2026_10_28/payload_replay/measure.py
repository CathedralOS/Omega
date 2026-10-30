"""Use the established Dungeon/accepted-lock/cache controls for payload attribution."""
import ctypes, importlib.util, json, pathlib, subprocess, sys, time
ROOT = next(parent for parent in pathlib.Path(__file__).resolve().parents if (parent/'AGENTS.md').exists())
path = pathlib.Path(__file__).resolve().parents[1]/'reconciliation/measure.py'
specification = importlib.util.spec_from_file_location('reconciliation_runner',path)
parent = importlib.util.module_from_spec(specification); specification.loader.exec_module(parent)

class MemoryCounters(ctypes.Structure):
    _fields_ = [('cb',ctypes.c_uint32),('faults',ctypes.c_uint32)] + [(name,ctypes.c_size_t) for name in
        ['peak_working_set','working_set','peak_paged','paged','peak_nonpaged','nonpaged','pagefile','peak_pagefile']]

def invoke(command,folder,environment,timeout=600):
    folder.mkdir(parents=True,exist_ok=False)
    started=time.perf_counter()
    process=subprocess.Popen([str(item) for item in command],cwd=ROOT,env=environment,
        stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        stdout,stderr=process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        process.kill();process.communicate();raise
    wall=time.perf_counter()-started
    created,exited,kernel,user=(ctypes.c_uint64() for _ in range(4))
    handle=ctypes.c_void_p(int(process._handle))
    assert ctypes.windll.kernel32.GetProcessTimes(handle,ctypes.byref(created),ctypes.byref(exited),ctypes.byref(kernel),ctypes.byref(user))
    memory=MemoryCounters();memory.cb=ctypes.sizeof(memory)
    has_memory=ctypes.windll.psapi.GetProcessMemoryInfo(handle,ctypes.byref(memory),ctypes.sizeof(memory))
    result=subprocess.CompletedProcess(command,process.returncode,stdout,stderr)
    (folder/'stdout.txt').write_bytes(stdout);(folder/'stderr.txt').write_bytes(stderr)
    record=dict(command=[str(item) for item in command],wall_seconds=wall,exit_code=process.returncode,
        cpu_seconds=(kernel.value+user.value)/1e7,peak_working_set_bytes=memory.peak_working_set if has_memory else None)
    (folder/'result.json').write_text(json.dumps(record,indent=2),encoding='utf-8')
    print(json.dumps(record),flush=True)
    if process.returncode!=0:raise RuntimeError(stderr.decode('utf-8','replace')[-6000:])
    return result,record

parent.runner.invoke=invoke

def measure(producer,label,mode='timing',comparison=False):
    result = parent.measure(producer,label,mode,comparison=comparison)
    if mode!='off':
        for row in result['reconciliation']:
            assert sum(value for key,value in row['metrics'].items() if key.endswith('_ns'))<=row['wall_ns']
        main=max((row for row in result['reconciliation'] if row['operation']=='memberships'),key=lambda row:row['machines'])
        print(label,main['wall_ns']/1e6,sorted(main['metrics'].items(),key=lambda item:item[1],reverse=True),flush=True)
    assert result.get('image_sha256','3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8')=='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
    return result

if __name__=='__main__':
    if len(sys.argv)>1:
        measure(*sys.argv[1:])
    else:
        for label in ['payload-before-a','payload-before-b','payload-before-c']:
            measure('payload',label)
        measure('payload','payload-counts','counts')
