import hashlib, json, shutil, sys
from pathlib import Path
sys.path.insert(0, str(Path('tools').resolve()))
from cache_gate import Gate
from compiler_profile import require_optimized
root=Path('build/ui-native-round').resolve()
output=root/'graph-checks-bounded'
output.mkdir()
compilers={'full':root/'omega-detailed.exe','scoped':root/'omega-scoped.exe'}
gates={name:Gate(compiler,output,'windows_x86_64') for name,compiler in compilers.items()}
positive=['domains/moved_payload_uses_current_membership_exit','traits/runtime_dyn_post_write_alternatives_exit','traits/runtime_dyn_selector_record_post_write_exit','traits/runtime_dyn_byte_subview_join','borrows/completed_reference_result_preserves_input_holds_exit']
negative=['domains/moved_payload_cannot_revive_issued_membership','traits/dyn_selector_prefix_write_invalidates','proofs/returned_reference_unproven_result_guarantee']
record=dict(status='failed',compilers={name:require_optimized(compiler) for name,compiler in compilers.items()},cases=[])
try:
 for category,cases in [('pass',positive),('fail',negative)]:
  for ordinal,fixture in enumerate(cases):
   source=Path('tests/omega')/category/fixture
   project=output/'projects'/category/fixture
   shutil.copytree(source,project,ignore=shutil.ignore_patterns('build','omega.lock'))

   expected=int((source/'expected_native_exit.txt').read_text()) if category=='pass' else None
   fragments=[line.strip() for line in (source/'expected.txt').read_text().splitlines() if line.strip()] if category=='fail' else []
   row=dict(fixture=str(source),observations={},inputs={str(p.relative_to(project)):hashlib.sha256(p.read_bytes()).hexdigest() for p in project.rglob('*.omg')})
   record['cases'].append(row)
   for name,gate in gates.items():
    cache=root/('cache-checks-'+name)
    label=f'{category}-{ordinal}-{name}'
    if category=='fail':
     code,log=gate.command(['--check',project/'main.omg'],project,cache,label+'-check')
     assert code==1,(label,code,log[-4000:])
     result=('rejected',tuple(line.strip() for line in log.splitlines() if line.strip().startswith('error:')))
     assert result[1],label
    else:
     result,log=gate.observe(project,cache,label,expected=expected)
    assert all(fragment in log for fragment in fragments),(label,fragments,log[-4000:])
    row['observations'][name]=result
    if category=='pass':
     code,text=gate.command(['run','--both',project/'main.omg'],project,cache,label+'-both')
     row[name+'_both'] = dict(exit_code=code, agreed=code==expected and f'interp exit: {expected}' in text and 'DECLINED' not in text, diagnostics=[line for line in text.splitlines() if line.startswith(('native ', 'interp ', 'Terminal interpretation', '  error:'))])
   assert row['observations']['full']==row['observations']['scoped'],row
   if category=='pass':
    assert row['full_both']==row['scoped_both'],row
   print(category,fixture,'matched',flush=True)
 record['status']='passed'
finally:
 (output/'result.json').write_text(json.dumps(record,indent=2),encoding='utf-8')
