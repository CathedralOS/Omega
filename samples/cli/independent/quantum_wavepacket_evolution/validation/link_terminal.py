"""Link the public Terminal sponsor against one successful Omega CLI build.
Usage: link_terminal.py CARGO_JSON REPOSITORY OUTPUT_BINARY
Uses full metadata plus matching stub libraries when mbx emits split artifacts.
This builds a sponsor; it does not prepare or accept source packages.
"""
from pathlib import Path
import hashlib,json,subprocess,sys
here=Path(__file__).resolve().parent
messages=Path(sys.argv[1])
root=Path(sys.argv[2]).resolve() if len(sys.argv)>2 else Path.cwd().resolve()
dependencies={'compiler', 'terminal_runtime','package_manager','target','terminal_interpreter','compilation_products'}
selected={}
metadata={}
build_success=False
source=here/'terminal_sdk.rs'
assert source.is_file()
dependency_dirs=set()
cli_binary=None
for line in messages.read_text().splitlines():
 try: row=json.loads(line)
 except json.JSONDecodeError: continue
 if row.get('reason')=='build-finished':
  assert row.get('success') is True
  build_success=True
 if row.get('reason')!='compiler-artifact':continue
 name=row.get('target',{}).get('name')
 if name=='omega' and row.get('executable'):
  assert Path(row['manifest_path']).resolve().is_relative_to(root)
  cli_binary=Path(row['executable']).resolve()
 for filename in row.get('filenames',[]):
  path=Path(filename)
  if path.suffix in {'.rlib','.rmeta','.so'}:
   assert path.is_file()
   dependency_dirs.add(path.resolve().parent)
 if name not in dependencies:continue
 assert Path(row['manifest_path']).resolve().is_relative_to(root)
 libraries=[Path(x) for x in row['filenames'] if x.endswith('.rlib')]
 assert len(libraries)==1,(name,libraries)
 assert libraries[0].is_file()
 assert name not in selected or selected[name]==libraries[0],(name,selected,libraries)
 selected[name]=libraries[0]
 full_metadata=[Path(x) for x in row['filenames'] if x.endswith('.rmeta')]
 assert len(full_metadata)==1 and full_metadata[0].is_file()
 assert full_metadata[0].with_suffix('.rlib')==libraries[0]
 assert name not in metadata or metadata[name]==full_metadata[0]
 metadata[name]=full_metadata[0]
assert build_success and set(selected)==dependencies,selected
assert cli_binary is not None and cli_binary.is_file()
assert all(p.is_relative_to(cli_binary.parent) for p in dependency_dirs)
assert all(p.resolve().parent in dependency_dirs for p in selected.values())
binary=Path(sys.argv[3]).resolve()
binary.parent.mkdir(parents=True,exist_ok=True)
command=['rustc','--edition=2024','-C','opt-level=1','-C','debuginfo=0']
for directory in sorted(dependency_dirs):command+=['-L','dependency='+str(directory)]
for name,path in sorted(metadata.items()):command+=['--extern',name+'='+str(path),'--extern',name+'='+str(selected[name])]
command += [str(source),'-o',str(binary)]
subprocess.run(command,check=True)
receipt={'method':'direct rustc over exact root CLI-build full metadata and matching stub libraries',
 'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),
 'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
 'cli_binary_sha256':hashlib.sha256(cli_binary.read_bytes()).hexdigest(),
 'command':command,'dependencies':{n:{'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),
 'metadata_path':str(metadata[n]),'metadata_sha256':hashlib.sha256(metadata[n].read_bytes()).hexdigest()} for n,p in selected.items()}}
binary.with_suffix('.link.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({'binary_sha256':receipt['binary_sha256'],'dependencies':list(selected)}))
