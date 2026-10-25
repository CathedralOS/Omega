"""Link the byte-exact validator against one verified compiler build (Linux).
No Cargo dependency acquisition or compiler rebuild. Refuse ambiguous artifacts.
The pinned Rust build separates full rmeta from rlib metadata stubs; pass both.
"""
from pathlib import Path
import argparse, subprocess, json, hashlib
parser=argparse.ArgumentParser(); parser.add_argument('--target-dir',required=True); parser.add_argument('--build-record'); options=parser.parse_args()
root=Path(__file__).resolve().parents[1]; target=Path(options.target_dir).resolve()
output=root/'build/validation'; output.mkdir(parents=True,exist_ok=True)
source=root/'reference/terminal_validate.rs'; binary=output/'terminal_validate'
artifacts={}
artifact_directories=set()
if options.build_record:
    for line in Path(options.build_record).read_text().splitlines():
        try: row=json.loads(line)
        except ValueError: continue
        if row.get('reason')!='compiler-artifact' or row.get('target',{}).get('kind')!=['lib']: continue
        libraries=[Path(name).resolve() for name in row.get('filenames',[]) if name.endswith('.rlib')]
        assert len(libraries)==1,(row,'expected one complete library')
        artifact_directories.add(libraries[0].parent)
        name=row['target']['name']
        if name in {'compiler', 'terminal_runtime','package_manager','terminal_interpreter','target','omega_build'}:
            previous=artifacts.setdefault(name,libraries[0])
            assert previous==libraries[0],(name,'ambiguous build record')
command=['rustc','--edition=2024','-C','opt-level=1',str(source),'-o',str(binary)]
directories=artifact_directories | set((target/'build').glob('*/*/out'))
for directory in sorted(directories): command+=['-L',str(directory)]
if (target/'deps').is_dir(): command+=['-L',str(target/'deps')]
for package,name in [('compiler','compiler'),('terminal-runtime','terminal_runtime'),('package-manager','package_manager'),('terminal-interpreter','terminal_interpreter'),('target','target'),('omega-build','omega_build')]:
    candidates=[artifacts[name]] if artifacts else list((target/'build'/package).glob(f'*/out/lib{name}-*.rlib'))
    if not candidates: candidates=list((target/'deps').glob(f'lib{name}-*.rlib'))
    assert len(candidates)==1,(package,'use one isolated verified build',candidates)
    library=candidates[0]; metadata=library.with_suffix('.rmeta')
    if not metadata.exists() and artifacts:
        # mbx can report Cargo's normalized public rlib export without its
        # paired metadata path. Recover the exact output by byte identity,
        # never by recency or an arbitrary matching filename.
        digest=hashlib.sha256(library.read_bytes()).digest()
        matches=[path for path in (target/'build'/package).glob(f'*/out/lib{name}-*.rlib')
                 if hashlib.sha256(path.read_bytes()).digest()==digest]
        assert len(matches)==1,(package,'export has no unique byte-identical output',matches)
        library=matches[0]; metadata=library.with_suffix('.rmeta')
    if metadata.exists(): command+=['--extern',f'{name}={metadata}']
    command+=['--extern',f'{name}={library}']
result=subprocess.run(command,capture_output=True)
(output/'helper-build.stdout').write_bytes(result.stdout); (output/'helper-build.stderr').write_bytes(result.stderr)
record={'build_record_sha256':hashlib.sha256(Path(options.build_record).read_bytes()).hexdigest() if options.build_record else None,'command':command,'exit':result.returncode,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest() if binary.exists() else None}
(output/'helper-build.json').write_text(json.dumps(record,indent=2)+'\n')
print(result.stderr.decode()); raise SystemExit(result.returncode)
