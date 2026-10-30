import json,shutil,subprocess,sys
from pathlib import Path
root=Path.cwd();source=root/'build/remaining-first-checking'
destination=root/'wiki/drafts/measurements/remaining_first_checking_2026_10_30'
sys.path.insert(0,str(root/'tools/benchmark'));import benchmark
for path in source.iterdir():
    if path.is_file() and path.suffix in ['.py','.json','.patch','.log','.md']:
        shutil.copyfile(path,destination/path.name)
for path in (source/'cases').glob('*/*.txt'):
    target=destination/path.relative_to(source);target.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(path,target)
name='specification_result_without_guarantees_requires_formation'
controls={}
for kind in ['pass','fail']:
    folder=root/'tests/omega'/kind/'calls'/name
    for path in folder.iterdir():
        if path.is_file():
            target=destination/'inputs'/kind/path.name;target.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(path,target)
            controls[str(path.relative_to(root))]=benchmark.sha256_file(path)
producers={}
for label,path in [('baseline',root/'build/first-checking/candidate/omega.exe'),('diagnostic',source/'diagnostic/omega.exe'),('candidate',source/'candidate/omega.exe')]:
    producers[label]=dict(path=str(path),sha256=benchmark.sha256_file(path),profile=json.loads(subprocess.check_output([path,'--build-profile'],text=True)))
(destination/'identity.json').write_text(json.dumps(dict(base='0ac181bf065e398d6d6ae455c2659416895d6b49',producers=producers,controls=controls,candidate_patch_sha256=benchmark.sha256_file(source/'candidate.patch'),diagnostic_patch_sha256=benchmark.sha256_file(source/'diagnostic.patch')),indent=2),encoding='utf-8')
print('Archived diagnostic and comparison records; retained binaries and caches remain under build.')
