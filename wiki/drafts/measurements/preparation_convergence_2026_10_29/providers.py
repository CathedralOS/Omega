import json,os,re,shutil,subprocess,sys,time
from pathlib import Path
root=Path.cwd(); base=root/'build/preparation-convergence'
sys.path.insert(0,str(root/'tools/benchmark'))
import benchmark
fixtures=['pass/capabilities/selected_applied_synchronous_chain_exit','pass/capabilities/selected_synchronous_helper_ceiling_exit','fail/capabilities/selected_applied_synchronous_cycle','fail/capabilities/selected_synchronous_helper_cycle','fail/capabilities/undeclared_synchronous_invocation']
records=[]
for variant,producer in [('baseline',root/'build/contract-query-publication/producer/omega.exe'),('candidate',base/'candidate/omega.exe')]:
    benchmark.require_optimized(producer)
    for fixture in fixtures:
        source=root/'tests/omega'/fixture
        directory=base/'cases'/(variant+'-'+source.name)
        project=directory/'input'; project.mkdir(parents=True,exist_ok=False)
        for p in source.glob('*.omg'):shutil.copyfile(p,project/p.name)
        cache=directory/'cache'; cache.mkdir()
        os.environ['LOCALAPPDATA']=str(cache)
        os.environ.pop('OMEGA_PHASE_PROFILE',None)
        os.environ.pop('OMEGA_REVIEW_TIMINGS',None)
        def invoke(command,cwd=None):
            start=time.monotonic()
            result=subprocess.run(command,cwd=cwd or root,capture_output=True,timeout=180)
            text=(result.stdout+result.stderr).decode('utf-8','replace')
            transcript=directory/f'{len(records):02}.txt'
            transcript.write_text(text,encoding='utf-8',newline='\n')
            records.append(dict(variant=variant,fixture=fixture,command=[str(x) for x in command],exit=result.returncode,seconds=time.monotonic()-start,transcript=str(transcript),producer_sha256=benchmark.sha256_file(producer),source_sha256=benchmark.sha256_file(source/'main.omg')))
            (base/'providers.json').write_text(json.dumps(records,indent=2),encoding='utf-8')
            print(f'{variant} {fixture} {command[1]}: {result.returncode}',flush=True)
            return result.returncode,text
        benchmark.run_plain=invoke
        if fixture.startswith('fail/'):
            expected=[x for x in (source/'expected.txt').read_text().splitlines() if x.strip()]
            if (source/'build.omg').exists():
                code,text=invoke([producer,'update','--project',project,'--target','windows_x86_64'])
                if not (code!=0 and all(x in text for x in expected)):
                    benchmark.settle_package_review(producer,project,'windows_x86_64')
                    code,text=invoke([producer,'--target','windows_x86_64','--build-dir',directory/'native',project/'main.omg'])
            else:
                code,text=invoke([producer,'--check',project/'main.omg'])
            assert code!=0 and all(x in text for x in expected),(fixture,text)
            records[-1].update(matched=True,expected=expected)
        else:
            benchmark.settle_package_review(producer,project,'windows_x86_64')
            code,text=invoke([producer,'--timings','--target','windows_x86_64','--build-dir',directory/'native',project/'main.omg'])
            _,artifact=benchmark.parse_timings(text)
            assert code==0 and artifact and Path(artifact).is_file(),text
            expected=int(re.search(r'Expected exit:\s*(\d+)',(source/'main.omg').read_text()).group(1))
            native=subprocess.run([artifact],capture_output=True,timeout=30)
            assert native.returncode==expected
            records[-1].update(native_exit=native.returncode,artifact_sha256=benchmark.sha256_file(artifact))
            code,text=invoke([producer,'run','--both','--keep','--target','windows_x86_64',project/'main.omg'])
            assert code==expected and 'DECLINED' not in text,text
        (base/'providers.json').write_text(json.dumps(records,indent=2),encoding='utf-8')
for fixture in fixtures[:2]:
    assert len({r['artifact_sha256'] for r in records if r['fixture']==fixture and 'artifact_sha256' in r})==1
