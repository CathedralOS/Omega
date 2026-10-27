from pathlib import Path
import json,hashlib,subprocess
root=Path('build/borrowed-entry')
def load(name):return json.loads((root/name).read_text())
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
inputs=['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']
base=Path('build/product-nested/runtime-candidate-producer.json');baseline=json.loads(base.read_text());assert baseline['revision']=='d871d8aaf7a98bfb2f0bb27de86aac844c5c8a3a'
assert digest('build/product-nested/runtime-candidate-producer/omega')==baseline['sha256'] and json.loads(baseline['profile'])['opt_level']=='1'
subprocess.run(['git','diff','--exit-code',baseline['revision'],'286c3a66623fcdda18830cd7d81ea8bc8d78d506','--',*inputs],check=True)
for filename,name in [('producer-1.json','omega'),('runner-producer.json','corpus_runner')]:
 metadata=load(filename);assert json.loads(metadata['profile'])['opt_level']=='1';assert digest(root/'producer-1'/name)==metadata['sha256']
 subprocess.run(['git','diff','--exit-code',metadata['revision'],'HEAD','--',*inputs],check=True)
for path,sha in load('frozen-inputs.json').items():assert digest(path)==sha,path
for name in ['row-baseline.json','row-repaired-1.json']:
 rows=load(name);assert len(rows)==18
 for row in rows:
  source=Path(row['command'][-1]);assert digest(source)==row['source_sha256']
  pin=Path('tests/omega')/('fail' if row['invalid'] else 'pass')/'facts'/f"nested_machine_private_arithmetic_borrowed_{row['place']}_{'rejected' if row['invalid'] else 'exit'}"
  for filename in ['main.omg','build.omg']:assert (source.parent/filename).read_bytes()==(pin/filename).read_bytes()
  if name=='row-baseline.json' or row['invalid']:assert row['exit']==1 and 'exact arithmetic in machine `checked`' in row['stderr'] and 'may overflow `u64`' in row['stderr'] and 'expects ' not in row['stderr'],row
  else:assert row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'],row
controls=load('controls-repaired-1.json')+load('extra-controls-candidate.json');assert len(controls)==5
for row in controls:
 source=Path(row['command'][-1]);assert digest(source)==row['source_sha256']
 if row['name']=='disjoint':assert row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'],row
 else:assert row['exit']==1 and 'may overflow `u64`' in row['stderr'],row
 prefix='mutable' if row['name'] in ['overlap','alias_overlap'] else 'borrowed';positive=row['name']=='disjoint'
 pin=Path('tests/omega')/('pass' if positive else 'fail')/'facts'/f"nested_machine_private_arithmetic_{prefix}_{row['name']}_{'exit' if positive else 'rejected'}"
 for filename in ['main.omg','build.omg']:assert (source.parent/filename).read_bytes()==(pin/filename).read_bytes()
source=load('corpus-source.json');both=load('corpus-both.json');assert len(source['observations'])==170 and len(both['observations'])==76
for record in [source,both]:
 assert not record['dirty'] and not record['infrastructure_error'] and not record['oracle_failures'],record['oracle_failures']
 paths=[str(Path('tests/omega')/row['fixture']) for row in record['observations']]
 subprocess.run(['git','diff','--exit-code',record['revision'],'HEAD','--',*inputs,*paths,'samples/cli/facts/nested_machine_invocation/private_arithmetic/borrowed'],check=True)
for row in source['observations']:
 assert row['status']==('rejected' if row['tier']=='fail' else 'checked'),row
 if row['tier']=='fail':assert row['expected_satisfied'] is True,row
for row in both['observations']:
 assert row['status']=='built',row
 if row['fixture']=='pass/facts/nested_machine_invocation/inferred_crash':assert row['facts']['exit']=='signal' and row['facts']['interpreter']=='trapped',row
 else:assert row['facts']['exit']==row['facts']['interpreter-exit']=='0' and row['facts']['interpreter']=='agrees',row
summary=dict(starting_source='286c3a66623fcdda18830cd7d81ea8bc8d78d506',baseline_producer=baseline['revision'],repaired_producer=load('producer-1.json')['revision'],host='macOS ARM64',nine_places_both_zero=9,zero_twins_reject=9,disjoint_both_zero=1,mutation_arrival_controls_reject=4,source_fixtures=170,native_terminal_outcomes=76,native_terminal_zero_agreements=75,authored_traps=1,qualification='Authenticates retained original engine observations; does not execute or relabel engines.',remaining='NESTED-MACHINE-UNIFICATION retains product compiler migration and broader invocation/restoration/custody/cycle obligations. No other-host native execution claimed.')
(root/'acceptance.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary))
