from pathlib import Path
import subprocess,shutil,json,hashlib
f=Path('build/path-identity');base=(f/'base.txt').read_text().strip();rows=[]
def run(command,name):
 result=subprocess.run(command,capture_output=True,text=True)
 (f/(name+'.stdout')).write_text(result.stdout);(f/(name+'.stderr')).write_text(result.stderr)
 rows.append(dict(command=command,exit=result.returncode,name=name));(f/'checks.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,result.returncode,flush=True)
 return result
# Focused tests captured separately in focused-tests.log.
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
run(['mbx','clippy','-p','package-compilation','-p','tokens-to-syntax-trees','-p','source-files-to-tokens','--no-deps','--','-D','warnings'],'clippy')
run(['git','diff','--check'],'diff-check')
run(['rustfmt','--edition','2024','--check',*['omega-rust/omega/packages/compilation-inputs/src/source_assembly.rs', 'omega-rust/omega/packages/compilation-inputs/src/source_imports.rs', 'omega-rust/omega/packages/compilation-inputs/src/source_imports/bindings.rs', 'omega-rust/omega/packages/compilation-inputs/src/source_imports/build_prelude.rs', 'omega-rust/omega/packages/compilation-inputs/src/source_imports/entry_contract_seed.rs', 'omega-rust/omega/packages/compilation-inputs/src/source_imports/frontier.rs', 'omega-rust/omega/packages/compilation-inputs/src/source_imports/project.rs', 'omega-rust/psi/compilation/pipeline/00_source-files-to-tokens/src/lex_sources.rs', 'omega-rust/psi/compilation/pipeline/00_source-files-to-tokens/src/lex_sources/tests.rs', 'omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/parser.rs', 'omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/tests/source_parsing.rs']],'format')
