import hashlib,json,re,shutil,subprocess
from pathlib import Path
root=Path.cwd()
base=root/'build/contract-query-publication'
record=root/'wiki/drafts/measurements/operand_order_overflow_2026_10_29/publication'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def copy(a,b):
    b.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(a,b)
assert not subprocess.check_output(['git','diff','f1058703209f782fc08fc621471257095a2f7f47','--','omega-rust','Cargo.lock','Cargo.toml','rust-toolchain.toml'])
rows=json.loads((base/'quality.json').read_text())
native=[r for r in rows if '--timings' in r['command']]
both=[r for r in rows if '--both' in r['command']]
rejects=[r for r in rows if r.get('matched')]
assert len(native)==len(both)==1 and len(rejects)==1
assert all(r['exit']==r['native_exit']==0 for r in native)
inputs={}
for row in rows:
    assert sha(Path(row['command'][0]))==row['producer_sha256']
    transcript=Path(row['transcript'])
    text=transcript.read_text()
    source=root/'tests/omega'/row['fixture']
    assert sha(source/'main.omg')==row['source_sha256']
    for p in source.glob('*.omg'):
        assert sha(p)==sha(transcript.parent/'input'/p.name)
        inputs[str(p.relative_to(root))]=sha(p)
    if '--both' in row['command']:
        assert row['exit']==0 and 'DECLINED' not in text
        match=re.search(r'build dir kept: (.+)',text)
        assert match
        artifacts=Path(match.group(1).strip())
        compiled=next(r for r in native if r['fixture']==row['fixture'])
        assert sha(artifacts/'omega-program.exe')==compiled['artifact_sha256']
        row['native_sha256']=sha(artifacts/'omega-program.exe')
        row['terminal_sha256']=sha(artifacts/'program.oterm')
    if row.get('matched'):
        assert row['exit']!=0 and all(e in text for e in row['expected'])
    relative=Path(source.name)/transcript.name
    copy(transcript,record/relative)
    row['transcript']=str(relative)
for p in (root/'source/library/std').rglob('*.omg'):
    inputs[str(p.relative_to(root))]=sha(p)
result=dict(compiler_source_revision='f1058703209f782fc08fc621471257095a2f7f47',built_checkout_revision='54eb79adaa9c23a2e610af12d0dc2d1a1344ddc3',compiler_tree=subprocess.check_output(['git','rev-parse','HEAD:omega-rust'],text=True).strip(),inputs=inputs,records=rows)
(record/'results.json').write_text(json.dumps(result,indent=2),encoding='utf-8',newline='\n')
copy(base/'quality.py',record/'quality.py')
copy(base/'collect.py',record/'collect.py')
copy(root.parent/'pass-ablation/build/package-first-preparation/publication-build.log',record/'build.log')
print('Verified new native/Terminal regression and expected reject; larger control remains unqualified')

for name in ['initial-timeout.json','diagnostic-stop.json','main-preparation-stack.json','main-preparation-stack.txt']:
    copy(base/name,record/name)
