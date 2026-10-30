import os,subprocess,sys,shutil,json
from pathlib import Path
root=Path.cwd();out=root/'build/source-resolution';scope=root/'omega-rust/omega'
paths={scope/'application/build/src/build_execution/source_resolution/readiness.rs':['execute_source','resolve_graph','join_sources'],scope/'compilation/driver/src/checked/compilation_session/source_graph.rs':[]}
original={p:p.read_bytes() for p in paths}
os.environ['MBX_CACHE_DIR']=str(root/'build/conjunction-flow/recovery-cache');os.environ['MBX_LEARNED_INCREMENTAL']='0'
try:
 for p,names in paths.items():
  source=original[p].decode()
  for name in names:
   token='fn '+name;assert source.count(token)==1,(p,name)
   at=source.index('{',source.index(token))+1
   source=source[:at]+'\n        let _detail_guard = phase_profile::guard("source.detail.'+name+'");'+source[at:]
  if p.name=='source_graph.rs':
   source=source.replace('#[derive(compiler_storage::Record)]\n','',1)
   at=source.index('pub(super) struct SourceGraphCache')
   implementation="""impl Record for SourceGraph {
    fn write(&self, output: &mut RecordWriter) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| { self.preparation.write(output)?; self.sources.write(output)?; self.checks.write(output) })
    }
    fn read<Context>(input: &mut compiler_storage::RecordReader<'_, Context>) -> Result<Self, compiler_storage::RecordError> {
        input.record(|input| {
            Ok(Self {
                preparation: phase_profile::scope("source.detail.decode_preparation", || Record::read(input))?,
                sources: phase_profile::scope("source.detail.decode_sources", || Record::read(input))?,
                checks: phase_profile::scope("source.detail.decode_checks", || Record::read(input))?,
            })
        })
    }
}

"""
   source=source[:at]+implementation+source[at:]
  p.write_text(source,encoding='utf-8',newline='')
 (out/'diagnostic.patch').write_bytes(subprocess.check_output(['git','diff','--','omega-rust']))
 print('Building source-resolution attribution producer',flush=True)
 with (out/'build.log').open('w',encoding='utf-8') as log:subprocess.run(['mbx','build','-p','omega','--locked','--target-dir','build/pass-producer'],stdout=log,stderr=subprocess.STDOUT,check=True)
 (out/'diagnostic').mkdir(exist_ok=True);shutil.copyfile(root/'build/pass-producer/debug/omega.exe',out/'diagnostic/omega.exe')
finally:
 for p,data in original.items():p.write_bytes(data)
assert not subprocess.check_output(['git','diff','--','omega-rust'])
print('Source restored; six interleaved native comparisons',flush=True)
subprocess.run([sys.executable,'wiki/drafts/measurements/spill_loop_2026_10_28/ablate.py','build/flow-evaluation/candidate/omega.exe',str(out/'diagnostic/omega.exe'),str(out/'screen')],check=True)
rows=json.loads((out/'screen/runs.json').read_text());assert len({r['artifact_sha256'] for r in rows})==1
print('Six successful compile/native observations with matching images.',flush=True)
