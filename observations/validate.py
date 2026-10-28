from pathlib import Path
import json,subprocess,hashlib
r=Path('build/wrapping-integration');old=Path('build/private-index-probe')
def load(n):return json.loads((r/n).read_text())
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
for name in ['omega','corpus_runner']:
 m=load(name+'-producer.json');assert m['revision']==head and digest(r/'producer'/name)==m['sha256'];assert json.loads(m['profile'])['opt_level']=='1'
for p,h in json.loads((old/'frozen-row.json').read_text())['files'].items():assert digest(p)==h,p
rows=load('row.json');controls=load('controls.json');assert len(rows)==18 and len(controls)==11
for a in rows+controls:
 p=Path(a['command'][-1]);assert digest(p)==a.get('source_sha256',a.get('sha256'));name=a.get('place',a.get('name'));bad=a['invalid'];target=Path('tests/omega')/('fail' if bad else 'pass')/'facts'/('nested_machine_wrapping_index_'+name+('_rejected' if bad else '_exit'))
 for n in ['main.omg','build.omg']:assert (p.parent/n).read_bytes()==(target/n).read_bytes(),target
 if bad:assert a['exit']==1 and (target/'expected.txt').read_text().strip() in a['stderr'],a
 else:assert a['exit']==0 and 'native exit 0' in a['stderr'] and 'interp exit: 0' in a['stderr'],a
x=load('counterfactual.json');assert digest(x['command'][-1])==x['sha256'] and x['exit']==255 and 'native signal 5' in x['stderr'] and 'interp crash: Trap (host presentation agrees)' in x['stderr']
results={};inherited={}
for mode in ['source','both']:
 x=load('corpus-'+mode+'.json');prior=json.loads((old/('corpus-'+mode+'.json')).read_text());prior={a['fixture']:a for a in prior['observations']};assert not x['dirty'] and x['revision']==head and not x['infrastructure_error'];failures=[];own=[]
 for a in x['observations']:
  if mode=='source':intended=a['status']==('rejected' if a['tier']=='fail' else 'checked') and(a['tier']!='fail' or a['expected_satisfied'] is True)
  else:intended=(a['status']=='built' and a['facts'].get('interpreter')=='agrees' and a['facts'].get('exit')==a['facts'].get('interpreter-exit')) or (a['status']=='checked' and not a['facts'])
  if 'nested_machine_wrapping_index_' in a['fixture']:assert intended,a;own.append(a['fixture'])
  if not intended:
   b=prior[a['fixture']];fields=['status','expected_satisfied','diagnostics'] if mode=='source' else ['status','facts','diagnostics'];assert all(a[k]==b[k] for k in fields),(mode,a,b)
   # Exact unchanged fixture, and the previously retained baseline reproduces
   # its refusal. These are inherited failures, never repinned as success.
   fixture=Path('tests/omega')/a['fixture']
   if a['fixture']=='pass/borrow/runtime_write_only_qualified_array_element_exit':
    prior_source=subprocess.check_output(['git','show','b7dd6b93124c5912f1f6b9d48b2c522e8819e7a3:'+str(fixture/'main.omg')],text=True)
    # Incoming owner-ratified domain-binder migration only. The current
    # declaration names the identical Utf8 predicate, with self replacing
    # the old explicit bytes subject; the five refusals are unchanged.
    assert prior_source.replace('domain [u8; 8]::Utf8(bytes) requires valid_utf8(bytes);','domain [u8; 8]::Utf8 requires valid_utf8(self);')==(fixture/'main.omg').read_text()
   else:subprocess.run(['git','diff','--exit-code','b7dd6b93124c5912f1f6b9d48b2c522e8819e7a3','HEAD','--',str(fixture)],check=True)
   failures.append(a['fixture'])
 assert len(own)==(29 if mode=='source' else 15)
 results[mode]=dict(observations=len(x['observations']),intended=len(x['observations'])-len(failures),own=len(own),declaration_only=sum(a['status']=='checked' and not a['facts'] for a in x['observations']) if mode=='both' else 0,runtime_exit_agreements=sum(a['status']=='built' and a['facts'].get('interpreter')=='agrees' for a in x['observations']) if mode=='both' else 0);inherited[mode]=failures
for n in ['row','controls','format','clippy']:assert load(n+'-command.json')['exit']==0
assert not (r/'format.log').read_text()
assert 'Finished' in (r/'clippy.log').read_text() and 'error:' not in (r/'clippy.log').read_text()
std=load('parser-admission-command.json');assert std['revision']==head and (isinstance(std['exit'],int) or std['exit']=='timeout'); assert 'captured entry is absent' not in (r/'parser-admission.log').read_text()
acceptance=dict(source=head,fixed_base='baf1b6ea0633681bcc2cf34d563d748a47fadb36',integration=load('integration.json'),target='macos_arm64',nine_positive_places_and_nine_fact_sensitive_twins=True,positive_controls=6,negative_controls=5,counterfactual='Native signal5 and Terminal Trap agree',fresh=results,inherited_unchanged_failures=inherited,parser_admission=std,remaining='Full product-parser/resolver declaration migration and completion requirements remain open. Other hosted native execution remains unverified. Historical larger scope retains original producer identities.')
encoded=json.dumps(acceptance,indent=2)+'\n'
if (r/'acceptance.json').exists():assert (r/'acceptance.json').read_text()==encoded
else:(r/'acceptance.json').write_text(encoded)
print(json.dumps(acceptance))
