from pathlib import Path
import subprocess,json
f=Path('build/import-index');rows={}
for leg in ['baseline','candidate']:
 binary=f/('omega-'+leg)
 names=['package_compilation::source_imports::discover_imports','package_compilation::source_imports::discover_package_imports','package_compilation::source_imports::discover_imports_with_packages','tokens_to_syntax_trees::parser::parse']
 outputs=[]
 for name in names:
  result=subprocess.run(['xcrun','llvm-objdump','-d','--demangle','--disassemble-symbols='+name,str(binary)],capture_output=True,text=True)
  if result.returncode:raise RuntimeError(result.stderr)
  outputs.append(result.stdout)
 (f/('assembly-'+leg+'.txt')).write_text('\n'.join(outputs));rows[leg]=names
(f/'assembly-symbols.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(rows,indent=2))
