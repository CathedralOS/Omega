from pathlib import Path
import hashlib,json,os,subprocess
ROOT=Path.cwd();WORK=ROOT/'build/proof-profile/source-checking-cpu';source=json.loads((WORK/'source.json').read_text())
for n,d in source['files'].items():assert hashlib.sha256((ROOT/n).read_bytes()).hexdigest()==d,n
tracked=subprocess.check_output(['git','diff','--name-only','--','omega-rust'],text=True).splitlines()
new=subprocess.check_output(['git','ls-files','--others','--exclude-standard','--','omega-rust'],text=True).splitlines()
assert sorted(tracked+new)==sorted(source['files']) and new==source['new_files']
index=WORK/'scratch.index';assert not index.exists();env=dict(os.environ,GIT_INDEX_FILE=str(index))
try:
    subprocess.run(['git','read-tree',source['base']],env=env,check=True)
    subprocess.run(['git','apply','--cached','--check',str(WORK/'diagnostic.patch')],env=env,check=True)
    subprocess.run(['git','apply','--cached',str(WORK/'diagnostic.patch')],env=env,check=True)
    for n,d in source['files'].items():assert hashlib.sha256(subprocess.check_output(['git','show',':'+n],env=env)).hexdigest()==d,n
finally:
    if index.exists():index.unlink()
(WORK/'patch-checks.json').write_text(json.dumps(dict(base=source['base'],replay_exact=True,files=source['files']),indent=2)+'\n',encoding='utf-8')
subprocess.run(['git','restore','--source=HEAD','--worktree','--',*tracked],check=True)
for n in new:
    p=(ROOT/n).resolve();assert p.is_relative_to(ROOT.resolve());p.unlink()
subprocess.run(['git','diff','--exit-code',source['base'],'--','omega-rust'],check=True)
print('Exact diagnostic patch replays; production sources restored.')
