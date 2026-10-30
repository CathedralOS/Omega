import json, subprocess, sys
from pathlib import Path
root=Path.cwd(); out=root/'build/first-checking'
sys.path.insert(0,str(root/'tools/benchmark')); import benchmark
rows=json.loads((out/'timings.json').read_text())+json.loads((out/'quality.json').read_text())
for row in rows:
    if 'artifact_sha256' not in row or row.get('variant')=='A': continue
    transcript=Path(row['transcript'])
    if not transcript.is_absolute(): transcript=out/transcript
    _,artifact=benchmark.parse_timings(transcript.read_text())
    assert artifact and benchmark.sha256_file(artifact)==row['artifact_sha256']
    fixture=row['fixture']
    if not fixture.startswith('pass/'): fixture='pass/calls/'+fixture
    expected=int((root/'tests/omega'/fixture/'expected_native_exit.txt').read_text())
    result=subprocess.run([artifact],capture_output=True,timeout=30)
    assert result.returncode==expected,(artifact,result.returncode,expected)
    print(f'{fixture}: native {result.returncode}; sha256 {row["artifact_sha256"]}',flush=True)
