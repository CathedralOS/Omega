"""Link the private capture harness against the one verified compiler build.
Usage: python3 build_terminal.py TARGET_DIRECTORY OUTPUT_BINARY [METADATA_MAP_JSON]
No new crates, compiler modifications, or additional concurrent Rust build.
"""
import json, pathlib, subprocess, sys
root=pathlib.Path(sys.argv[1]).resolve()
source=pathlib.Path(__file__).with_name('terminal.rs')
command=['rustc','--edition=2024','-O',str(source),'-o',sys.argv[2]]
archives=list(root.rglob('*.rlib'))
archives+=list(root.rglob('*.so'))
metadata=list(root.rglob('*.rmeta'))
selection=json.loads(pathlib.Path(sys.argv[3]).read_text()) if len(sys.argv)>3 else None
if selection is not None:
    assert set(selection)=={'compiler', 'terminal_runtime','omega_build', 'package_manager','target','terminal_interpreter'}
for folder in sorted({p.parent for p in archives}):command+=['-L',f'dependency={folder}']
for name in ('compiler', 'terminal_runtime','omega_build', 'package_manager','target','terminal_interpreter'):
    candidates=[(root/selection[name]).resolve()] if selection is not None else [p for p in metadata if p.name.startswith(f'lib{name}-')]
    assert len(candidates)==1,(name,'ambiguous artifacts: supply an explicit metadata map',candidates)
    selected=candidates[0]
    assert selected.is_relative_to(root) and selected.is_file() and selected.with_suffix('.rlib').is_file(),selected
    command+=['--extern',f'{name}={selected}','--extern',f'{name}={selected.with_suffix(".rlib")}']
print(' '.join(command),flush=True)
raise SystemExit(subprocess.run(command).returncode)
