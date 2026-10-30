import hashlib,json,sys
from pathlib import Path
sys.path.insert(0,str(Path('tools').resolve()))
from cache_gate import Gate
root=Path('build/ui-native-round').resolve()
output=root/'graph-terminal'
output.mkdir()
record=dict(status='failed',cases=[])
try:
 for index,fixture in enumerate(['domains/moved_payload_uses_current_membership_exit','traits/runtime_dyn_post_write_alternatives_exit']):
  project=root/'graph-checks-bounded/projects/pass'/fixture
  row=dict(fixture=fixture,artifacts={})
  record['cases'].append(row)
  for name,filename in [('full','omega-detailed.exe'),('scoped','omega-scoped.exe')]:
   gate=Gate(root/filename,output,'windows_x86_64')
   cache=root/('cache-checks-'+name)
   code,log=gate.prepare(project,cache,f'{index}-{name}')
   assert code==0,log[-4000:]
   artifact=output/f'{index}-{name}.oterm'
   code,log=gate.command(['inspect-terminal','--machine','main','--target','windows_x86_64','--output',artifact,project/'main.omg'],project,cache,f'{index}-{name}-produce')
   assert code==0 and artifact.is_file(),log[-4000:]
   code,log=gate.command(['inspect-terminal','--artifact',artifact],project,cache,f'{index}-{name}-validate')
   assert code==0,log[-4000:]
   row['artifacts'][name]=hashlib.sha256(artifact.read_bytes()).hexdigest()
  assert row['artifacts']['full']==row['artifacts']['scoped'],row
  print(fixture,'canonical artifacts match and validate',flush=True)
 record['status']='passed'
finally:
 (output/'result.json').write_text(json.dumps(record,indent=2),encoding='utf-8')
