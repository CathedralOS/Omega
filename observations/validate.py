from pathlib import Path
import hashlib,json,subprocess
r=Path('build/private-cyclic')
inputs=['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']
def load(name):return json.loads((r/name).read_text())
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
for name,binary in [('producer-4.json','omega'),('runner-producer.json','corpus_runner')]:
 m=load(name);assert digest(r/'producer-4'/binary)==m['sha256'];assert json.loads(m['profile'])['opt_level']=='1'
 subprocess.run(['git','diff','--exit-code',m['revision'],'HEAD','--',*inputs],check=True)
for name in ['frozen-root.json','frozen-row.json']:
 for path,expected in load(name)['files'].items():assert digest(path)==expected,path
for name,count in [('root-baseline.json',2),('row-baseline.json',18)]:
 rows=load(name);assert len(rows)==count
 for x in rows:
  assert digest(x['command'][-1])==x['source_sha256']
  assert x['exit']==1 and 'assignment may overflow `u64`' in x['stderr'],x
for name,count in [('row-repaired-4.json',18),('controls-repaired-4.json',7)]:
 rows=load(name);assert len(rows)==count
 for x in rows:
  p=Path(x['command'][-1]);assert digest(p)==x.get('source_sha256',x.get('sha256'))
  suffix=x.get('place',x.get('name'));bad=x['invalid'];target=Path('tests/omega')/('fail' if bad else 'pass')/'facts'/('nested_machine_private_cyclic_'+suffix+('_rejected' if bad else '_exit'))
  for n in ['main.omg','build.omg']:assert (p.parent/n).read_bytes()==(target/n).read_bytes()
  if bad:assert x['exit']==1 and 'assignment may overflow `u64`' in x['stderr'],x
  else:assert x['exit']==0 and 'native exit 0' in x['stderr'] and 'interp exit: 0' in x['stderr'],x
assert (r/'root-repaired-4.log').read_text()=='native exit 0\ninterp exit: 0\n'
root=Path('samples/cli/facts/nested_machine_invocation/private_arithmetic/cyclic')
for bad in [False,True]:
 p=root/'invalid' if bad else root;t=Path('tests/omega')/('fail' if bad else 'pass')/'facts'/('nested_machine_private_cyclic_root_'+('rejected' if bad else 'exit'))
 for n in ['main.omg','build.omg']:assert (p/n).read_bytes()==(t/n).read_bytes()
x=load('counterfactual.json');assert digest(x['command'][-1])==x['sha256'] and x['exit']==255 and 'native signal 5' in x['stderr'] and 'interp crash: Trap (host presentation agrees)' in x['stderr']
for name,count,mode in [('pins-source.json',27,'source'),('pins-both.json',12,'both'),('corpus-source.json',315,'source'),('corpus-both.json',141,'both')]:
 x=load(name);assert len(x['observations'])==count,(name,len(x['observations']))
 assert not x['oracle_failures'] and not x['infrastructure_error'],name
 if name.startswith('pins-'):
  # Recording dirties the golden itself; source stamp excludes only record targets.
  assert x['verdict']=='recorded' and all(a['recorded_at']==x['revision'][:12] for a in x['observations']),name
 else:assert not x['dirty'],name
 subprocess.run(['git','diff','--exit-code',x['revision'],'HEAD','--',*inputs,*[str(Path('tests/omega')/a['fixture']) for a in x['observations']]],check=True)
 for a in x['observations']:
  if mode=='source':
   assert a['status']==('rejected' if a['tier']=='fail' else 'checked'),a
   if a['tier']=='fail':assert a['expected_satisfied'] is True,a
  else:
   if a['fixture']=='pass/calls/terminal_call_acknowledgements_declarations':
    assert a['status']=='checked' and not a['facts'],a
    continue
   assert a['status']=='built',a
   if a['fixture']=='pass/facts/nested_machine_invocation/inferred_crash':assert a['facts']['exit']=='signal' and a['facts']['interpreter']=='trapped',a
   else:assert a['facts']['exit']==a['facts']['interpreter-exit']=='0' and a['facts']['interpreter']=='agrees',a
previous=json.loads(Path('build/effectful-integration/corpus-source.json').read_text())
old_moves={a['fixture']:a for a in previous['fixtures'] if a['verdict']=='moved'}
new_moves={a['fixture']:a for a in load('corpus-source.json')['fixtures'] if a['verdict']=='moved'}
assert old_moves.keys()==new_moves.keys()
for fixture in new_moves:assert old_moves[fixture]['after']['diagnostics']==new_moves[fixture]['after']['diagnostics'],fixture
assert not (r/'format.log').read_text()
assert 'Finished' in (r/'clippy.log').read_text() and 'error:' not in (r/'clippy.log').read_text()
text=(r/'parser-admission-4.log').read_text();assert '14 diagnostic(s)' in text;assert 'exact arithmetic in machine `rds_pop_go`' not in text and 'exact arithmetic in machine `rds_push_go`' not in text
x=dict(parent='NESTED-MACHINE-UNIFICATION',producer=load('producer-4.json')['revision'],runner=load('runner-producer.json')['revision'],host='macOS ARM64',new_source_fixtures=27,new_native_terminal_outcomes=12,focused_source_fixtures=315,focused_both_observations=141,focused_native_terminal_outcomes=140,zero_agreements=139,authored_traps=1,declaration_only_checks=1,positive_places=9,reject_twins=9,root_positive_and_twin=True,extra_positive_controls=2,extra_reject_controls=5,remaining='14 standard-library diagnostics, broader cyclic invariants/completion contracts, Omega-written frontend and downstream migration; other-host native unverified.',qualification='Authenticates retained actual observations and source/producer identities. No fresh runtime execution is asserted by this validator.',prerequisite='Published PR435 joint natural SCC rank repair integrated for three newly exposed library rank obligations; memo conflict preserves immutable graph cache via shared Arc<ClosedArrivals>.')
(r/'acceptance.json').write_text(json.dumps(x,indent=2)+'\n');print(json.dumps(x))
