from pathlib import Path
import hashlib,json,subprocess
r=Path('build/effectful-completion')
def load(n): return json.loads((r/n).read_text())
def digest(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
inputs=['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for metadata,binary in [('producer-5.json','omega'),('runner-producer.json','corpus_runner')]:
 m=load(metadata);assert digest(r/'producer-5'/binary)==m['sha256'];assert json.loads(m['profile'])['opt_level']=='1';subprocess.run(['git','diff','--exit-code',m['revision'],'HEAD','--',*inputs],check=True)
b=Path('build/private-bounds/producer.json');m=json.loads(b.read_text());assert m['revision']=='40c6fdc57750f9d3c23ac0d754c9ad4fcdb7e194' and digest(b.parent/'producer/omega')==m['sha256'];subprocess.run(['git','diff','--exit-code',m['revision'],'7c682db5a9feefc1322ac98fa6ddf18642d3766e','--',*inputs],check=True)
for p,h in load('frozen-row.json')['files'].items():assert digest(p)==h,p
for label in ['baseline','repaired-5']:
 rows=load('row-'+label+'.json');assert len(rows)==18
 for x in rows:
  p=Path(x['command'][-1]);assert digest(p)==x['source_sha256'];target=Path('tests/omega')/('fail' if x['invalid'] else 'pass')/'facts'/('nested_machine_effectful_completion_'+x['place']+('_rejected' if x['invalid'] else '_exit'))
  for n in ['main.omg','build.omg']:assert (p.parent/n).read_bytes()==(target/n).read_bytes()
  if label=='baseline' or x['invalid']:assert x['exit']==1 and 'cannot prove ensures contract for exit from probe' in x['stderr'] and 'result !=' in x['stderr'],x
  else:assert x['exit']==0 and 'native exit 0' in x['stderr'] and 'interp exit: 0' in x['stderr'],x
for x in load('controls-repaired-5.json'):
 p=Path(x['command'][-1]);assert digest(p)==x['sha256'];target=Path('tests/omega')/('fail' if x['invalid'] else 'pass')/'facts'/('nested_machine_effectful_completion_'+x['name']+('_rejected' if x['invalid'] else '_exit'))
 for n in ['main.omg','build.omg']:assert (p.parent/n).read_bytes()==(target/n).read_bytes()
 if x['invalid']:assert x['exit']==1 and 'cannot prove ensures contract for exit from probe' in x['stderr'],x
 else:assert x['exit']==0 and 'native exit 0' in x['stderr'] and 'interp exit: 0' in x['stderr'],x
old=load('controls-baseline.json');assert len(old)==5
for x in old:
 assert digest(x['command'][-1])==x['sha256']
 if x['name']=='stale_caller_storage':assert x['exit']==0 # Demonstrated old unsound acceptance, now rejected.
 elif x['invalid']:assert x['exit']==1 and 'cannot prove ensures contract' in x['stderr'],x
 else:assert x['exit']==200 and 'cannot prove ensures contract' in x['stderr'],x
x=load('counterfactual.json');assert digest(x['command'][-1])==x['sha256'] and x['exit']==255 and 'native signal 5' in x['stderr'] and 'interp crash: Trap (host presentation agrees)' in x['stderr']
source=load('corpus-source.json');both=load('corpus-both.json');assert len(source['observations'])==260 and len(both['observations'])==114
for record in [source,both]:
 assert not record['dirty'] and not record['infrastructure_error'] and not record['oracle_failures'],record['oracle_failures'];subprocess.run(['git','diff','--exit-code',record['revision'],'HEAD','--',*inputs,*[str(Path('tests/omega')/x['fixture']) for x in record['observations']]],check=True)
# Three golden diagnostics were already stale on the frozen baseline.
# Compare all 237 existing outcomes, including exact diagnostic text, rather
# than pinning unrelated old golden changes or calling the moved gate clean.
old=json.loads(Path('build/private-bounds/corpus-source.json').read_text());previous={x['fixture']:x for x in old['observations']};current={x['fixture']:x for x in source['observations']};keys=['status','expected_satisfied','facts','diagnostics'];assert len(previous)==237
for name,before in previous.items():
 assert {k:before[k] for k in keys}=={k:current[name][k] for k in keys},name
subprocess.run(['git','diff','--exit-code',old['revision'],'HEAD','--',*[str(Path('tests/omega')/name) for name in previous]],check=True)
(r/'baseline-comparison.json').write_text(json.dumps(dict(existing_source_outcomes=237,changed_outcomes=[],current_record_sha256=digest(r/'corpus-source.json'),baseline_record_sha256=digest('build/private-bounds/corpus-source.json'),baseline_revision=old['revision'],golden_differences='Three unchanged baseline diagnostic differences; not repinned.'),indent=2)+'\n')
for x in source['observations']:
 assert x['status']==('rejected' if x['tier']=='fail' else 'checked'),x
 if x['tier']=='fail':assert x['expected_satisfied'] is True,x
old_both=json.loads(Path('build/private-bounds/corpus-both.json').read_text());old_by_name={x['fixture']:x for x in old_both['observations']};new_by_name={x['fixture']:x for x in both['observations']};assert len(old_by_name)==103
for name,before in old_by_name.items():
 assert before['status']==new_by_name[name]['status'] and before['facts']==new_by_name[name]['facts'],name
for x in both['observations']:
 assert x['status']=='built',x
 if x['fixture']=='pass/facts/nested_machine_invocation/inferred_crash':assert x['facts']['exit']=='signal' and x['facts']['interpreter']=='trapped',x
 else:assert x['facts']['exit']==x['facts']['interpreter-exit']=='0' and x['facts']['interpreter']=='agrees',x
for label,count in [('source',23),('both',11)]:
 x=load('corpus-new-'+label+'.json');assert len(x['observations'])==count and not x['oracle_failures'] and not x['infrastructure_error']
for x in load('golden-splice.json'):assert x['removed']==0
assert (r/'format.log').read_text()==''
assert 'with 2 diagnostic(s)' in (r/'parser-admission.log').read_text() and 'rds_pop_go' in (r/'parser-admission.log').read_text() and 'rds_push_go' in (r/'parser-admission.log').read_text()
assert not subprocess.check_output(['git','diff','7c682db5a9f','HEAD','--','omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/exits.rs'],text=True)
x=dict(parent='NESTED-MACHINE-UNIFICATION',producer=load('producer-5.json')['revision'],runner=load('runner-producer.json')['revision'],host='macOS ARM64',positive_places=9,reject_twins=9,extra_positive_controls=2,extra_reject_controls=3,source_fixtures=260,native_terminal_outcomes=114,zero_agreements=113,authored_traps=1,remaining='Std admission currently refuses two exact-arithmetic obligations in directory-stack helpers. Earlier diagnostics may be hidden by that frontier. Product frontend and downstream migration/full parent acceptance remain open. Other-host native unverified.',qualification='Authenticates retained actual observations and unchanged consumer inputs; it neither executes nor relabels engine outcomes.')
(r/'acceptance.json').write_text(json.dumps(x,indent=2)+'\n');print(json.dumps(x))
