from pathlib import Path
import subprocess,shutil,json,hashlib
f=Path('build/import-custody');base=(f/'base.txt').read_text().strip();rows=[]
def run(command,name):
 result=subprocess.run(command,capture_output=True,text=True)
 (f/(name+'.stdout')).write_text(result.stdout);(f/(name+'.stderr')).write_text(result.stderr)
 rows.append(dict(command=command,exit=result.returncode,name=name));(f/'checks.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,result.returncode,flush=True)
 return result
run(['mbx','nextest','run','-p','package-compilation'],'package-tests')
filters=(f/'corpus-filter.txt').read_text().strip()
for mode in ['check','both']:
 result=run(['python3','tools/corpus_gate.py','--compare',base,'--filter',filters,'--json',str(f/('corpus-'+mode+'.json'))]+(['--both'] if mode=='both' else []),'corpus-'+mode)
 if mode=='check':
  for name,root in [('candidate',Path('target')),('baseline',Path('build/cmp/target'))]:
   paths=[p for p in root.glob('debug/build/compiler/*/out/corpus_runner-*') if p.suffix=='']
   assert paths,(name,'missing runner')
   source=max(paths,key=lambda p:p.stat().st_mtime);dest=f/('corpus-runner-'+name);shutil.copy2(source,dest)
   profile=Path(str(source)+'.compiler-profile.json')
   if profile.exists():shutil.copy2(profile,str(dest)+'.compiler-profile.json')
run(['python3',str(f/'capture_corpus.py')],'corpus-raw')
run([str((f/'omega-candidate').resolve()),'run','--both','--keep','samples/cli/facts/length/local/main.omg'],'length-both')
run(['mbx','clippy','-p','package-compilation','--','-D','warnings'],'clippy')
run(['git','diff','--check'],'diff-check')
run(['rustfmt','--edition','2024','--check','omega-rust/omega/packages/compilation-inputs/src/source_assembly.rs','omega-rust/omega/packages/compilation-inputs/src/source_imports/frontier.rs','omega-rust/omega/packages/compilation-inputs/src/source_imports/import_queue.rs'],'format')
