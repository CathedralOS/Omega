from pathlib import Path
import hashlib,json,shutil,subprocess
root=Path.cwd();fact=Path('samples/cli/facts/open_const_implementation');out=Path('build/swarm/nov1')
archive=out/'original-open-const-row';archive.mkdir(exist_ok=True)
for name in subprocess.check_output(['git','ls-files',str(fact)],text=True).splitlines():
 source=Path(name);target=archive/source.relative_to(fact);target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,target)
(out/'original-open-const-row.json').write_text(json.dumps({'source_revision':'2de5d6af94b2460e5c85e5162a72a1fbd568eee1','fact_tree':subprocess.check_output(['git','rev-parse','HEAD:'+str(fact)],text=True).strip(),'result':'second-original-row.json','sources':{str(p.relative_to(archive)):hashlib.sha256(p.read_bytes()).hexdigest() for p in archive.rglob('*') if p.is_file()}},indent=2)+'\n')
# Strengthen only runtime falsifiers after the unmodified row passed.
for invalid in [False,True]:
 field=fact/'field'/('invalid/main.omg' if invalid else 'main.omg');s=field.read_text().replace('data Envelope { value: u64; }','data Envelope { before: u64; value: u64; after: u64; }').replace('Envelope { value: construct<3>() }','Envelope { before: 13, value: construct<3>(), after: 17 }').replace('transition difference == 0 {','transition difference == 0 && envelope.before == 13 && envelope.after == 17 {');field.write_text(s)
 match=fact/'match_join'/('invalid/main.omg' if invalid else 'main.omg');s=(out/'probes/runtime_match_paths/main.omg').read_text()
 if invalid:s=s.replace('selected<Base + 4, Selected<Base + 4>>()','selected<Base + 4, Selected<Base + 3>>()')
 match.write_text(s)
# Same-row productions: private record constant closure across a package and
# a statement-position selected call whose write result becomes a constant.
variants=[(out/'probes/open_const_package_record',fact/'const_name/package_projection'),(out/'probes/statement_const_result',fact/'const_name/statement_result')]
for source,destination in variants:
 destination.mkdir(parents=True,exist_ok=True)
 for path in source.rglob('*.omg'):
  if 'build' in path.relative_to(source).parts:continue
  target=destination/path.relative_to(source);target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,target)
 invalid=destination/'invalid';invalid.mkdir(exist_ok=True)
 for path in list(destination.rglob('*.omg')):
  if 'invalid' in path.relative_to(destination).parts or 'build' in path.relative_to(destination).parts:continue
  target=invalid/path.relative_to(destination);target.parent.mkdir(parents=True,exist_ok=True);target.write_text(path.read_text().replace('selected<Base + 4, Selected<Base + 4>>()','selected<Base + 4, Selected<Base + 3>>()'))
 shutil.copy2(fact/'local/invalid/sample.json',invalid/'sample.json')
# A stronger concrete member does not donate equality to its generic caller.
control=fact/'controls/private_result_guarantee';control.mkdir(parents=True,exist_ok=True)
shutil.copy2(out/'probes/private_result_guarantee/main.omg',control/'main.omg');shutil.copy2(out/'probes/private_result_guarantee/build.omg',control/'build.omg')
(control/'sample.json').write_text(json.dumps({'role':'intended-invalid','entry':'main.omg','rejection_contains':['cannot prove ensures contract for exit from selected','result == Minimum']},indent=2)+'\n')
# Existing mirrors stay byte-identical to their place sources.
for place in ['field','match_join']:
 for tier,source,suffix in [('pass',fact/place,'exit'),('fail',fact/place/'invalid','rejected')]:
  destination=Path('tests/omega')/tier/'facts'/('open_const_implementation_through_'+place+'_'+suffix)
  for name in ['main.omg','build.omg']:shutil.copy2(source/name,destination/name)
# Pin additional productions within the same fact family.
for _,source in variants:
 name='private_record_package' if source.name=='package_projection' else 'statement_const_result'
 for tier,path,suffix in [('pass',source,'exit'),('fail',source/'invalid','rejected')]:
  destination=Path('tests/omega')/tier/'facts'/('open_const_implementation_'+name+'_'+suffix);destination.mkdir(parents=True,exist_ok=True)
  for file in path.rglob('*.omg'):
   relative=file.relative_to(path)
   if 'invalid' in relative.parts or 'build' in relative.parts:continue
   target=destination/relative;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(file,target)
  if tier=='pass':
   for oracle in ['expected_native_exit.txt','expected_interpreter_exit.txt']:(destination/oracle).write_text('0\n')
  else:(destination/'expected.txt').write_text('expected a complete `Reading` implementation with the instantiated trait arguments\n')
destination=Path('tests/omega/fail/facts/open_const_implementation_private_result_guarantee');destination.mkdir(parents=True,exist_ok=True)
for name in ['main.omg','build.omg']:shutil.copy2(control/name,destination/name)
(destination/'expected.txt').write_text('cannot prove ensures contract for exit from selected\nresult == Minimum\n')
print('Retained original passing row, strengthened field/join runtime falsifiers, and pinned same-fact closure/statement/private-guarantee evidence.')
