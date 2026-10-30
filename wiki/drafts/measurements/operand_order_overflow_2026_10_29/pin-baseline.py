"""Verify the committed regression shape still fails on the exact old producer."""
import hashlib,json,subprocess,time
from pathlib import Path

root=Path.cwd()
base=root/'build/operand-order-overflow'
out=base/'cases/pinned-baseline'
out.mkdir(exist_ok=False)
source=root/'tests/omega/pass/calls/selected_contract_equation_reentry_exit/main.omg'
(out/'main.omg').write_bytes(source.read_bytes())
producer=root/'build/source-resolution-remaining/candidate/omega.exe'
assert hashlib.sha256(producer.read_bytes()).hexdigest()=='10373901d2eee2c7d278da3e56e0b6dc17d8e2e5822a2ec85f69b84b74335b1d'
command=[str(producer),'--check',str(out/'main.omg')]
started=time.monotonic()
with (out/'output.txt').open('wb') as log:
    try:
        code=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,timeout=120).returncode
    except subprocess.TimeoutExpired:
        code='timeout'
text=(out/'output.txt').read_text(encoding='utf-8',errors='replace')
record=dict(command=command,exit=code,seconds=time.monotonic()-started,stack_overflow='has overflowed its stack' in text,producer_sha256=hashlib.sha256(producer.read_bytes()).hexdigest(),source_sha256=hashlib.sha256(source.read_bytes()).hexdigest())
(out/'result.json').write_text(json.dumps(record,indent=2))
print(json.dumps(record,indent=2),flush=True)
assert code==3221225725 and record['stack_overflow'],text[-3000:]
