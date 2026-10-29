import hashlib,json,os,shutil,subprocess,sys,time
from pathlib import Path

root=Path.cwd()
sys.path.insert(0,str(root/'tools/benchmark'))
import benchmark
base=root/'build/first-checking'
producer=base/'candidate/omega.exe'
records=[]
benchmark.require_optimized(producer)
fixtures=['pass/calls/runtime_numeric_operand_single_evaluation_exit', 'pass/calls/selected_contract_equation_reentry_exit', 'pass/borrows/runtime_reference_call_scalar_snapshot_exit', 'pass/facts/copied_call_argument_snapshot_through_match_join_exit', 'fail/calls/authored_numeric_name_requires', 'fail/calls/selected_contract_equation_reentry_unproved_requirement', 'fail/arithmetic/numeric_operand_abs_unrepresentable', 'fail/calls/closed_constructor_requires_restricted_nested_call_rejected', 'fail/borrow/scalar_reference_result_does_not_preserve_mutated_bound', 'fail/calls/copied_argument_ensures_cannot_restore_callee_mutated_source']
for fixture in fixtures:
    source=root/'tests/omega'/fixture
    directory=base/'cases'/('candidate-'+source.name)
    project=directory/'input'
    project.mkdir(parents=True,exist_ok=False)
    for p in source.glob('*.omg'):shutil.copyfile(p,project/p.name)
    cache=directory/'cache'
    cache.mkdir()
    os.environ['LOCALAPPDATA']=str(cache)
    os.environ['OMEGA_REVIEW_TIMINGS']='1'
    os.environ.pop('OMEGA_PHASE_PROFILE',None)
    def invoke(command,cwd=None):
        started=time.monotonic()
        try:
            result=subprocess.run(command,cwd=cwd or root,capture_output=True,timeout=360)
        except subprocess.TimeoutExpired as error:
            (directory/'timeout.txt').write_bytes((error.stdout or b'')+(error.stderr or b''))
            raise
        text=(result.stdout+result.stderr).decode('utf-8','replace')
        label=f'{len(records):02}'
        (directory/(label+'.txt')).write_text(text,encoding='utf-8')
        row=dict(fixture=fixture,command=[str(a) for a in command],exit=result.returncode,seconds=time.monotonic()-started,transcript=str(directory/(label+'.txt')),producer_sha256=benchmark.sha256_file(producer),source_sha256=benchmark.sha256_file(project/'main.omg'))
        records.append(row)
        (base/'quality.json').write_text(json.dumps(records,indent=2))
        print(f'{fixture}: {command[1]} exit {result.returncode} in {row["seconds"]:.2f}s',flush=True)
        return result.returncode,text
    if fixture.startswith('fail/'):
        code,text=invoke([producer,'--check',project/'main.omg'])
        expected=[line for line in (source/'expected.txt').read_text().splitlines() if line.strip()]
        assert code!=0 and all(line in text for line in expected),(fixture,text[-2000:])
        records[-1]['expected']=expected
        records[-1]['matched']=True
    else:
        benchmark.run_plain=invoke
        benchmark.settle_package_review(producer,project,'windows_x86_64')
        code,text=invoke([producer,'--timings','--target','windows_x86_64','--build-dir',directory/'native',project/'main.omg'])
        _,artifact=benchmark.parse_timings(text)
        assert code==0 and artifact and Path(artifact).is_file(),text[-3000:]
        records[-1]['artifact_sha256']=benchmark.sha256_file(artifact)
        native=subprocess.run([artifact],capture_output=True,timeout=30)
        expected=int((source/'expected_native_exit.txt').read_text())
        assert native.returncode==expected,(fixture,native.returncode)
        records[-1]['native_exit']=native.returncode
        code,text=invoke([producer,'run','--both','--keep',project/'main.omg'])
        assert code==expected and f'interp exit: {expected}' in text and 'native ' in text and 'DECLINED' not in text,(fixture,text[-3000:])
    (base/'quality.json').write_text(json.dumps(records,indent=2))

# Correct the historical target-only harness calls and require actual host observations.
subprocess.run([sys.executable,str(root/'build/preparation-convergence/host-replay/replay.py')],check=True)
subprocess.run([sys.executable,str(base/'host.py')],check=True)
