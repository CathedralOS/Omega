import json,os,subprocess,sys,time
from pathlib import Path
root=Path.cwd();out=root/'build/first-checking'
sys.path.insert(0,str(root/'tools/benchmark'));import benchmark
records=[]
cases=[]
# Complete the actual host comparisons omitted by the historical target-only calls.
old=root/'build/preparation-convergence'
for n,v in enumerate(['A','B','B','A']):
    producer=old/'candidate/omega.exe' if v=='A' else out/'candidate/omega.exe'
    cases.append(('comparison-'+v,producer,out/'cases'/f'timing-{n}-{v}'))
for label,producer,directory in cases:
    benchmark.require_optimized(producer)
    os.environ['LOCALAPPDATA']=str(directory/'cache')
    os.environ.pop('OMEGA_PHASE_PROFILE',None);os.environ.pop('OMEGA_REVIEW_TIMINGS',None)
    command=[producer,'run','--both','--keep',directory/'input/main.omg']
    start=time.monotonic();result=subprocess.run(command,capture_output=True,timeout=180)
    text=(result.stdout+result.stderr).decode('utf-8','replace')
    transcript=out/f'host-{len(records):02}.txt';transcript.write_text(text,encoding='utf-8')
    records.append(dict(label=label,command=[str(x) for x in command],exit=result.returncode,seconds=time.monotonic()-start,transcript=transcript.name,producer_sha256=benchmark.sha256_file(producer)))
    (out/'host.json').write_text(json.dumps(records,indent=2),encoding='utf-8')
    print(f'{label} {directory.name}: {result.returncode}\n{text[-450:]}',flush=True)
    assert result.returncode==0 and 'interp exit: 0' in text and 'native ' in text and 'DECLINED' not in text,text
