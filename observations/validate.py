from pathlib import Path
import json,hashlib,subprocess
r=Path('build/private-feasible')
def load(name):return json.loads((r/name).read_text())
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
inputs=['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for filename,directory,binary in [('producer.json',r/'producer','omega'),('runner-producer.json',r/'producer','corpus_runner'),('../borrowed-entry/producer-4.json',Path('build/borrowed-entry/producer-4'),'omega')]:
 m=load(filename);assert json.loads(m['profile'])['opt_level']=='1';assert digest(directory/binary)==m['sha256']
 if 'borrowed-entry' not in filename:subprocess.run(['git','diff','--exit-code',m['revision'],'HEAD','--',*inputs],check=True)
subprocess.run(['git','diff','--exit-code','9f2d790a91c3d5a7e4961e329c60ed5fa3710fea','e522fcf48168330f1d47f16b938a89d9adc3c104','--',*inputs],check=True)
for label in ['baseline-final','repaired-final']:
 rows=load('row-'+label+'.json');assert len(rows)==18
 for x in rows:
  p=Path(x['command'][-1]);assert digest(p)==x['source_sha256']
  d=Path('tests/omega')/('fail' if x['invalid'] else 'pass')/'facts'/f"nested_machine_private_arithmetic_feasible_{x['place']}_{'rejected' if x['invalid'] else 'exit'}"
  for f in ['main.omg','build.omg']:assert (p.parent/f).read_bytes()==(d/f).read_bytes()
  if label=='baseline-final' or x['invalid']:assert x['exit']==1 and 'exact arithmetic in machine `second`' in x['stderr'] and 'may overflow `u64`' in x['stderr'],x
  else:assert x['exit']==0 and 'native exit 0' in x['stderr'] and 'interp exit: 0' in x['stderr'],x
for tag in ['baseline','repaired']:
 controls=load('controls-'+tag+'.json');assert len(controls)==3
 for x in controls:
  assert x['exit']==1 and 'exact arithmetic' in x['stderr'],x
  p=Path(x['command'][-1]);assert digest(p)==x['sha256'];d=Path('tests/omega/fail/facts')/('nested_machine_private_arithmetic_feasible_'+x['name']+'_rejected')
  for f in ['main.omg','build.omg']:assert (p.parent/f).read_bytes()==(d/f).read_bytes()
  expected='machine `probe`' if x['name']=='self_formation' else 'machine `second`';assert expected in x['stderr'],x
x=load('counterfactual.json');assert digest(x['command'][-1])==x['sha256'] and x['exit']==255 and 'native signal 5' in x['stderr'] and 'interp crash: Trap (host presentation agrees)' in x['stderr']
source=load('corpus-source.json');both=load('corpus-both.json');assert len(source['observations'])==212 and len(both['observations'])==94
for record in [source,both]:
 assert not record['dirty'] and not record['infrastructure_error'] and not record['oracle_failures'],record['oracle_failures']
 paths=[str(Path('tests/omega')/x['fixture']) for x in record['observations']]
 subprocess.run(['git','diff','--exit-code',record['revision'],'HEAD','--',*inputs,*paths],check=True)
for x in source['observations']:
 assert x['status']==('rejected' if x['tier']=='fail' else 'checked'),x
 if x['tier']=='fail':assert x['expected_satisfied'] is True,x
for x in both['observations']:
 assert x['status']=='built',x
 if x['fixture']=='pass/facts/nested_machine_invocation/inferred_crash':assert x['facts']['exit']=='signal' and x['facts']['interpreter']=='trapped',x
 else:assert x['facts']['exit']==x['facts']['interpreter-exit']=='0' and x['facts']['interpreter']=='agrees',x
for leg in ['source','both']:
 record=load('corpus-new-'+leg+'.json');assert not record['infrastructure_error'] and not record['oracle_failures'] and len(record['observations'])==(21 if leg=='source' else 9)
 subprocess.run(['git','diff','--exit-code',record['revision'],'HEAD','--',*inputs,*[str(Path('tests/omega')/x['fixture']) for x in record['observations']]],check=True)
for x in load('golden-splice.json'):assert x['removed']==0
p=Path('build/product-nested/parser-after-407.log');q=r/'parser-admission.log';assert 'with 7 diagnostic(s)' in p.read_text() and 'with 4 diagnostic(s)' in q.read_text();assert 'may overflow' not in q.read_text()
summary=dict(parent='NESTED-MACHINE-UNIFICATION',starting_source='e522fcf48168330f1d47f16b938a89d9adc3c104',producer=load('producer.json')['revision'],source_fixtures=212,native_terminal_outcomes=94,zero_agreements=93,authored_traps=1,positive_places=9,reject_twins=9,extra_reject_controls=3,counterfactual='Both engines trap for corrupted match-result oracle; initial wrapper expected obsolete wording and its failure is preserved.',standard_library_diagnostics_before=7,standard_library_diagnostics_after=4,host='macOS ARM64',remaining='Qualified ancestor-call admission, Omega-written frontend migration, remaining custody/restoration and bounded-stack acceptance, downstream source migration. No other-host native observations.',qualification='Authenticates retained actual engine observations and exact producer/input relationships, not a new engine execution.')
(r/'acceptance.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary))
