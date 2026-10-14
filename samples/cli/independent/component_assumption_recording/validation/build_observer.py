"""Link an observer against one existing compiler build; never rebuild the compiler."""
import json, os, pathlib, subprocess, sys
root=pathlib.Path(sys.argv[1]).resolve()
source=pathlib.Path(__file__).with_name('observe.rs').resolve()
repo=source.parents[5]
names=('symbols','compiler','package_manager','target','terminal_interpreter','build_declarations','diagnostics','package_compilation','semantic_vocabulary','source_files_to_tokens','syntax_trees','tokens_to_syntax_trees','source','effects','terminal_psi','native_artifact','terminal_psi_to_abstract_operations','component_description','terminal_codec')
archives=list(root.rglob('*.rlib'))+list(root.rglob('*.so'))
command=['rustc','--edition=2024','-O',str(source),'-o',sys.argv[2]]
for folder in sorted({p.parent for p in archives}): command+=['-L',f'dependency={folder}']
selected={}
selection=json.loads(pathlib.Path(sys.argv[3]).read_text()) if len(sys.argv)>3 else None
for name in names:
    candidates=[pathlib.Path(selection[name])] if selection else list(root.rglob(f'lib{name}-*.rmeta'))
    assert len(candidates)==1,(name,candidates)
    path=candidates[0]; assert path.with_suffix('.rlib').is_file()
    command+=['--extern',f'{name}={path}','--extern',f'{name}={path.with_suffix(".rlib")}']
    selected[name]=str(path)
environment=dict(os.environ,CARGO_MANIFEST_DIR=str(repo/'omega-rust/omega/compilation/driver'))
result=subprocess.run(command,env=environment)
pathlib.Path(sys.argv[2]+'.link.json').write_text(json.dumps({'command':command,'selected':selected,'exit':result.returncode},indent=2))
raise SystemExit(result.returncode)
