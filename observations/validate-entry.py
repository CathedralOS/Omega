from pathlib import Path
import json,hashlib,subprocess
root=Path('build/product-nested')
def load(name):return json.loads((root/name).read_text())
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
producer=load('entry-producer-4.json');runner=load('entry-runner-producer.json')
revision='88b9f71bc0c167521ae8032e3985412afdd7a677';assert producer['revision']==runner['revision']==revision
for name,metadata in [('omega',producer),('corpus_runner',runner)]:
 assert digest(root/'entry-producer-4'/name)==metadata['sha256']
 assert json.loads(metadata['profile'])['opt_level']=='1'
compiler_inputs=['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']
subprocess.run(['git','diff','--exit-code',revision,'HEAD','--',*compiler_inputs],check=True)
for path,sha in load('entry-frozen-inputs.json').items():assert digest(path)==sha,path
baseline=load('entry-row-baseline.json');actual=load('entry-row-repaired-4.json');assert len(baseline)==len(actual)==18
for row in baseline:
 assert digest(row['command'][-1])==row['source_sha256']
 if row['invalid']:assert row['exit']==0,row
 else:assert row['exit']==1 and 'expects 2 argument(s), got 3' in row['stderr'],row
for row in actual:
 source=Path(row['command'][-1]);assert digest(source)==row['source_sha256']
 pin=Path('tests/omega')/('fail' if row['invalid'] else 'pass')/'facts'/f"nested_machine_lexical_entry_{row['place']}_{'rejected' if row['invalid'] else 'exit'}"
 for name in ['main.omg','build.omg']:assert (source.parent/name).read_bytes()==(pin/name).read_bytes()
 if row['invalid']:assert row['exit']==1 and 'expects 3 argument(s), got 2' in row['stderr'],row
 else:assert row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'],row
fixture_inputs=[str(Path('tests/omega')/row['fixture']) for row in load('entry-corpus-source.json')['observations']]
scope_inputs=[*compiler_inputs,'samples/cli/facts/nested_machine_invocation',*fixture_inputs]
changed=subprocess.check_output(['git','diff','--name-only','371d35eadbbce54e5f311618ce3f80dac231a76e','HEAD','--',*scope_inputs],text=True).splitlines()
assert changed==['tests/omega/fail/calls/terminal_call_marked_state/expected.txt'],changed
controls=load('entry-controls-repaired-4.json');assert len(controls)==10
for row in controls:
 source=Path(row['command'][-1]);assert digest(source)==row['source_sha256']
 key=row['name'].removesuffix('/main.omg').replace('/invalid','')
 suffix='exit' if row['positive'] else 'rejected'
 pin=Path('tests/omega')/('pass' if row['positive'] else 'fail')/'facts'/('nested_machine_lexical_entry_'+key+'_'+suffix)
 for path in source.parent.glob('*.omg'):assert path.read_bytes()==(pin/path.name).read_bytes()
 if row['positive']:assert row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'],row
 else:
  assert row['exit']==1,row
  if 'qualified/invalid/' in row['name']:assert 'violates required fact `value > 0`' in row['stderr'] and '0 > 0' in row['stderr'],row
  elif 'ordinary/invalid/' in row['name'] or 'parenthesized/invalid/' in row['name']:assert 'expects 3 argument(s), got 2' in row['stderr'],row
  elif row['name'].startswith('shadowed_namespace_ordinary/'):assert 'does not resolve to a callable machine entry or requirement' in row['stderr'],row
  else:assert 'does not select a declared machine entry or callable requirement' in row['stderr'],row
record=load('entry-corpus-source.json');assert record['revision']=='371d35eadbbce54e5f311618ce3f80dac231a76e' and not record['dirty']
assert not record['infrastructure_error'] and not record['oracle_failures'];assert len(record['observations'])==124
known='pass/asm/entry_zero_branch_conditional_exit';marker='fail/calls/terminal_call_marked_state'
rows={row['fixture']:dict(row,observed_revision=record['revision']) for row in record['observations'] if row['fixture']!=known}
correction=load('entry-marker-correction.json');assert len(correction['observations'])==1 and not correction['dirty'] and not correction['infrastructure_error'] and not correction['oracle_failures']
for row in correction['observations']:assert row['fixture']==marker and row['expected_satisfied'] is True;rows[marker]=dict(row,observed_revision=correction['revision'])
assert len(rows)==123
for row in rows.values():
 assert row['status']==('rejected' if row['tier']=='fail' else 'checked'),row
 if row['tier']=='fail':assert row['expected_satisfied'] is True,row
both=load('entry-corpus-both.json');assert len(both['observations'])==58 and not both['dirty'] and not both['infrastructure_error']
assert all(failure.startswith(known+':') for failure in both['oracle_failures'])
executions=[row for row in both['observations'] if row['fixture']!=known];assert len(executions)==57
for row in executions:
 assert row['status']=='built',row
 if row['fixture']=='pass/facts/nested_machine_invocation/inferred_crash':assert row['facts']['exit']=='signal' and row['facts']['signal']=='5' and row['facts']['interpreter']=='trapped' and row['facts']['terminal-targets']=='match',row
 else:assert row['facts']['exit']==row['facts']['interpreter-exit']=='0' and row['facts']['interpreter']=='agrees',row
attribution=load('baseline-regression-attribution.json');assert len(attribution)==3
for row in attribution:
 assert row['producer']['revision']=='70d5cda38eda5688b811050f86a3938b50e3c553' and row['exit']==1 and digest(row['command'][-1])==row['source_sha256'],row
 current=next(candidate for candidate in record['observations'] if candidate['fixture']==row['fixture'])
 assert all(message in row['stderr'] for message in current['diagnostics']),row
summary=dict(producer_revision=revision,baseline_revision='70d5cda38eda5688b811050f86a3938b50e3c553',host='macOS ARM64',nine_places_both_zero=9,arity_reject_twins=9,supplementary_controls=10,accepted_source_fixtures=len(rows),actual_fixture_executions=57,native_terminal_zero_agreements=56,authored_trap_agreements=1,baseline_assembly_refusal=known,qualification='Validates retained actual engine observations; no engine executions relabeled or reexecuted by this validator.',setup_correction='Qualified module control originally omitted module declaration. Original setup retained as a reject; no valid-positive baseline claim is made for that setup.')
(root/'entry-acceptance.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary))
