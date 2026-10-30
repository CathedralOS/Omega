import os,subprocess,sys,shutil,json
from pathlib import Path
root=Path.cwd();out=root/'build/source-resolution-remaining';out.mkdir(exist_ok=True)
paths={
 'omega-rust/omega/application/build/src/build_execution/source_resolution.rs':['resolve_project_sources','resolve_acquired_sources'],
 'omega-rust/omega/application/build/src/build_execution/source_resolution/readiness.rs':['execute_source','resolve_graph','join_sources','build_request'],
 'omega-rust/omega/compilation/driver/src/checked/compilation_session.rs':['begin_build','prepare_source_with_inputs'],
 'omega-rust/omega/compilation/driver/src/checked/checking.rs':['prepare_checked_source','prepare_typed_frontier','prepare_build_source_invocation'],
 'omega-rust/omega/compilation/driver/src/checked/checking/build_continuation.rs':['prepare_build_invocation'],
}
original={root/p:(root/p).read_bytes() for p in paths}
assert not subprocess.check_output(['git','diff','--','omega-rust'])
os.environ['MBX_CACHE_DIR']=str(root/'build/conjunction-flow/recovery-cache');os.environ['MBX_LEARNED_INCREMENTAL']='0'
try:
 for relative,names in paths.items():
  p=root/relative;source=original[p].decode()
  for name in names:
   token='fn '+name+'(';assert source.count(token)==1,(p,name)
   at=source.index('{',source.index(token))+1
   source=source[:at]+'\n    let _resolution_detail = phase_profile::guard("source.remaining.'+name+'");'+source[at:]
  if relative.endswith('/source_resolution.rs'):
   for name in ['source_acquisitions','acquire_root']:
    old='preparation.'+name+'().map_err(Preparation)?'
    assert source.count(old)==1
    source=source.replace(old,'phase_profile::scope("source.remaining.'+name+'", || preparation.'+name+'()).map_err(Preparation)?')
   old='preparation\n        .complete_source_resolution(resolved)'
   assert source.count(old)==1
   source=source.replace(old,'phase_profile::scope("source.remaining.complete_resolution", || preparation.complete_source_resolution(resolved))')
  p.write_text(source,encoding='utf-8',newline='')
 (out/'diagnostic.patch').write_bytes(subprocess.check_output(['git','diff','--','omega-rust']))
 print('Building remaining source-resolution diagnostic',flush=True)
 with (out/'build.log').open('w',encoding='utf-8') as log:subprocess.run(['mbx','build','-p','omega','--locked','--target-dir','build/pass-producer'],stdout=log,stderr=subprocess.STDOUT,check=True)
 (out/'diagnostic').mkdir(exist_ok=True);shutil.copyfile(root/'build/pass-producer/debug/omega.exe',out/'diagnostic/omega.exe')
finally:
 for p,data in original.items():p.write_bytes(data)
assert not subprocess.check_output(['git','diff','--','omega-rust'])
print('Temporary instrumentation restored; six interleaved observations',flush=True)
subprocess.run([sys.executable,'wiki/drafts/measurements/spill_loop_2026_10_28/ablate.py','build/source-resolution/candidate/omega.exe',str(out/'diagnostic/omega.exe'),str(out/'screen')],check=True)
rows=json.loads((out/'screen/runs.json').read_text());assert len(rows)==6 and len({r['artifact_sha256'] for r in rows})==1
