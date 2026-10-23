"""Freeze a completed baseline/instrumented build and its effective profiles."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

sys.path.insert(0, str(Path('tools').resolve()))
from compiler_profile import artifact_codegen, require_optimized

name = sys.argv[1]
assert name in ('candidate', 'instrumented')
folder = Path('build/placed-result-selection')
rows = []
for line in (folder/(name+'-build.jsonl')).read_text().splitlines():
    try:
        rows.append(json.loads(line))
    except json.JSONDecodeError:
        pass
assert any(row.get('reason')=='build-finished' and row['success'] for row in rows)
executables = [row for row in rows if row.get('reason')=='compiler-artifact'
               and row.get('target',{}).get('name')=='omega' and row.get('executable')]
assert len(executables)==1, executables
binary = folder/('omega-'+name)
source_binary = Path(executables[0]['executable'])
if binary.exists():
    assert hashlib.sha256(binary.read_bytes()).digest() == hashlib.sha256(source_binary.read_bytes()).digest()
else:
    shutil.copy2(source_binary,binary)
require_optimized(binary)
profiles = []
for row in rows:
    if row.get('reason')=='compiler-artifact' and row.get('target',{}).get('name') in ('omega','compiler'):
        original_filenames = list(row['filenames'])
        mapped_executables = []
        if row.get('executable'):
            # mbx reports the public debug/omega link, while its exact Cargo
            # fingerprint belongs to the content-matching build/.../out image.
            digest = hashlib.sha256(binary.read_bytes()).digest()
            mapped_executables = [path for path in Path('build/gate-cli/debug/build/omega').glob('*/out/omega')
                if path.is_file() and hashlib.sha256(path.read_bytes()).digest() == digest]
            assert len(mapped_executables) == 1, mapped_executables
            row = dict(row, filenames=original_filenames+[str(mapped_executables[0].resolve())])
        profiles.append(dict(target=row['target'],profile=row['profile'],
            cargo_filenames=original_filenames,
            matching_executable_paths=[str(path.resolve()) for path in mapped_executables],
            effective=artifact_codegen(row)))
assert profiles
record = dict(base=(folder/'base.txt').read_text().strip(),
    binary=str(binary.resolve()),sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
    profile=json.loads(subprocess.check_output([str(binary.resolve()),'--build-profile'],text=True)),
    profiles=profiles,
    compiler_diff_sha256=hashlib.sha256(subprocess.check_output(['git','diff','--','omega-rust'])).hexdigest())
(folder/(name+'-producer.json')).write_text(json.dumps(record,indent=2)+'\n')
print(name,record['sha256'],record['profile'])
