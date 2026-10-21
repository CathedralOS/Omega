from pathlib import Path
import subprocess,json,hashlib,shutil,re
out=Path('build/import-assembly')
programs=[('samples/cli/facts/length/local/main.omg',0),('tests/omega/pass/modules/direct_file_import_process_exit/main.omg',7),('tests/omega/pass/build/nested_generated_dependency_dual_purpose/main.omg',0),('tests/omega/pass/build/dependency_generated_source_unimported/main.omg',0)]
records=[]
for name in ['baseline','candidate']:
 binary=out/f'omega-{name}'
 for program,expected in programs:
  command=[str(binary),'run','--both','--keep',program];result=subprocess.run(command,capture_output=True,text=True)
  row=dict(producer=name,program=program,expected=expected,command=command,exit=result.returncode,stdout=result.stdout,stderr=result.stderr,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest())
  destination=out/'replay-inputs'/name/Path(program).parent;destination.mkdir(parents=True,exist_ok=True)
  for path in Path(program).parent.rglob('*'):
   if path.is_file() and path.suffix in ['.omg','.lock','.txt'] and not any(part in ['build','target','.omega'] for part in path.relative_to(Path(program).parent).parts[:-1]):
    target=destination/path.relative_to(Path(program).parent);target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,target)
  product=re.search(r'build dir kept: (.+)',result.stderr)
  if product:
   target=out/'products'/f'{name}-{len(records)}';shutil.copytree(Path(product[1]),target,dirs_exist_ok=True);row['product']=str(target)
  row['observed_oracle']=f'native exit {expected}' in result.stderr and f'interp exit: {expected}' in result.stderr
  records.append(row);(out/'runtime.json').write_text(json.dumps(records,indent=2)+'\n');print(name,program,result.returncode,row['observed_oracle'],flush=True)
assert all(row['observed_oracle'] for row in records)
