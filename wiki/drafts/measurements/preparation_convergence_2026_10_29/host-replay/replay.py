import json,os,subprocess,sys,time
from pathlib import Path
root=Path.cwd();out=root/'build/preparation-convergence/host-replay';out.mkdir(parents=True,exist_ok=True)
sys.path.insert(0,str(root/'tools/benchmark'));import benchmark
records=[]
cases=[]
# Complete the actual host comparisons omitted by the historical target-only calls.
old=root/'build/preparation-convergence'
for fixture in ['runtime_numeric_operand_order_exit','runtime_numeric_operand_single_evaluation_exit','selected_contract_equation_reentry_exit','runtime_reference_call_scalar_snapshot_exit','copied_call_argument_snapshot_through_match_join_exit','selected_applied_synchronous_chain_exit','selected_synchronous_helper_ceiling_exit']:
    cases.append(('convergence',old/'candidate/omega.exe',old/'cases'/('candidate-'+fixture)))
for fixture in ['selected_applied_synchronous_chain_exit','selected_synchronous_helper_ceiling_exit']:
    cases.append(('pre-convergence',root/'build/contract-query-publication/producer/omega.exe',old/'cases'/('baseline-'+fixture)))
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
