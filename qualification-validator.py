import concurrent.futures,hashlib,json,subprocess,sys,time
from pathlib import Path
base=Path(__file__).resolve().parent
receipt=json.loads((base/'integrated-producer-21.json').read_text())
cli=Path(receipt['cli']);runner=Path(receipt['runner'])
for path,key in [(cli,'sha256'),(runner,'corpus_sha256')]:assert hashlib.sha256(path.read_bytes()).hexdigest()==receipt[key]
for leg in ['source','both']:
    capture=Path(receipt[leg]);assert hashlib.sha256(capture.read_bytes()).hexdigest()==receipt[leg+'_sha256']
    data=json.loads(capture.read_text());assert not data['oracle_failures'] and not data['infrastructure_error'],data
    counts=data['counts'];assert not counts['wrong_reason'] and not counts['fail_not_rejected'],counts
    if leg=='source':
        known='pass/control_flow/saved_case_predicate_disjoint_effects_exit'
        baseline_path=base/'saved-case-source-baseline-622.json'
        baseline=json.loads(baseline_path.read_text())
        assert baseline['exit']!=0 and 'cannot prove selected case for payload access `choice.amount`' in baseline['stderr']
        assert counts['lost_success_fixtures']==[known],counts
        current=next(observation for observation in data['observations'] if observation['fixture']==known)
        assert current['status']=='rejected' and len(current['diagnostics'])==1
        assert current['diagnostics'][0] in baseline['stderr'],current
        print(json.dumps({'retained_baseline_gap':known,'baseline_producer':baseline['producer'],'current_diagnostic':current['diagnostics']}),flush=True)
    else:assert not counts['lost_successes'],counts
    if leg=='both':
        for observation in data['observations']:
            assert observation['status']=='built' and observation['facts']['exit']=='0' and observation['facts']['interpreter-exit']=='0' and observation['facts']['interpreter']=='agrees',observation
    print(json.dumps({'retained_capture':str(capture),'producer':receipt['revision'],'target':data['target'],'counts':counts}),flush=True)
root=Path('samples/cli/facts/descriptor_replacement_order');places='local field parameter call_result match_join const_name other_state other_machine target_body'.split()
def run(job):
    place,invalid=job;path=root/place/'guard_replacement'/('invalid/main.omg' if invalid else 'main.omg')
    fixture=Path('tests/omega')/('fail' if invalid else 'pass')/'facts'/('descriptor_guard_replacement_through_'+place+('_rejected' if invalid else '_exit'))
    for name in ['main.omg','build.omg']:assert path.with_name(name).read_bytes()==(fixture/name).read_bytes()
    command=[str(cli),'--check',str(path)] if invalid else [str(cli),'run','--both',str(path)]
    started=time.monotonic();result=subprocess.run(command,capture_output=True,text=True,timeout=180)
    record=dict(place=place,invalid=invalid,command=command,source_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),exit=result.returncode,stdout=result.stdout,stderr=result.stderr,seconds=time.monotonic()-started)
    if invalid:
        expected=[line.strip() for line in (fixture/'expected.txt').read_text().splitlines() if line.strip()]
        record['accepted']=result.returncode!=0 and bool(expected) and all(fragment in result.stderr for fragment in expected)
    else:record['accepted']=result.returncode==0 and 'native exit 0' in result.stderr and 'interp exit: 0' in result.stderr
    print(place,invalid,record['accepted'],flush=True);return record
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:records=list(pool.map(run,[(place,invalid) for place in places for invalid in (False,True)]))
(base/'guard-row-qualified-integrated-21.json').write_text(json.dumps(records,indent=2)+'\n');print(json.dumps(records,indent=2),flush=True)
assert len(records)==18 and all(record['accepted'] for record in records)
