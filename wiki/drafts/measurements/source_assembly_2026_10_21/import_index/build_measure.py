import subprocess,shutil,sys
from pathlib import Path
f=Path('build/import-index'); name=sys.argv[1]
for command,source,dest in [(['mbx','build','--manifest-path',str(f/'harness/Cargo.toml')],f/'harness/target/debug/assembly-measurement',f/('harness-'+name)),(['mbx','build','-p','omega'],Path('target/debug/omega'),f/('omega-'+name))]:
 with (f/('build-'+dest.name+'.log')).open('w') as log:subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
 shutil.copy2(source,dest)
 if dest.name.startswith('harness-'):shutil.copy2(f/'harness/target/debug/all_threads',f/('harness-all-'+name))
 print('frozen',dest,flush=True)
