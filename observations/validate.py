from pathlib import Path
import gzip,hashlib,json,subprocess
root=Path('build/private-formation')
def load(name):return json.loads((root/name).read_text())
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
compiler_inputs=['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']
producer=load('producer-candidate.json'); runner=load('producer-runner.json')
assert producer['revision']==runner['revision']=='50e50cb1029062ab7fd38145b3312c9bb3ac76e6'
for name,metadata in [('omega',producer),('corpus_runner',runner)]:
 assert digest(root/'producer-candidate'/name)==metadata['sha256']
 assert json.loads(metadata['profile'])['opt_level']=='1'
subprocess.run(['git','diff','--exit-code',producer['revision'],'HEAD','--',*compiler_inputs],check=True)
baseline=load('producer-baseline.json');hashed=hashlib.sha256()
with gzip.open(root/'producer-baseline/omega.gz','rb') as stream:
 while block:=stream.read(1024*1024):hashed.update(block)
assert hashed.hexdigest()==baseline['sha256']
for path,sha in load('frozen-witness-inputs.json').items():assert digest(path)==sha,path
old=load('row-baseline.json');new=load('row-candidate.json');assert len(old)==len(new)==18
for row in old:
 assert row['exit']==1 and 'exact arithmetic in machine `apply`' in row['stderr'] and 'may overflow `u64`' in row['stderr'],row
for row in new:
 source=Path(row['command'][-1]);assert digest(source)==row['source_sha256']
 suffix='rejected' if row['invalid'] else 'exit';tier='fail' if row['invalid'] else 'pass'
 pin=Path('tests/omega')/tier/'facts'/f"nested_machine_private_arithmetic_{row['place']}_{suffix}"
 for name in ['main.omg','build.omg']:assert (pin/name).read_bytes()==(source.parent/name).read_bytes()
 if row['invalid']:assert row['exit']==1 and 'exact arithmetic in machine `apply`' in row['stderr'] and 'may overflow `u64`' in row['stderr'],row
 else:assert row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'],row
controls=load('controls-candidate.json');assert len(controls)==7
for row in controls:
 source=Path(row['command'][-1]);assert digest(source)==row['source_sha256']
 if '--both' in row['command']:assert row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'],row
 else:assert row['exit']==1 and 'exact arithmetic' in row['stderr'] and 'may overflow `u64`' in row['stderr'],row
for leg in ['source','both']:
 record=load('corpus-'+leg+'.json');assert record['revision']=='b8ce72f09729ee24597e3255a8f48fb93e9e5add' and not record['dirty']
 assert not record['oracle_failures'] and not record['infrastructure_error']
 rows=record['observations'];assert len(rows)==(52 if leg=='source' else 22)
 for row in rows:
  if leg=='source':
   assert row['status']==('rejected' if row['tier']=='fail' else 'checked'),row
   if row['tier']=='fail':assert row['expected_satisfied'] is True,row
  elif row['status']=='built':assert row['facts']['exit']==row['facts']['interpreter-exit']=='0' and row['facts']['interpreter']=='agrees',row
  else:assert row['status']=='checked',row
subprocess.run(['git','diff','--exit-code','b8ce72f09729ee24597e3255a8f48fb93e9e5add','HEAD','--',*compiler_inputs,'samples/cli/facts/nested_machine_invocation'],check=True)
summary=dict(producer=producer['revision'],baseline=baseline['revision'],nine_places_both_exit_zero=9,zero_twins_reject=9,soundness_controls=7,source_fixtures=52,fixture_engine_agreement=21,host='macOS ARM64',qualification='Validates retained engine observations; does not reexecute them.',golden_note='New rows only. Three older guarded-read-count negative diagnostics differ from historical golden; all still reject with intended fragments. No historical diagnostics rewritten.')
(root/'acceptance.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary))
