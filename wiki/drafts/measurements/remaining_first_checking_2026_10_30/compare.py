import json, os, re, shutil, subprocess, sys, time
from pathlib import Path
root=Path.cwd(); out=root/'build/remaining-first-checking'
sys.path.insert(0,str(root/'tools/benchmark')); import benchmark
records=[]
inputs={str(p.relative_to(root)):benchmark.sha256_file(p) for p in (root/'source/library/std').rglob('*.omg')}
producers={'A':root/'build/first-checking/candidate/omega.exe','B':out/'candidate/omega.exe'}
assert benchmark.sha256_file(producers['A'])=='1c7645a93c087116c69bb5951cc940709cdadecfeae847d38f9de957d3ad4d60'
for p in producers.values(): benchmark.require_optimized(p)
for number,variant in enumerate(['A','B','B','A']):
    producer=producers[variant]; fixture='runtime_numeric_operand_order_exit'
    source=root/'tests/omega/pass/calls'/fixture
    directory=out/'cases'/f'timing-{number}-{variant}'; project=directory/'input'
    project.mkdir(parents=True,exist_ok=False)
    for p in source.glob('*.omg'):
        shutil.copyfile(p,project/p.name); inputs[str(p.relative_to(root))]=benchmark.sha256_file(p)
    cache=directory/'cache'; cache.mkdir()
    os.environ['LOCALAPPDATA']=str(cache)
    os.environ['OMEGA_REVIEW_TIMINGS']='1'; os.environ.pop('OMEGA_PHASE_PROFILE',None)
    def invoke(command,cwd=None):
        start=time.monotonic()
        try: result=subprocess.run(command,cwd=cwd or root,capture_output=True,timeout=360)
        except subprocess.TimeoutExpired as error:
            (directory/'timeout.txt').write_bytes((error.stdout or b'')+(error.stderr or b'')); raise
        text=(result.stdout+result.stderr).decode('utf-8','replace')
        transcript=directory/f'{len(records):02}.txt'; transcript.write_text(text,encoding='utf-8')
        records.append(dict(trial=number,variant=variant,fixture=fixture,command=[str(x) for x in command],exit=result.returncode,seconds=time.monotonic()-start,transcript=str(transcript.relative_to(out)),producer_sha256=benchmark.sha256_file(producer)))
        (out/'timings.json').write_text(json.dumps(records,indent=2),encoding='utf-8')
        print(f'{number} {variant}: {command[1]} exit {result.returncode} in {records[-1]["seconds"]:.2f}s',flush=True)
        return result.returncode,text
    benchmark.run_plain=invoke
    benchmark.settle_package_review(producer,project,'windows_x86_64')
    assert records[-2]['exit']==3 and records[-1]['exit']==0
    code,text=invoke([producer,'--timings','--target','windows_x86_64','--build-dir',directory/'native',project/'main.omg'])
    _,artifact=benchmark.parse_timings(text)
    assert code==0 and artifact and Path(artifact).is_file() and 'DECLINED' not in text,text
    native=subprocess.run([artifact],capture_output=True,timeout=30)
    assert native.returncode==0
    records[-1].update(native_exit=0,artifact_sha256=benchmark.sha256_file(artifact))
    code,text=invoke([producer,'run','--both','--keep',project/'main.omg'])
    assert code==0 and 'interp exit: 0' in text and 'native exit 0' in text and 'DECLINED' not in text,text
    (out/'timings.json').write_text(json.dumps(records,indent=2),encoding='utf-8')
assert len({r['artifact_sha256'] for r in records if 'artifact_sha256' in r})==1
for path,digest in inputs.items(): assert benchmark.sha256_file(root/path)==digest
(out/'timing-inputs.json').write_text(json.dumps(inputs,indent=2),encoding='utf-8')
