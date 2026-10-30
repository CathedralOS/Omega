from pathlib import Path
import hashlib,json,os,subprocess
ROOT=Path.cwd();WORK=ROOT/'build/proof-profile/same-binary-graph';identity=json.loads((WORK/'source.json').read_text());name=identity['file']
assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==identity['source_sha256']
assert subprocess.check_output(['git','diff','--name-only','--','omega-rust'],text=True).splitlines()==[name]
assert not subprocess.check_output(['git','ls-files','--others','--exclude-standard','--','omega-rust']).strip()
index=WORK/'scratch.index';assert not index.exists();env=dict(os.environ,GIT_INDEX_FILE=str(index))
try:
    subprocess.run(['git','read-tree',identity['base']],env=env,check=True)
    subprocess.run(['git','apply','--cached','--check',str(WORK/'diagnostic.patch')],env=env,check=True)
    subprocess.run(['git','apply','--cached',str(WORK/'diagnostic.patch')],env=env,check=True)
    digest=hashlib.sha256(subprocess.check_output(['git','show',':'+name],env=env)).hexdigest();assert digest==identity['source_sha256']
finally:
    if index.exists():index.unlink()
(WORK/'patch-checks.json').write_text(json.dumps(dict(base=identity['base'],applies=True,patched_source_sha256=digest),indent=2)+'\n',encoding='utf-8',newline='\n')
subprocess.run(['git','restore','--source=HEAD','--worktree','--',name],check=True)
subprocess.run(['git','diff','--exit-code',identity['semantic_base'],'--','omega-rust'],check=True)
print('Verified patch replay and source digest; restored original compiler sources.')
