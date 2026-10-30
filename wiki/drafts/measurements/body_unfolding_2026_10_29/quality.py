import hashlib,json,os,shutil,subprocess,sys,time
from pathlib import Path

root=Path.cwd()
sys.path.insert(0,str(root/'tools/benchmark'))
import benchmark
base=root/'build/body-unfolding'
producer=base/'candidate/omega.exe'
records=[]
benchmark.require_optimized(producer)
fixtures=[
    'pass/calls/selected_contract_equation_reentry_exit',
    'pass/proofs/constant_boolean_return_join_exit',
    'pass/borrows/runtime_reference_call_scalar_snapshot_exit',
    'pass/facts/copied_call_argument_snapshot_through_match_join_exit',
    'pass/contracts/current_computed_scalar_guard_exit',
    'fail/calls/selected_contract_equation_reentry_unproved_requirement',
    'fail/contracts/current_computed_scalar_guard_false_arm',
    'fail/contracts/current_computed_scalar_guard_skipped',
    'fail/contracts/current_computed_scalar_guard_overwritten',
    'fail/core/scoped_integer_call_wrapping_actual_requires_formation',
    'fail/calls/closed_constructor_requires_restricted_nested_call_rejected',
    'fail/borrow/scalar_reference_result_does_not_preserve_mutated_bound',
    'fail/calls/copied_argument_ensures_cannot_restore_callee_mutated_source',
    'fail/contracts/boolean_result_conflicting_normal_returns',
    'fail/proofs/unknown_boolean_return_is_not_omitted',
]

timing_rows=json.loads((base/'timings.json').read_text())
times={variant:[row['seconds'] for row in timing_rows if row['variant']==variant and row['command'][1:3]==['update','--project']] for variant in ['A','B']}
assert all(len(values)==2 for values in times.values()),times
means={variant:sum(values)/len(values) for variant,values in times.items()}
reduction=1-means['B']/means['A']
screen_passed=reduction>=0.10 and max(times['B'])<min(times['A'])
(base/'performance-screen.json').write_text(json.dumps(dict(times=times,means=means,reduction=reduction,separated_ranges=max(times['B'])<min(times['A']),passed=screen_passed),indent=2))
if not screen_passed:
    print('Performance screen failed: retain only selected-contract recursion positive/refusal controls; broader suite skipped.',flush=True)
    fixtures=['pass/calls/selected_contract_equation_reentry_exit','fail/calls/selected_contract_equation_reentry_unproved_requirement']
for fixture in fixtures:
    source=root/'tests/omega'/fixture
    directory=base/'cases'/('candidate-'+fixture.replace('/','-'))
    project=directory/'input'
    project.mkdir(parents=True,exist_ok=False)
    for p in source.glob('*.omg'):shutil.copyfile(p,project/p.name)
    cache=base/'cases/timing-1-B/cache'
    assert cache.is_dir()
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
        assert code==expected and f'interp exit: {expected}' in text and f'native exit {expected}' in text and 'DECLINED' not in text,(fixture,text[-3000:])
    (base/'quality.json').write_text(json.dumps(records,indent=2))
