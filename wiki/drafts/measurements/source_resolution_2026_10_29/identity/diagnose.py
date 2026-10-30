import os,subprocess,sys,shutil,json
from pathlib import Path
root=Path.cwd();out=root/'build/source-resolution/identity';scope=root/'omega-rust/omega'
paths={scope/'compilation/driver/src/checked/compilation_session.rs':[]}
original={p:p.read_bytes() for p in paths}
os.environ['MBX_CACHE_DIR']=str(root/'build/conjunction-flow/recovery-cache');os.environ['MBX_LEARNED_INCREMENTAL']='0'
try:
 for p,names in paths.items():
  source=original[p].decode()
  for name in names:
   token='fn '+name;assert source.count(token)==1,(p,name)
   at=source.index('{',source.index(token))+1
   source=source[:at]+'\n        let _detail_guard = phase_profile::guard("source.detail.'+name+'");'+source[at:]
  old='        self.restore_source_graph();';assert source.count(old)==1
  source=source.replace(old,old+"""
        if phase_profile::enabled() {
            eprintln!("RESTORE request {:?} sources={}",root_path,self.sources.len());
            for prepared in &self.sources {
                eprintln!("RESTORE candidate {:?} root={} inputs={} capture={}",prepared.root,prepared.root==root_path,prepared.inputs==inputs,prepared.capture.as_ref()==capture);
            }
        }
""")
  p.write_text(source,encoding='utf-8',newline='')
 (out/'diagnostic.patch').write_bytes(subprocess.check_output(['git','diff','--','omega-rust']))
 print('Building source-resolution attribution producer',flush=True)
 with (out/'build.log').open('w',encoding='utf-8') as log:subprocess.run(['mbx','build','-p','omega','--locked','--target-dir','build/pass-producer'],stdout=log,stderr=subprocess.STDOUT,check=True)
 (out/'diagnostic').mkdir(exist_ok=True);shutil.copyfile(root/'build/pass-producer/debug/omega.exe',out/'diagnostic/omega.exe')
finally:
 for p,data in original.items():p.write_bytes(data)
assert not subprocess.check_output(['git','diff','--','omega-rust'])
print('Source restored; identity diagnostic native check',flush=True)
subprocess.run([sys.executable,'wiki/drafts/measurements/spill_loop_2026_10_28/profile.py',str(out/'diagnostic/omega.exe'),str(out/'observation')],stdout=(out/'profile.log').open('w'),check=True)
row=json.loads((out/'observation/result.json').read_text());baseline=json.loads((root/'build/source-resolution/screen/runs.json').read_text())[0]
assert row['artifact_sha256']==baseline['artifact_sha256'] and row['native_exit']==0
print('Identity observation matches baseline native image.',flush=True)
