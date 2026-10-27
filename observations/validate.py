from pathlib import Path
import json,hashlib,subprocess
r=Path('build/private-bounds')
def load(n):return json.loads((r/n).read_text())
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
inputs=['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for path,metadata,binary in [(r/'producer',r/'producer.json','omega'),(r/'producer',r/'runner-producer.json','corpus_runner')]:
 m=json.loads(metadata.read_text());assert digest(path/binary)==m['sha256'] and json.loads(m['profile'])['opt_level']=='1';subprocess.run(['git','diff','--exit-code',m['revision'],'HEAD','--',*inputs],check=True)
baseline=Path('build/posix-static/baseline-producer.json');m=json.loads(baseline.read_text());assert m['revision']=='061b60ff113a256a28fb450fd1bcf9b8b27a6bde' and digest(baseline.parent/'baseline-producer/omega')==m['sha256'] and json.loads(m['profile'])['opt_level']=='1'
for tag in ['baseline','repaired']:
 rows=load('row-'+tag+'.json');assert len(rows)==18
 for x in rows:
  p=Path(x['command'][-1]);assert digest(p)==x['source_sha256'];d=Path('tests/omega')/('fail' if x['invalid'] else 'pass')/'facts'/f"nested_machine_private_bounds_{x['place']}_{'rejected' if x['invalid'] else 'exit'}"
  for f in ['main.omg','build.omg']:assert (p.parent/f).read_bytes()==(d/f).read_bytes()
  if tag=='baseline' or x['invalid']:assert x['exit']==1 and 'cannot prove index `index`' in x['stderr'] and 'get::entry' in x['stderr'],x
  else:assert x['exit']==0 and 'native exit 0' in x['stderr'] and 'interp exit: 0' in x['stderr'],x
 controls=load('controls-'+tag+'.json');assert len(controls)==4
 for x in controls:
  assert x['exit']==1 and 'cannot prove index' in x['stderr'],x;p=Path(x['command'][-1]);assert digest(p)==x['sha256'];d=Path('tests/omega/fail/facts')/('nested_machine_private_bounds_'+x['name']+'_rejected')
  for f in ['main.omg','build.omg']:assert (p.parent/f).read_bytes()==(d/f).read_bytes()
x=load('counterfactual.json');assert digest(x['command'][-1])==x['sha256'] and x['exit']==255 and 'native signal 5' in x['stderr'] and 'interp crash: Trap (host presentation agrees)' in x['stderr']
source=load('corpus-source.json');both=load('corpus-both.json');assert len(source['observations'])==237 and len(both['observations'])==103
for record in [source,both]:
 assert not record['dirty'] and not record['infrastructure_error'] and not record['oracle_failures'],record['oracle_failures'];paths=[str(Path('tests/omega')/x['fixture']) for x in record['observations']];subprocess.run(['git','diff','--exit-code',record['revision'],'HEAD','--',*inputs,*paths],check=True)
for x in source['observations']:
 assert x['status']==('rejected' if x['tier']=='fail' else 'checked'),x
 if x['tier']=='fail':assert x['expected_satisfied'] is True,x
for x in both['observations']:
 assert x['status']=='built',x
 if x['fixture']=='pass/facts/nested_machine_invocation/inferred_crash':assert x['facts']['exit']=='signal' and x['facts']['interpreter']=='trapped',x
 else:assert x['facts']['exit']==x['facts']['interpreter-exit']=='0' and x['facts']['interpreter']=='agrees',x
for x in load('diagnostic-locations.json'):assert x['exit']==1 and 'call acknowledgements must match exactly' in x['stderr'],x
for n,x in enumerate(load('package-diagnostic-locations.json')):
 assert x['exit']==1 and 'call acknowledgements must match exactly' in x['stderr'] and 'main.omg:' in x['stderr'],x
 assert (r/'diagnostic-projects'/('library_'+str(n))/'main.omg').read_bytes()==(Path('tests/omega/fail/calls')/x['name']/'main.omg').read_bytes()
for leg in ['source','both']:
 p=load('corpus-new-'+leg+'.json');assert not p['oracle_failures'] and not p['infrastructure_error'] and len(p['observations'])==(22 if leg=='source' else 9);subprocess.run(['git','diff','--exit-code',p['revision'],'HEAD','--',*inputs,*[str(Path('tests/omega')/x['fixture']) for x in p['observations']]],check=True)
for x in load('golden-splice.json'):assert x['removed']==0
before=Path('build/posix-static/after-acknowledgements.log').read_text();after=(r/'parser-admission.log').read_text();assert 'with 39 diagnostic(s)' in before and 'with 22 diagnostic(s)' in after
assert not subprocess.check_output(['git','diff','45ece93c10b70e945b479a24122c5390df5aa94a','HEAD','--','omega-rust/psi/foundation/diagnostics/src/bounded.rs'],text=True)
summary=dict(parent='NESTED-MACHINE-UNIFICATION',source_fixtures=237,native_terminal_outcomes=103,zero_agreements=102,authored_traps=1,positive_places=9,reject_twins=9,extra_reject_controls=4,producer=load('producer.json')['revision'],host='macOS ARM64',stdlib_diagnostics_before_bounds=39,stdlib_diagnostics_after_bounds=22,source_migration='POSIX helpers now free explicit machines;198 authored hosted-adapter blocking calls acknowledged; error locations retain actual call spans. Temporary presentation override fully restored.',remaining='Cyclic bounds, effectful runtime completion and joint cycles prevent std admission. Omega-written frontend and downstream migration and full parent acceptance remain open. Other-host native not claimed.',qualification='Authenticates original observations and unchanged consuming source closure; does not execute or relabel engines.')
(r/'acceptance.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary))
