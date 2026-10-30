from pathlib import Path
import hashlib,json,os,subprocess
ROOT=Path.cwd();WORK=ROOT/'build/proof-profile/graph-phase-cpu';source=json.loads((WORK/'source.json').read_text())
for name,digest in source['files'].items():assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==digest
tracked=subprocess.check_output(['git','diff','--name-only','--','omega-rust'],text=True).splitlines()
untracked=subprocess.check_output(['git','ls-files','--others','--exclude-standard','--','omega-rust'],text=True).splitlines()
assert sorted(tracked+untracked)==sorted(source['files']) and untracked==source['new_files']
index=WORK/'scratch.index';assert not index.exists();env=dict(os.environ,GIT_INDEX_FILE=str(index))
try:
    subprocess.run(['git','read-tree',source['base']],env=env,check=True)
    subprocess.run(['git','apply','--cached','--check',str(WORK/'diagnostic.patch')],env=env,check=True)
    subprocess.run(['git','apply','--cached',str(WORK/'diagnostic.patch')],env=env,check=True)
    for name,digest in source['files'].items():assert hashlib.sha256(subprocess.check_output(['git','show',':'+name],env=env)).hexdigest()==digest
finally:
    if index.exists():index.unlink()
(WORK/'patch-checks.json').write_text(json.dumps(dict(base=source['base'],applies=True,files=source['files']),indent=2)+'\n',encoding='utf-8',newline='\n')
subprocess.run(['git','restore','--source=HEAD','--worktree','--',*tracked],check=True)
for name in untracked:
    target=(ROOT/name).resolve();assert target.is_relative_to(ROOT.resolve());target.unlink()
subprocess.run(['git','diff','--exit-code',source['semantic_base'],'--','omega-rust'],check=True)
print('Exact three-file diagnostic patch replays; original compiler sources restored and own temporary module removed.')
