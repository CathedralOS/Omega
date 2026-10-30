import json,os,shutil,subprocess,sys,time
from pathlib import Path
root=Path.cwd();out=root/'build/remaining-first-checking'
sys.path.insert(0,str(root/'tools/benchmark'));import benchmark
records=[]
name='specification_result_without_guarantees_requires_formation'
for variant,kind in [('B','fail'),('A','pass'),('A','fail')]:
    producer=root/'build/first-checking/candidate/omega.exe' if variant=='A' else out/'candidate/omega.exe'
    benchmark.require_optimized(producer)
    fixture=f'{kind}/calls/{name}';source=root/'tests/omega'/fixture
    directory=out/'cases'/f'formation-replay-{variant}-{kind}'
    project=directory/'input';project.mkdir(parents=True,exist_ok=False)
    for path in source.glob('*.omg'):shutil.copyfile(path,project/path.name)
    cache=directory/'cache';cache.mkdir();os.environ['LOCALAPPDATA']=str(cache)
    os.environ.pop('OMEGA_PHASE_PROFILE',None);os.environ.pop('OMEGA_REVIEW_TIMINGS',None)
    def invoke(command,cwd=None):
        started=time.monotonic();result=subprocess.run(command,cwd=cwd or root,capture_output=True,timeout=120)
        text=(result.stdout+result.stderr).decode('utf-8','replace')
        transcript=directory/f'{len(records):02}.txt';transcript.write_text(text,encoding='utf-8')
        records.append(dict(variant=variant,fixture=fixture,command=[str(x) for x in command],exit=result.returncode,seconds=time.monotonic()-started,transcript=str(transcript),producer_sha256=benchmark.sha256_file(producer),source_sha256=benchmark.sha256_file(project/'main.omg')))
        (out/'formation-replay.json').write_text(json.dumps(records,indent=2),encoding='utf-8')
        print(f'{variant} {kind} {command[1]}: {result.returncode}',flush=True)
        return result.returncode,text
    if kind=='fail':
        code,text=invoke([producer,'--check',project/'main.omg'])
        expected=(source/'expected.txt').read_text().strip()
        assert code!=0 and expected in text,text
        records[-1].update(expected=expected,matched=True)
    else:
        benchmark.run_plain=invoke;benchmark.settle_package_review(producer,project,'windows_x86_64')
        code,text=invoke([producer,'run','--both','--keep',project/'main.omg'])
        assert code==0 and 'native exit 0' in text and 'interp exit: 0' in text and 'DECLINED' not in text,text
    (out/'formation-replay.json').write_text(json.dumps(records,indent=2),encoding='utf-8')
