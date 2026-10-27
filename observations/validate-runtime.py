from pathlib import Path
import json,hashlib,subprocess,gzip
root=Path('build/product-nested')
def load(name):return json.loads((root/name).read_text())
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
inputs=['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for metadata,name in [('runtime-candidate-producer.json','omega'),('runtime-runner-producer.json','corpus_runner')]:
 producer=load(metadata);assert json.loads(producer['profile'])['opt_level']=='1'
 assert digest(root/'runtime-candidate-producer'/name)==producer['sha256']
 subprocess.run(['git','diff','--exit-code',producer['revision'],'HEAD','--',*inputs],check=True)
base=load('runtime-base-producer.json');assert base['revision']=='baf1b6ea0633681bcc2cf34d563d748a47fadb36'
with gzip.open(root/'runtime-base-producer/omega.gz','rb') as stream:
 hash=hashlib.sha256()
 while chunk:=stream.read(4*1024*1024):hash.update(chunk)
 assert hash.hexdigest()==base['sha256']
for path,sha in load('runtime-corrected-frozen-inputs.json').items():assert digest(path)==sha,path
for name in ['runtime-row-corrected-baseline.json','runtime-row-corrected-candidate.json']:
 rows=load(name);assert len(rows)==18
 for row in rows:
  source=Path(row['command'][-1]);assert digest(source)==row['source_sha256']
  pin=Path('tests/omega')/('fail' if row['invalid'] else 'pass')/'facts'/f"nested_machine_private_arithmetic_mutable_{row['place']}_{'rejected' if row['invalid'] else 'exit'}"
  for filename in ['main.omg','build.omg']:assert (source.parent/filename).read_bytes()==(pin/filename).read_bytes()
  if 'baseline' in name or row['invalid']:
   assert row['exit']==1 and 'may overflow `u64`' in row['stderr'] and 'expects ' not in row['stderr'],row
  else:assert row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'],row
controls=load('runtime-controls/candidate.json');assert len(controls)==3
for row in controls:
 assert digest(row['command'][-1])==row['source_sha256']
 assert row['exit']==1 and 'may overflow `u64`' in row['stderr'],row
 # Disjoint mutable-referent preservation is recorded unfinished, not acceptance.
 if row['name']!='disjoint':
  pin=Path('tests/omega/fail/facts')/('nested_machine_private_arithmetic_mutable_'+row['name']+'_rejected')
  assert (pin/'main.omg').read_bytes()==Path(row['command'][-1]).read_bytes()
source=load('runtime-corpus-source.json');both=load('runtime-corpus-both.json')
for record in [source,both]:
 assert not record['dirty'] and not record['infrastructure_error'] and not record['oracle_failures'],record['oracle_failures']
 subprocess.run(['git','diff','--exit-code',record['revision'],'HEAD','--',*inputs,'samples/cli/facts/nested_machine_invocation',*[str(Path('tests/omega')/row['fixture']) for row in record['observations']]],check=True)
for row in source['observations']:
 assert row['status']==('rejected' if row['tier']=='fail' else 'checked'),row
 if row['tier']=='fail':assert row['expected_satisfied'] is True,row
for row in both['observations']:
 assert row['status']=='built',row
 if row['fixture']=='pass/facts/nested_machine_invocation/inferred_crash':assert row['facts']['exit']=='signal' and row['facts']['interpreter']=='trapped',row
 else:assert row['facts']['exit']==row['facts']['interpreter-exit']=='0' and row['facts']['interpreter']=='agrees',row
summary=dict(baseline=base['revision'],producer=load('runtime-candidate-producer.json')['revision'],host='macOS ARM64',nine_places_both_zero=9,zero_arrival_reject_twins=9,overlapping_write_rejects=2,source_fixtures=len(source['observations']),native_terminal_outcomes=len(both['observations']),qualification='Authenticates retained actual engine observations; does not execute or relabel engines.',remaining='Mutable referent-field bound probe still refuses; parent product migration and other invocation obligations remain open.',setup_correction=load('runtime-const-setup-correction.json')['reason'])
(root/'runtime-acceptance.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary))
