import os,subprocess,sys,shutil,json
from pathlib import Path
root=Path.cwd();out=root/'build/source-resolution-remaining/detail';out.mkdir(exist_ok=True)
paths=['omega-rust/omega/application/build/src/build_execution/source_resolution.rs','omega-rust/omega/compilation/build-evaluation/src/build_evaluation.rs']
original={root/p:(root/p).read_bytes() for p in paths}
assert not subprocess.check_output(['git','diff','--','omega-rust'])
os.environ['MBX_CACHE_DIR']=str(root/'build/conjunction-flow/recovery-cache');os.environ['MBX_LEARNED_INCREMENTAL']='0'
try:
 p=root/paths[0];source=original[p].decode()
 old='acquisitions\n        .retain_root(&acquired)';assert source.count(old)==1
 source=source.replace(old,'phase_profile::scope("source.detail.retain_root", || acquisitions.retain_root(&acquired))');p.write_text(source,encoding='utf-8',newline='')
 p=root/paths[1];source=original[p].decode()
 # Sequential timestamps divide the same admission flow without changing its calls.
 markers=[('checked_build','    let (prepared, retained_check) = crate::admission::checking::prepare_checked_build('),('filesystem_scope','    let filesystem_scope = crate::prepare_filesystem_scope('),('admit_build','    let admitted = crate::admit_build_program('),('restricted_requests','    let restricted_build_requests = admitted.restricted_build_requests();'),('pure_inputs','    let inputs = admitted.pure_consumption_inputs(package_inputs, snapshot);'),('pure_lookup','    let reusable = inputs'),('execution_prepare','    let state = if let Some(configuration) = reusable'),('finish','    Ok(PreparedBuildEvaluation {\n        state,')]
 previous=None
 for name,marker in markers:
  assert source.count(marker)==1,(name,source.count(marker))
  added=''
  if previous:added=f'    if std::env::var_os("OMEGA_PHASE_PROFILE").is_some() {{ eprintln!("RESOLUTION_DETAIL {previous} {{:.3}}", resolution_detail_started.elapsed().as_secs_f64()*1000.0); }}\n'
  if name!='finish':added+='    let resolution_detail_started = std::time::Instant::now();\n'
  source=source.replace(marker,added+marker);previous=name
 p.write_text(source,encoding='utf-8',newline='')
 (out/'diagnostic.patch').write_bytes(subprocess.check_output(['git','diff','--','omega-rust']))
 print('Building retained-root and Build admission detail',flush=True)
 with (out/'build.log').open('w',encoding='utf-8') as log:subprocess.run(['mbx','build','-p','omega','--locked','--target-dir','build/pass-producer'],stdout=log,stderr=subprocess.STDOUT,check=True)
 (out/'diagnostic').mkdir(exist_ok=True);shutil.copyfile(root/'build/pass-producer/debug/omega.exe',out/'diagnostic/omega.exe')
finally:
 for p,data in original.items():p.write_bytes(data)
assert not subprocess.check_output(['git','diff','--','omega-rust'])
print('Source restored; six detailed comparisons',flush=True)
subprocess.run([sys.executable,'wiki/drafts/measurements/spill_loop_2026_10_28/ablate.py','build/source-resolution/candidate/omega.exe',str(out/'diagnostic/omega.exe'),str(out/'screen')],check=True)
rows=json.loads((out/'screen/runs.json').read_text());assert len(rows)==6 and len({r['artifact_sha256'] for r in rows})==1
