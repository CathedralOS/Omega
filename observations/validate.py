from pathlib import Path
import json,hashlib,subprocess,collections
r=Path('build/private-index-probe');inputs=['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']
def load(n):return json.loads((r/n).read_text())
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def same_source(revision,fixtures=()):subprocess.run(['git','diff','--exit-code',revision,'HEAD','--',*inputs,*[str(Path('tests/omega')/p) for p in fixtures]],check=True)
def same_outcome(a,b,fields):return all(a[k]==b[k] for k in fields)
for name,binary in [('producer-3.json','omega'),('runner-producer.json','corpus_runner')]:
 m=load(name);assert digest(r/'producer-3'/binary)==m['sha256'] and json.loads(m['profile'])['opt_level']=='1';same_source(m['revision'])
for p,h in load('frozen-row.json')['files'].items():assert digest(p)==h,p
baseline=load('row-baseline-formed.json');assert len(baseline)==18
for a in baseline:assert digest(a['command'][-1])==a['source_sha256'] and a['exit']==1 and 'cannot prove index `window.index` is within length 512 in read::entry' in a['stderr'],a
rows=load('row-repaired-3.json');controls=load('controls-final.json');assert len(rows)==18 and len(controls)==11
for a in rows+controls:
 p=Path(a['command'][-1]);assert digest(p)==a.get('source_sha256',a.get('sha256'));name=a.get('place',a.get('name'));bad=a['invalid'];target=Path('tests/omega')/('fail' if bad else 'pass')/'facts'/('nested_machine_wrapping_index_'+name+('_rejected' if bad else '_exit'))
 for n in ['main.omg','build.omg']:assert (p.parent/n).read_bytes()==(target/n).read_bytes(),target
 if bad:
  expected=(target/'expected.txt').read_text().strip();assert a['exit']==1 and expected in a['stderr'],a
 else:assert a['exit']==0 and 'native exit 0' in a['stderr'] and 'interp exit: 0' in a['stderr'],a
old_controls=load('controls-baseline-headers.json');assert len(old_controls)==11
for a in old_controls:
 assert digest(a['command'][-1])==a['sha256']
 if a['name']=='actual_wrap':assert a['exit']==201 and 'native exit 0' in a['stderr'] and 'VerifiedOperationMalformed' in a['stderr']
 elif a['name']=='actual_clamp':assert a['exit']==201 and 'native signal 5' in a['stderr'] and 'VerifiedOperationMalformed' in a['stderr']
 elif not a['invalid']:assert a['exit']==200 and 'cannot prove index' in a['stderr']
 else:assert a['exit']==1 and ('exact arithmetic' if a['name']=='exact_child_formation' else 'cannot prove index') in a['stderr']
a=load('counterfactual.json');assert digest(a['command'][-1])==a['sha256'] and a['exit']==255 and 'native signal 5' in a['stderr'] and 'interp crash: Trap (host presentation agrees)' in a['stderr']
for name,count in [('pins-source.json',29),('pins-both.json',15)]:
 x=load(name);assert len(x['observations'])==count and x['verdict']=='recorded' and not x['oracle_failures'] and not x['infrastructure_error'];assert all(a['recorded_at']==x['revision'][:12] for a in x['observations']);same_source(x['revision'],[a['fixture'] for a in x['observations']])
 for a in x['observations']:
  if name=='pins-source.json':assert a['status']==('rejected' if a['tier']=='fail' else 'checked') and (a['tier']!='fail' or a['expected_satisfied'] is True),a
  else:assert a['status']=='built' and a['facts']['exit']==a['facts']['interpreter-exit']=='0' and a['facts']['interpreter']=='agrees',a
source=load('corpus-source.json');both=load('corpus-both.json');assert len(source['observations'])==523 and len(both['observations'])==240
old_source={a['fixture']:a for n in ['baseline-source-selected.json','baseline-source-extra.json'] for a in load(n)['observations']};old_both={a['fixture']:a for a in load('baseline-both-selected.json')['observations']};assert len(old_source)==14 and len(old_both)==12
for x in [source,both]:
 assert not x['dirty'] and not x['infrastructure_error'];same_source(x['revision'],[a['fixture'] for a in x['observations']])
for a in source['observations']:
 if a['fixture'] in old_source:assert same_outcome(a,old_source[a['fixture']],['status','expected_satisfied','diagnostics']),a
 else:assert a['status']==('rejected' if a['tier']=='fail' else 'checked') and (a['tier']!='fail' or a['expected_satisfied'] is True),a
for a in both['observations']:
 if a['fixture'] in old_both:assert same_outcome(a,old_both[a['fixture']],['status','facts','diagnostics']),a
 elif a['status']=='checked':assert not a['facts'],a
 elif a['fixture']=='pass/facts/nested_machine_invocation/inferred_crash':assert a['status']=='built' and a['facts']['exit']=='signal' and a['facts']['interpreter']=='trapped',a
 else:assert a['status']=='built' and a['facts']['interpreter']=='agrees' and a['facts']['exit']==a['facts']['interpreter-exit'],a
assert not source['oracle_failures'];assert all(any(message.startswith(fixture+':') for fixture in old_both) for message in both['oracle_failures'])
for x in [load('baseline-source-selected.json'),load('baseline-source-extra.json'),load('baseline-both-selected.json')]:assert not x['infrastructure_error']
old=Path('build/private-cyclic/producer-4/corpus_runner');m=json.loads(Path('build/private-cyclic/runner-producer.json').read_text());assert digest(old)==m['sha256'] and json.loads(m['profile'])['opt_level']=='1'
assert not (r/'format.log').read_text();assert 'Finished' in (r/'clippy.log').read_text() and 'error:' not in (r/'clippy.log').read_text()
std=load('parser-admission.json');assert isinstance(std['exit'],int) and std['exit']!=0;assert '7 diagnostic(s)' in (r/'parser-admission.log').read_text();assert 'cannot prove index' not in (r/'parser-admission.log').read_text()
acceptance=dict(parent='NESTED-MACHINE-UNIFICATION',producer=load('producer-3.json')['revision'],runner=load('runner-producer.json')['revision'],host='macOS ARM64',new_source_fixtures=29,new_native_terminal_zero_agreements=15,nine_positive_places=True,nine_fact_sensitive_twins=True,focused_source_observations=523,intended_source_outcomes=509,inherited_source_failures_unchanged=sorted(old_source),focused_both_observations=240,native_terminal_exit_agreements=223,authored_trap_agreements=1,declaration_only_checks=4,inherited_runtime_refusals_unchanged=sorted(old_both),qualification='Authenticates actual retained source/runtime observations and baseline attribution, not another runtime execution. Raw gate exits remain3 for unchanged inherited failures; their goldens were not repinned.',remaining='See actual std admission log; full library/product-source declaration migration, broader completion obligations and other-host native execution remain open.')
(r/'acceptance.json').write_text(json.dumps(acceptance,indent=2)+'\n');print(json.dumps(acceptance))
