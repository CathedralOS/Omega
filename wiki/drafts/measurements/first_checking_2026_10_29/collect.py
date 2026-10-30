from pathlib import Path
import json,shutil
root=Path.cwd(); source=root/'build/first-checking'; target=root/'wiki/drafts/measurements/first_checking_2026_10_29'
target.mkdir(parents=True,exist_ok=True)
for path in source.iterdir():
    if path.is_file() and path.suffix in {'.py','.json','.md','.patch','.log','.txt'}:
        shutil.copyfile(path,target/path.name)
for path in (source/'cases').glob('*/*.txt'):
    dest=target/path.relative_to(source); dest.parent.mkdir(parents=True,exist_ok=True); shutil.copyfile(path,dest)
