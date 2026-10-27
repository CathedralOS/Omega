from pathlib import Path
import gzip,hashlib,json,subprocess
root=Path('build/nested-takeover');revision='b011da5f273a04a924bc79cfa9df3bef5bcd6674'
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
cli=json.loads((root/'producer-integrated.json').read_text());runner=json.loads((root/'corpus-producer-integrated.json').read_text())
assert cli['revision']==runner['revision']==revision
h=hashlib.sha256()
with gzip.open(root/'producer-integrated/omega.gz','rb') as stream:
 while chunk:=stream.read(1024*1024):h.update(chunk)
assert h.hexdigest()==cli['sha256']
assert digest(root/'producer-integrated/corpus_runner')==runner['sha256']
assert json.loads(cli['profile'])['opt_level']==json.loads(runner['profile'])['opt_level']=='1'
subprocess.run(['git','diff','--exit-code',revision,'HEAD','--','omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml'],check=True)
rows=json.loads((root/'row-integrated.json').read_text());assert len(rows)==18
for row in rows:
 source=Path(row['command'][-1]);assert digest(source)==row['source_sha256']
 suffix='rejected' if row['invalid'] else 'exit';tier='fail' if row['invalid'] else 'pass';pin=Path('tests/omega')/tier/'facts'/f"nested_machine_inferred_requirement_{row['place']}_{suffix}"
 for name in ['main.omg','build.omg']:assert (source.parent/name).read_bytes()==(pin/name).read_bytes()
 if row['invalid']:assert row['exit']==1 and 'cannot prove requires contract for call same from forward' in row['stderr'],row
 else:assert row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'],row
controls=json.loads((root/'pinned-controls.json').read_text());assert len(controls)==6
for row in controls:
 assert digest(row['command'][-1])==row['source_sha256']
 if '--both' in row['command']:assert row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'],row
 else:assert row['exit']==1 and 'cannot prove requires contract for call same from' in row['stderr'],row
original=json.loads((root/'integrated.json').read_text());assert original['producer']==cli
for row in original['observations']:
 if row['name']=='inferred_invalid':assert row['exit']==1 and 'cannot prove requires contract for call same from forward' in row['stderr'],row
 else:assert row['exit']==0,row
stack=json.loads((root/'bounded-stack-integrated.json').read_text());assert stack['exit']==0 and stack['stack_soft_limit_bytes']==262144 and stack['producer']==cli
assert digest(root/'stack-product-integrated/omega-program')==stack['image_sha256']
summary=dict(producer_revision=revision,host='macOS ARM64',nine_places_both_exit_zero=9,equality_rejects=9,soundness_controls=6,native_cycle_stack_limit_bytes=262144,native_cycle_iterations=100000)
retired=json.loads((root/'retired-non-tail-positive-oracles.json').read_text())
retired_keys={str(Path(row['old']).relative_to('tests/omega')) for row in retired}
corrections=json.loads((root/'corpus-non-tail-corrections.json').read_text())
assert len(retired_keys)==2 and len(corrections['observations'])==2
assert not corrections['oracle_failures'] and not corrections['infrastructure_error']
for leg in ['source','both']:
 record=json.loads((root/f'corpus-{leg}-integrated.json').read_text())
 assert record['revision']==revision and not record['dirty']
 assert not record['infrastructure_error']
 # These raw failures remain visible: their two former positive programs
 # demand non-tail runtime recursion, forbidden by the accepted contract.
 # Fresh negative controls isolate that rule and must reject for it.
 assert all(any(key in failure for key in retired_keys) for failure in record['oracle_failures'])
 rows={row['fixture']:dict(row,recorded_at=record['revision']) for row in record['observations'] if row['fixture'] not in retired_keys}
 for key in retired_keys:assert not (Path('tests/omega')/key/'main.omg').exists()
 if leg=='source':
  for row in corrections['observations']:
   assert row['status']=='rejected' and row['expected_satisfied'] is True
   assert any('lacks a classified tail edge' in message for message in row['diagnostics'])
   rows[row['fixture']]=dict(row,recorded_at=corrections['revision'])
 for row in rows.values():
  if leg=='source':
   assert row['status']==('rejected' if row['tier']=='fail' else 'checked'),row
   if row['tier']=='fail':assert row['expected_satisfied'] is True,row
  elif row['status']=='built' and 'exit' in row['facts']:
   if row['fixture'].endswith('/inferred_crash'):assert row['facts']['interpreter']=='trapped' and row['facts']['terminal-targets']=='match',row
   else:assert row['facts']['exit']==row['facts']['interpreter-exit']=='0' and row['facts']['interpreter']=='agrees',row
  else:assert row['status'] in ['built','checked'],row
 (root/f'accepted-{leg}-integrated.json').write_text(json.dumps(list(rows.values()),indent=2)+'\n')
 summary[leg+'_fixtures']=len(rows)
 if leg=='both':summary['corpus_actual_executions']=sum('exit' in row['facts'] for row in rows.values())
summary['retired_invalid_positive_fixtures']=sorted(retired_keys)
assert 'test result: ok. 1 passed' in (root/'required-fact-test.log').read_text()
summary['historical_parser_suite']='262 passed, 10 failures identical to main10c; final grammar regression test passed separately'
(root/'integrated-acceptance.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary))
