import json, os, shutil, subprocess, sys, time
from pathlib import Path

root = Path.cwd()
out = root / 'build/first-checking'
sys.path.insert(0, str(root / 'tools/benchmark'))
import benchmark

producer = root / 'build/preparation-convergence/candidate/omega.exe'
assert benchmark.sha256_file(producer) == '5a596bcaa6fb117eefd8735c6bb545e0ebbde1cbf1b9b8c3ba79a586a6b16308'
benchmark.require_optimized(producer)
records = []
inputs = {str(p.relative_to(root)): benchmark.sha256_file(p) for p in (root/'source/library/std').rglob('*.omg')}
for fixture in ['runtime_numeric_operand_order_exit']:
    source = root/'tests/omega/pass/calls'/fixture
    directory = out/'cases'/('phase-'+fixture)
    project = directory/'input'
    project.mkdir(parents=True, exist_ok=False)
    for p in source.glob('*.omg'):
        shutil.copyfile(p, project/p.name)
        inputs[str(p.relative_to(root))] = benchmark.sha256_file(p)
    cache = directory/'cache'
    cache.mkdir()
    os.environ['LOCALAPPDATA'] = str(cache)
    os.environ['OMEGA_REVIEW_TIMINGS'] = '1'
    os.environ['OMEGA_PHASE_PROFILE'] = '1'
    def invoke(command, cwd=None):
        started = time.monotonic()
        result = subprocess.run(command, cwd=cwd or root, capture_output=True, timeout=360)
        text = (result.stdout+result.stderr).decode('utf-8','replace')
        transcript = directory/f'{len(records):02}.txt'
        transcript.write_text(text, encoding='utf-8', newline='\n')
        row = dict(fixture=fixture, command=[str(a) for a in command], exit=result.returncode, seconds=time.monotonic()-started, transcript=str(transcript.relative_to(out)), producer_sha256=benchmark.sha256_file(producer))
        records.append(row)
        (out/'phase.json').write_text(json.dumps(records,indent=2),encoding='utf-8')
        print(f'{fixture}: {command[1]} exit {result.returncode} in {row["seconds"]:.2f}s',flush=True)
        return result.returncode,text
    benchmark.run_plain = invoke
    benchmark.settle_package_review(producer, project, 'windows_x86_64')
    assert records[-2]['exit'] == 3 and records[-1]['exit'] == 0
    code,text = invoke([producer,'run','--both','--keep','--target','windows_x86_64',project/'main.omg'])
    assert code == 0 and 'DECLINED' not in text, text
for path,digest in inputs.items():
    assert benchmark.sha256_file(root/path) == digest
(out/'phase-inputs.json').write_text(json.dumps(dict(revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(), inputs=inputs),indent=2),encoding='utf-8')
