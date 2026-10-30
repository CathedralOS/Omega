import hashlib, json, os, subprocess, time
from pathlib import Path

root = Path.cwd()
out = root/'build/operand-order-overflow/cases/inline-single-evaluation'
out.mkdir(parents=True, exist_ok=False)
source = (root/'tests/omega/pass/calls/runtime_numeric_operand_single_evaluation_exit/main.omg').read_text()
math = (root/'source/library/std/math.omg').read_text()
helper = math[math.index('public machine abs_i32'):math.index('public machine min_i64')]
source = source.replace('use omega_language_std::math;', helper)
(out/'main.omg').write_text(source)
producer = root/'build/source-resolution-remaining/candidate/omega.exe'
command = [str(producer),'--check',str(out/'main.omg')]
started = time.monotonic()
with (out/'output.txt').open('wb') as log:
    try:
        code = subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,timeout=120).returncode
    except subprocess.TimeoutExpired:
        code = 'timeout'
text = (out/'output.txt').read_text()
record = dict(command=command,exit=code,seconds=time.monotonic()-started,stack_overflow='has overflowed its stack' in text,source_sha256=hashlib.sha256((out/'main.omg').read_bytes()).hexdigest())
(out/'result.json').write_text(json.dumps(record,indent=2))
print(json.dumps(record,indent=2),flush=True)
print(text[-2000:],flush=True)
subprocess.run(['python',str(root/'build/operand-order-overflow/candidate-chain.py')],check=True)
