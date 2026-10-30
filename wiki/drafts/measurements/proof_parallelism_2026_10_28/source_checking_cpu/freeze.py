import sys
from pathlib import Path
import subprocess,json,hashlib,shutil,os
r=Path.cwd();d=r/'build/proof-profile/producers'/sys.argv[1];d.mkdir(exist_ok=False)
s=r/'build/proof-profile/compiler/debug/omega.exe'
# This script must be run only after the build wrapper exits successfully.
shutil.copy2(s,d/'omega.exe')
p=subprocess.run([str(d/'omega.exe'),'--build-profile'],capture_output=True,check=True)
(d/'build-profile.json').write_bytes(p.stdout)
print(p.stdout.decode())
env=os.environ.copy();env['OMEGA_LIBRARY_ROOT']=str(r/'source/library')
p=subprocess.run([str(d/'omega.exe'),'update','--project',str(r/'samples/cli/games/dungeon_crawler_cli'),'--target','windows_x86_64','--offline'],env=env,capture_output=True)
(r/('build/proof-profile/'+sys.argv[1]+'-update.log')).write_bytes(p.stdout+p.stderr)
assert p.returncode==0,(p.stdout+p.stderr).decode()[-3000:]
shutil.copy2(r/'samples/cli/games/dungeon_crawler_cli/omega.lock',d/'omega.lock')
print('frozen',hashlib.sha256((d/'omega.exe').read_bytes()).hexdigest())
