from pathlib import Path
import json,hashlib,subprocess
r=Path('build/effectful-integration');old=Path('build/effectful-completion')
def load(name):return json.loads((r/name).read_text())
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
inputs=['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for name,binary in [('producer.json','omega'),('runner-producer.json','corpus_runner')]:
 m=load(name);assert digest(r/'producer'/binary)==m['sha256'] and json.loads(m['profile'])['opt_level']=='1';subprocess.run(['git','diff','--exit-code',m['revision'],'HEAD','--',*inputs],check=True)
for p,h in json.loads((old/'frozen-row.json').read_text())['files'].items():assert digest(p)==h,p
rows=load('row-integrated.json');assert len(rows)==18
for x in rows:
 p=Path(x['command'][-1]);assert digest(p)==x['source_sha256'];target=Path('tests/omega')/('fail' if x['invalid'] else 'pass')/'facts'/('nested_machine_effectful_completion_'+x['place']+('_rejected' if x['invalid'] else '_exit'))
 for n in ['main.omg','build.omg']:assert (p.parent/n).read_bytes()==(target/n).read_bytes()
 if x['invalid']:assert x['exit']==1 and 'cannot prove ensures contract for exit from probe' in x['stderr'],x
 else:assert x['exit']==0 and 'native exit 0' in x['stderr'] and 'interp exit: 0' in x['stderr'],x
controls=load('controls-integrated.json');assert len(controls)==5
for x in controls:
 p=Path(x['command'][-1]);assert digest(p)==x['sha256'];target=Path('tests/omega')/('fail' if x['invalid'] else 'pass')/'facts'/('nested_machine_effectful_completion_'+x['name']+('_rejected' if x['invalid'] else '_exit'))
 for n in ['main.omg','build.omg']:assert (p.parent/n).read_bytes()==(target/n).read_bytes()
 if x['invalid']:assert x['exit']==1 and 'cannot prove ensures contract for exit from probe' in x['stderr'],x
 else:assert x['exit']==0 and 'native exit 0' in x['stderr'] and 'interp exit: 0' in x['stderr'],x
x=load('counterfactual.json');assert digest(x['command'][-1])==x['sha256'] and x['exit']==255 and 'native signal 5' in x['stderr'] and 'interp crash: Trap (host presentation agrees)' in x['stderr']
source=load('corpus-source.json');both=load('corpus-both.json');assert len(source['observations'])==260 and len(both['observations'])==114
for x in [source,both]:
 assert not x['dirty'] and not x['oracle_failures'] and not x['infrastructure_error'];subprocess.run(['git','diff','--exit-code',x['revision'],'HEAD','--',*inputs,*[str(Path('tests/omega')/v['fixture']) for v in x['observations']]],check=True)
for x in source['observations']:
 assert x['status']==('rejected' if x['tier']=='fail' else 'checked'),x
 if x['tier']=='fail':assert x['expected_satisfied'] is True,x
for x in both['observations']:
 assert x['status']=='built',x
 if x['fixture']=='pass/facts/nested_machine_invocation/inferred_crash':assert x['facts']['exit']=='signal' and x['facts']['interpreter']=='trapped',x
 else:assert x['facts']['exit']==x['facts']['interpreter-exit']=='0' and x['facts']['interpreter']=='agrees',x
assert (r/'format.log').read_text()==''
assert 'exact arithmetic' in (r/'parser-admission.log').read_text()
x=dict(parent='NESTED-MACHINE-UNIFICATION',producer=load('producer.json')['revision'],runner=load('runner-producer.json')['revision'],host='macOS ARM64',source_fixtures=260,native_terminal_outcomes=114,zero_agreements=113,authored_traps=1,positive_places=9,reject_twins=9,extra_positive_controls=2,extra_reject_controls=3,conflict_integration='Actual PR428 conflicts resolved against incoming main1762e3ac388930e835f0dbc662635f88685b58ea. Original preintegration observations/qualification remain on separate immutable records.',remaining='See actual parser-admission.log. Full library admission, Omega-written frontend/downstream migration and other-host native execution remain open.',qualification='Authenticates actual integrated-source observations and immutable inputs. No original run is relabelled.')
(r/'acceptance.json').write_text(json.dumps(x,indent=2)+'\n');print(json.dumps(x))
