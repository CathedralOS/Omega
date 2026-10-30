import ast,json,shutil,subprocess,sys
from pathlib import Path
root=Path.cwd();source=root/'build/integer-premise-preparation'
destination=root/'wiki/drafts/measurements/integer_premise_preparation_2026_10_29'
sys.path.insert(0,str(root/'tools/benchmark'));import benchmark
for path in source.iterdir():
    if path.is_file() and path.suffix in ['.py','.json','.patch','.log','.md'] and path.name not in ['claims-status.json','initial-instrumentation.patch','prepare.py','instrument.py','candidate-chain.py','compare.py','quality.py'] and not path.name.startswith('qualification'):
        shutil.copyfile(path,destination/path.name)
for path in (source/'cases').glob('*/*.txt'):
    target=destination/path.relative_to(source);target.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(path,target)
producers={}
for label,path in [('baseline',root/'build/first-checking/candidate/omega.exe'),('diagnostic',source/'diagnostic/omega.exe'),('candidate',source/'candidate/omega.exe')]:
    if path.exists():
        producers[label]=dict(path=str(path),sha256=benchmark.sha256_file(path),profile=json.loads(subprocess.check_output([path,'--build-profile'],text=True)))
controls={}
for record_file in ['diagnostic.json','quality.json','timings.json']:
    if not (source/record_file).exists():continue
    for row in json.loads((source/record_file).read_text()):
        fixture=row['fixture']
        if not fixture.startswith(('pass/','fail/')):fixture='pass/calls/'+fixture
        for path in (root/'tests/omega'/fixture).glob('*'):
            if path.is_file() and path.suffix in ['.omg','.txt']:
                controls[str(path.relative_to(root))]=benchmark.sha256_file(path)
identity=dict(base='88a0a52d5ea0a7d86df92724e31299ce7e4ccf5f',producers=producers,controls=controls,patches={path.name:benchmark.sha256_file(path) for path in source.glob('*.patch') if path.name!='initial-instrumentation.patch'})
(destination/'identity.json').write_text(json.dumps(identity,indent=2),encoding='utf-8')
for path in destination.glob('*.py'):ast.parse(path.read_text(encoding='utf-8'))
for path in destination.glob('*.json'):json.loads(path.read_text(encoding='utf-8'))
print('Archived records and parsed scripts/JSON. Binaries and caches retained locally under build.')
