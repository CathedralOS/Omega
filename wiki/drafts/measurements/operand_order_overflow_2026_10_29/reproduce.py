"""Bounded replay of the unchanged numeric operand-order control."""
import hashlib, json, os, shutil, subprocess, sys, time
from pathlib import Path

root = Path.cwd()
base = root / 'build/operand-order-overflow'
producer = root / 'build/source-resolution-remaining/candidate/omega.exe'
assert hashlib.sha256(producer.read_bytes()).hexdigest() == '10373901d2eee2c7d278da3e56e0b6dc17d8e2e5822a2ec85f69b84b74335b1d'
out = base / 'cases' / sys.argv[1]
out.mkdir(parents=True, exist_ok=False)
project = out / 'input'
project.mkdir()
source = root / 'tests/omega/pass/calls/runtime_numeric_operand_order_exit'
for file in source.glob('*.omg'):
    shutil.copyfile(file, project / file.name)
cache = out / 'cache'
cache.mkdir()
env = dict(os.environ, LOCALAPPDATA=str(cache))
env.pop('OMEGA_PHASE_PROFILE', None)
mode = sys.argv[2]
command = [str(producer), '--check', str(project/'main.omg')] if mode == 'check' else [str(producer), 'update', '--project', str(project), '--target', 'windows_x86_64']
started = time.monotonic()
with (out/'output.txt').open('wb') as log:
    try:
        completed = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=240)
        code = completed.returncode
    except subprocess.TimeoutExpired:
        code = 'timeout'
text = (out/'output.txt').read_text(encoding='utf-8', errors='replace')
record = dict(command=command, exit=code, seconds=time.monotonic()-started, stack_overflow='has overflowed its stack' in text, producer_sha256=hashlib.sha256(producer.read_bytes()).hexdigest(), inputs={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in project.glob('*.omg')})
(out/'result.json').write_text(json.dumps(record, indent=2))
print(json.dumps(record, indent=2), flush=True)
print(text[-2000:], flush=True)
