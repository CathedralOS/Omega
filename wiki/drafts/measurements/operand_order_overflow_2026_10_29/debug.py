"""Capture the overflowing checker stack using the existing symbol-bearing producer."""
import hashlib, json, os, shutil, subprocess, sys, time
from pathlib import Path

root = Path.cwd()
base = root / 'build/operand-order-overflow'
out = base / 'cases' / sys.argv[1]
out.mkdir(parents=True, exist_ok=False)
project = out/'input'
project.mkdir()
source = root/'tests/omega/pass/calls/runtime_numeric_operand_order_exit'
for file in source.glob('*.omg'):
    shutil.copyfile(file, project/file.name)
if len(sys.argv)>2 and sys.argv[2]=='minimal':
    (project/'main.omg').write_text('use omega_language_std::math;\nmachine main() {}\n')
producer = root/'build/pass-producer/debug/omega.exe'
assert hashlib.sha256(producer.read_bytes()).hexdigest() == 'f3371bbeb4acdb4915df4cd7e89875732a597c322aa4fb0ad66e0e02e33914ff'
debugger = Path('C:/Program Files/WindowsApps/Microsoft.WinDbg_1.2606.22001.0_x64__8wekyb3d8bbwe/amd64/cdb.exe')
cache = out/'cache'
cache.mkdir()
env = dict(os.environ, LOCALAPPDATA=str(cache))
env.pop('OMEGA_PHASE_PROFILE',None)
command = [str(debugger), '-G', '-y', str(producer.parent), '-c', 'sxe -c "k 100; q" sov; g', str(producer), 'update','--project',str(project),'--target','windows_x86_64']
started = time.monotonic()
with (out/'debugger.txt').open('wb') as log:
    try:
        result = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=300, creationflags=subprocess.CREATE_NO_WINDOW)
        code = result.returncode
    except subprocess.TimeoutExpired:
        code = 'timeout'
record = dict(command=command, exit=code, seconds=time.monotonic()-started, producer_sha256=hashlib.sha256(producer.read_bytes()).hexdigest(), pdb_sha256=hashlib.sha256(producer.with_suffix('.pdb').read_bytes()).hexdigest())
(out/'result.json').write_text(json.dumps(record,indent=2))
print(json.dumps(record,indent=2),flush=True)
