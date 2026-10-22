import subprocess,shutil,sys
from pathlib import Path
f=Path('build/import-custody'); name=sys.argv[1]
for command,source,dest in [(['mbx','build','--manifest-path',str(f/'harness/Cargo.toml')],f/'harness/target/debug/assembly-measurement',f/('harness-'+name)),(['mbx','build','-p','omega'],Path('target/debug/omega'),f/('omega-'+name))]:
 with (f/('build-'+dest.name+'.log')).open('w') as log:subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
 shutil.copy2(source,dest)
 print('frozen',dest,flush=True)
